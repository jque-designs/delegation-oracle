use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use delegation_oracle::alert::engine::evaluate_alerts;
use delegation_oracle::alert::rules::AlertEventKind;
use delegation_oracle::alert::sink::{AlertSink, StdoutSink, WebhookSink};
use delegation_oracle::config::{Config, ConfigOverrides};
use delegation_oracle::criteria::{build_drift_report, CriteriaDrift, MetricKey, ProgramId};
use delegation_oracle::eligibility::arbitrage::build_arbitrage_opportunities;
use delegation_oracle::eligibility::evaluator::evaluate_validator;
use delegation_oracle::eligibility::history::{record_from_result, summarize_timeline};
use delegation_oracle::eligibility::vulnerability::analyze_vulnerabilities;
use delegation_oracle::eligibility::{
    ArbitrageOpportunity, EligibilityResult, VulnerableValidator,
};
use delegation_oracle::metrics::collector::{
    collect_validator_metrics, sample_competitors, MetricOverrides,
};
use delegation_oracle::metrics::normalize::normalize_metrics;
use delegation_oracle::optimizer::conflicts::detect_conflicts;
use delegation_oracle::optimizer::recommendations::build_recommendations;
use delegation_oracle::optimizer::whatif::simulate_whatif;
use delegation_oracle::optimizer::{OptimizationRecommendation, WhatIfResult};
use delegation_oracle::output::csv::{arbitrage_to_csv, status_to_csv};
use delegation_oracle::output::json::render_json;
use delegation_oracle::output::table::{
    render_arbitrage_table, render_drift_table, render_gaps_table, render_history_table,
    render_recommendations_table, render_status_table, render_vulnerability_table,
    render_whatif_table,
};
use delegation_oracle::programs::ProgramRegistry;
use delegation_oracle::server::run_server;
use delegation_oracle::snapshot::store::SnapshotStore;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Csv,
}

#[derive(Debug, Parser)]
#[command(
    name = "delegation-oracle",
    about = "Unified delegation program intelligence"
)]
struct Cli {
    #[arg(short, long)]
    validator: Option<String>,
    #[arg(short, long)]
    config: Option<PathBuf>,
    #[arg(short, long)]
    rpc: Option<String>,
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    output: OutputFormat,
    #[arg(short = 'p', long)]
    programs: Option<String>,
    #[command(flatten)]
    metrics: MetricArgs,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, clap::Args, Clone, Default)]
struct MetricArgs {
    #[arg(long)]
    commission: Option<f64>,
    #[arg(long = "activated-stake")]
    activated_stake: Option<f64>,
    #[arg(long = "skip-rate")]
    skip_rate: Option<f64>,
    #[arg(long = "vote-credits")]
    vote_credits: Option<f64>,
    #[arg(long = "uptime")]
    uptime_percent: Option<f64>,
    #[arg(long = "solana-version")]
    solana_version: Option<String>,
    #[arg(long = "datacenter-concentration")]
    datacenter_concentration: Option<f64>,
    #[arg(long = "superminority")]
    superminority_status: Option<bool>,
    #[arg(long = "mev-commission")]
    mev_commission: Option<f64>,
    #[arg(long = "stake-concentration")]
    stake_concentration: Option<f64>,
    #[arg(long = "infra-diversity")]
    infrastructure_diversity: Option<f64>,
}

impl From<MetricArgs> for MetricOverrides {
    fn from(value: MetricArgs) -> Self {
        Self {
            commission: value.commission,
            activated_stake: value.activated_stake,
            skip_rate: value.skip_rate,
            vote_credits: value.vote_credits,
            uptime_percent: value.uptime_percent,
            solana_version: value.solana_version,
            datacenter_concentration: value.datacenter_concentration,
            superminority_status: value.superminority_status,
            mev_commission: value.mev_commission,
            stake_concentration: value.stake_concentration,
            infrastructure_diversity: value.infrastructure_diversity,
        }
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    Status,
    Gaps,
    Arbitrage {
        #[arg(long, default_value = "roi")]
        sort: String,
    },
    Whatif {
        #[arg(long)]
        commission: Option<f64>,
        #[arg(long = "skip-rate")]
        skip_rate: Option<f64>,
        #[arg(long = "mev-commission")]
        mev_commission: Option<f64>,
        #[arg(long = "activated-stake")]
        activated_stake: Option<f64>,
        #[arg(long = "uptime")]
        uptime_percent: Option<f64>,
    },
    Vulnerable {
        #[arg(long)]
        program: Option<String>,
        #[arg(long, default_value_t = 5.0)]
        margin: f64,
    },
    Drift {
        #[arg(long, default_value_t = 5)]
        since: u64,
    },
    History {
        #[arg(long, default_value_t = 50)]
        epochs: usize,
        #[arg(long)]
        program: Option<String>,
    },
    Optimize {
        #[arg(long, default_value_t = 5)]
        top: usize,
    },
    Watch {
        #[arg(long, default_value_t = 60)]
        interval_secs: u64,
        #[arg(long)]
        vulnerability_interval_secs: Option<u64>,
        #[arg(long)]
        drift_interval_secs: Option<u64>,
        #[arg(long, default_value_t = 1)]
        iterations: u32,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 3001)]
        port: u16,
    },
    Config {
        #[arg(long)]
        init: bool,
        #[arg(long)]
        show: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let config_path = cli.config.clone().unwrap_or_else(Config::default_path);
    let mut config = Config::load(Some(&config_path))?;
    config.apply_overrides(ConfigOverrides {
        vote_pubkey: cli.validator.clone(),
        rpc_url: cli.rpc.clone(),
        enabled_programs: cli
            .programs
            .as_ref()
            .map(|raw| {
                parse_program_list(raw.as_str()).map(|ids| {
                    ids.into_iter()
                        .map(|id| id.as_slug().to_string())
                        .collect::<Vec<_>>()
                })
            })
            .transpose()?,
    });
    let selected_programs = resolve_selected_programs(&config, cli.programs.as_deref())?;

    if matches!(cli.command, Commands::Config { .. }) {
        return handle_config_command(&cli.command, &config, &config_path);
    }
    if let Commands::Serve { host, port } = &cli.command {
        let bind = format!("{host}:{port}");
        let addr: SocketAddr = bind
            .parse()
            .map_err(|e| anyhow!("invalid bind address {bind}: {e}"))?;
        return run_server(config, addr).await;
    }

    let registry = ProgramRegistry::with_defaults();
    let metric_overrides: MetricOverrides = cli.metrics.clone().into();
    let mut your_metrics = collect_validator_metrics(
        Some(config.validator.vote_pubkey.as_str()),
        &config.rpc.url,
        &metric_overrides,
    )
    .await?;
    normalize_metrics(&mut your_metrics);

    let db_path = config.resolved_db_path();
    let store = SnapshotStore::open(&db_path)?;

    match &cli.command {
        Commands::Status => {
            let (results, _, _) =
                evaluate_selected_programs(&registry, &selected_programs, &your_metrics).await?;
            persist_eligibility_history(&store, &your_metrics.vote_pubkey, &results)?;
            print_status(&results, cli.output)?;
        }
        Commands::Gaps => {
            let (results, _, _) =
                evaluate_selected_programs(&registry, &selected_programs, &your_metrics).await?;
            print_gaps(&results, cli.output)?;
        }
        Commands::Arbitrage { sort } => {
            let (results, _, estimate_by_program) =
                evaluate_selected_programs(&registry, &selected_programs, &your_metrics).await?;
            let mut opportunities = build_arbitrage_opportunities(&results, &estimate_by_program);
            if sort.eq_ignore_ascii_case("effort") {
                opportunities.sort_by(|a, b| a.total_effort.cmp(&b.total_effort));
            }
            print_arbitrage(&opportunities, cli.output)?;
        }
        Commands::Whatif {
            commission,
            skip_rate,
            mev_commission,
            activated_stake,
            uptime_percent,
        } => {
            let mut changes = Vec::new();
            if let Some(v) = commission {
                changes.push((MetricKey::Commission, *v));
            }
            if let Some(v) = skip_rate {
                changes.push((MetricKey::SkipRate, *v));
            }
            if let Some(v) = mev_commission {
                changes.push((MetricKey::MevCommission, *v));
            }
            if let Some(v) = activated_stake {
                changes.push((MetricKey::ActivatedStake, *v));
            }
            if let Some(v) = uptime_percent {
                changes.push((MetricKey::UptimePercent, *v));
            }
            if changes.is_empty() {
                return Err(anyhow!(
                    "at least one --<metric> change is required for whatif"
                ));
            }
            let result = simulate_whatif(
                &registry,
                &your_metrics,
                &changes,
                Some(selected_programs.as_slice()),
            )
            .await?;
            print_whatif(&result, cli.output)?;
        }
        Commands::Vulnerable { program, margin } => {
            let target_programs = if let Some(program) = program {
                vec![ProgramId::from_str(program)?]
            } else {
                selected_programs.clone()
            };
            let competitors = sample_competitors(&your_metrics);
            let mut combined: Vec<VulnerableValidator> = Vec::new();
            for pid in &target_programs {
                let Some(program_impl) = registry.by_id(*pid) else {
                    continue;
                };
                let criteria = program_impl.fetch_criteria().await?;
                combined.extend(analyze_vulnerabilities(
                    *pid,
                    &criteria,
                    &competitors,
                    *margin,
                ));
            }
            print_vulnerable(&combined, cli.output)?;
        }
        Commands::Drift { since: _ } => {
            let drifts =
                run_drift_detection(&registry, &store, &selected_programs, &your_metrics).await?;
            print_drift(&drifts, cli.output)?;
        }
        Commands::History { epochs, program } => {
            let program_filter = program.as_deref().map(ProgramId::from_str).transpose()?;
            let history = store.load_history(&your_metrics.vote_pubkey, program_filter, *epochs)?;
            let summary = summarize_timeline(&history, program_filter);
            match cli.output {
                OutputFormat::Table => {
                    println!("{}", render_history_table(&history));
                    println!("{summary}");
                }
                OutputFormat::Json => println!("{}", render_json(&history)?),
                OutputFormat::Csv => {
                    warn!("CSV output for history not implemented, using JSON");
                    println!("{}", render_json(&history)?);
                }
            }
        }
        Commands::Optimize { top } => {
            let (results, criteria_sets, estimate_by_program) =
                evaluate_selected_programs(&registry, &selected_programs, &your_metrics).await?;
            let opportunities = build_arbitrage_opportunities(&results, &estimate_by_program);
            let conflicts = detect_conflicts(&criteria_sets);
            let recommendations = build_recommendations(&opportunities, &conflicts, *top);
            print_recommendations(&recommendations, cli.output)?;
        }
        Commands::Watch {
            interval_secs,
            vulnerability_interval_secs,
            drift_interval_secs,
            iterations,
        } => {
            run_watch_loop(
                &registry,
                &store,
                &config,
                &selected_programs,
                &config.validator.vote_pubkey,
                &config.rpc.url,
                &metric_overrides,
                *interval_secs,
                *vulnerability_interval_secs,
                *drift_interval_secs,
                *iterations,
            )
            .await?;
        }
        Commands::Config { .. } => {}
        Commands::Serve { .. } => unreachable!("serve command handled before dispatch"),
    }

    Ok(())
}

fn handle_config_command(command: &Commands, config: &Config, config_path: &PathBuf) -> Result<()> {
    let Commands::Config { init, show } = command else {
        return Ok(());
    };
    if *init {
        Config::write_template(config_path)?;
        println!("Wrote config template to {}", config_path.display());
    }
    if *show || !*init {
        println!("{}", render_json(config)?);
    }
    Ok(())
}

fn resolve_selected_programs(
    config: &Config,
    cli_programs: Option<&str>,
) -> Result<Vec<ProgramId>> {
    if let Some(raw) = cli_programs {
        return parse_program_list(raw);
    }
    if config.programs.enabled.is_empty() {
        return Ok(ProgramId::ALL.to_vec());
    }
    let mut parsed = Vec::new();
    for entry in &config.programs.enabled {
        parsed.push(ProgramId::from_str(entry)?);
    }
    Ok(parsed)
}

fn parse_program_list(raw: &str) -> Result<Vec<ProgramId>> {
    let mut out = Vec::new();
    for piece in raw.split(',') {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(ProgramId::from_str(trimmed)?);
    }
    if out.is_empty() {
        return Err(anyhow!("program filter is empty"));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

async fn evaluate_selected_programs(
    registry: &ProgramRegistry,
    selected: &[ProgramId],
    metrics: &delegation_oracle::metrics::ValidatorMetrics,
) -> Result<(
    Vec<EligibilityResult>,
    Vec<delegation_oracle::criteria::CriteriaSet>,
    BTreeMap<ProgramId, f64>,
)> {
    let mut results = Vec::new();
    let mut criteria_sets = Vec::new();
    let mut estimate_by_program = BTreeMap::new();
    for id in selected {
        let Some(program) = registry.by_id(*id) else {
            warn!("program not found in registry: {id}");
            continue;
        };
        let criteria = program.fetch_criteria().await?;
        let estimate = program
            .estimate_delegation(metrics, &criteria)
            .unwrap_or(0.0);
        let result = program.evaluate(metrics, &criteria);
        estimate_by_program.insert(*id, estimate);
        criteria_sets.push(criteria);
        results.push(result);
    }
    Ok((results, criteria_sets, estimate_by_program))
}

fn persist_eligibility_history(
    store: &SnapshotStore,
    vote_pubkey: &str,
    results: &[EligibilityResult],
) -> Result<()> {
    let epoch = store.next_epoch_hint()?;
    for result in results {
        let record = record_from_result(vote_pubkey.to_string(), epoch, result);
        store.insert_eligibility_record(&record)?;
    }
    Ok(())
}

async fn run_drift_detection(
    registry: &ProgramRegistry,
    store: &SnapshotStore,
    selected: &[ProgramId],
    your_metrics: &delegation_oracle::metrics::ValidatorMetrics,
) -> Result<Vec<CriteriaDrift>> {
    let mut drifts = Vec::new();
    for id in selected {
        let Some(program) = registry.by_id(*id) else {
            continue;
        };
        let new_set = program.fetch_criteria().await?;
        let old_set = store.latest_criteria(*id)?;
        if let Some(old) = old_set {
            let before = evaluate_validator(
                *id,
                your_metrics,
                &old,
                program.estimate_delegation(your_metrics, &old),
            );
            let after = evaluate_validator(
                *id,
                your_metrics,
                &new_set,
                program.estimate_delegation(your_metrics, &new_set),
            );
            if let Some(drift) = build_drift_report(&old, &new_set, Some(&before), Some(&after)) {
                drifts.push(drift);
            }
        }
        store.insert_criteria(&new_set)?;
    }
    Ok(drifts)
}

async fn run_watch_loop(
    registry: &ProgramRegistry,
    store: &SnapshotStore,
    config: &Config,
    selected: &[ProgramId],
    vote_pubkey: &str,
    rpc_url: &str,
    metric_overrides: &MetricOverrides,
    interval_secs: u64,
    vulnerability_interval_secs: Option<u64>,
    drift_interval_secs: Option<u64>,
    iterations: u32,
) -> Result<()> {
    let mut previous_results: Option<Vec<EligibilityResult>> = None;
    let mut sinks: Vec<Box<dyn AlertSink>> = Vec::new();
    if config.alerts.enable_stdout {
        sinks.push(Box::new(StdoutSink));
    }
    if !config.alerts.discord_webhook.trim().is_empty() {
        sinks.push(Box::new(WebhookSink::new(
            config.alerts.discord_webhook.clone(),
        )));
    }

    let status_interval = Duration::from_secs(interval_secs.max(1));
    let vulnerability_interval = Duration::from_secs(
        vulnerability_interval_secs.unwrap_or_else(|| interval_secs.saturating_mul(5).max(60)),
    );
    let default_drift_secs = u64::from(config.analysis.drift_check_interval_hours)
        .saturating_mul(3600)
        .max(interval_secs.max(60));
    let drift_interval = Duration::from_secs(drift_interval_secs.unwrap_or(default_drift_secs));

    let mut last_vulnerability_scan: Option<Instant> = None;
    let mut last_drift_scan: Option<Instant> = None;

    let total_iterations = iterations.max(1);
    for i in 0..total_iterations {
        info!("watch iteration {}", i + 1);
        let mut live_metrics =
            collect_validator_metrics(Some(vote_pubkey), rpc_url, metric_overrides).await?;
        normalize_metrics(&mut live_metrics);

        let (results, criteria_sets, _) =
            evaluate_selected_programs(registry, selected, &live_metrics).await?;

        let now = Instant::now();
        let should_run_vulnerability = last_vulnerability_scan
            .map(|last| now.duration_since(last) >= vulnerability_interval)
            .unwrap_or(true);
        let vulnerabilities = if should_run_vulnerability {
            last_vulnerability_scan = Some(now);
            let competitors = sample_competitors(&live_metrics);
            let mut scan = Vec::new();
            for set in &criteria_sets {
                scan.extend(analyze_vulnerabilities(
                    set.program,
                    set,
                    &competitors,
                    config.analysis.vulnerability_margin_pct,
                ));
            }
            scan
        } else {
            Vec::new()
        };

        let should_run_drift = last_drift_scan
            .map(|last| now.duration_since(last) >= drift_interval)
            .unwrap_or(true);
        let drifts = if should_run_drift {
            last_drift_scan = Some(now);
            run_drift_detection(registry, store, selected, &live_metrics).await?
        } else {
            Vec::new()
        };

        let alerts = evaluate_alerts(
            previous_results.as_deref(),
            &results,
            &drifts,
            &vulnerabilities,
        );
        let alerts = apply_alert_rules(alerts, config);
        for alert in &alerts {
            for sink in &sinks {
                if let Err(err) = sink.send(alert).await {
                    warn!("failed sending alert: {err}");
                }
            }
        }
        persist_eligibility_history(store, &live_metrics.vote_pubkey, &results)?;
        previous_results = Some(results);

        if i + 1 < total_iterations {
            tokio::time::sleep(status_interval).await;
        }
    }
    Ok(())
}

fn apply_alert_rules(
    alerts: Vec<delegation_oracle::alert::engine::AlertEvent>,
    config: &Config,
) -> Vec<delegation_oracle::alert::engine::AlertEvent> {
    alerts
        .into_iter()
        .filter(|event| match event.kind {
            AlertEventKind::CriteriaDrift => config.alerts.rules.criteria_drift,
            AlertEventKind::VulnerabilityDetected => config.alerts.rules.vulnerability_detected,
            AlertEventKind::EligibilityLost => config.alerts.rules.eligibility_lost,
            AlertEventKind::EligibilityGained => config.alerts.rules.eligibility_gained,
        })
        .collect()
}

fn print_status(results: &[EligibilityResult], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_status_table(results)),
        OutputFormat::Json => println!("{}", render_json(results)?),
        OutputFormat::Csv => println!("{}", status_to_csv(results)?),
    }
    Ok(())
}

fn print_gaps(results: &[EligibilityResult], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_gaps_table(results)),
        OutputFormat::Json => println!("{}", render_json(results)?),
        OutputFormat::Csv => {
            warn!("CSV output for gaps not implemented, using JSON");
            println!("{}", render_json(results)?);
        }
    }
    Ok(())
}

fn print_arbitrage(opps: &[ArbitrageOpportunity], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_arbitrage_table(opps)),
        OutputFormat::Json => println!("{}", render_json(opps)?),
        OutputFormat::Csv => println!("{}", arbitrage_to_csv(opps)?),
    }
    Ok(())
}

fn print_whatif(result: &WhatIfResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_whatif_table(result)),
        OutputFormat::Json => println!("{}", render_json(result)?),
        OutputFormat::Csv => {
            warn!("CSV output for whatif not implemented, using JSON");
            println!("{}", render_json(result)?);
        }
    }
    Ok(())
}

fn print_vulnerable(items: &[VulnerableValidator], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_vulnerability_table(items)),
        OutputFormat::Json => println!("{}", render_json(items)?),
        OutputFormat::Csv => {
            warn!("CSV output for vulnerable not implemented, using JSON");
            println!("{}", render_json(items)?);
        }
    }
    Ok(())
}

fn print_drift(drifts: &[CriteriaDrift], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_drift_table(drifts)),
        OutputFormat::Json => println!("{}", render_json(drifts)?),
        OutputFormat::Csv => {
            warn!("CSV output for drift not implemented, using JSON");
            println!("{}", render_json(drifts)?);
        }
    }
    Ok(())
}

fn print_recommendations(
    recommendations: &[OptimizationRecommendation],
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Table => println!("{}", render_recommendations_table(recommendations)),
        OutputFormat::Json => println!("{}", render_json(recommendations)?),
        OutputFormat::Csv => {
            warn!("CSV output for optimize not implemented, using JSON");
            println!("{}", render_json(recommendations)?);
        }
    }
    Ok(())
}

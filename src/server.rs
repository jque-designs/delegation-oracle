use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::alert::engine::{evaluate_alerts, AlertEvent};
use crate::alert::rules::AlertEventKind;
use crate::config::Config;
use crate::criteria::{build_drift_report, CriteriaDrift, CriteriaSet, MetricKey, ProgramId};
use crate::eligibility::arbitrage::build_arbitrage_opportunities;
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::history::{record_from_result, summarize_timeline};
use crate::eligibility::vulnerability::analyze_vulnerabilities;
use crate::eligibility::{
    ArbitrageOpportunity, EligibilityRecord, EligibilityResult, VulnerableValidator,
};
use crate::metrics::collector::{collect_validator_metrics, sample_competitors, MetricOverrides};
use crate::metrics::normalize::normalize_metrics;
use crate::optimizer::conflicts::detect_conflicts;
use crate::optimizer::recommendations::build_recommendations;
use crate::optimizer::whatif::simulate_whatif;
use crate::optimizer::{MetricChange, OptimizationRecommendation, WhatIfResult};
use crate::programs::ProgramRegistry;
use crate::snapshot::store::SnapshotStore;

#[derive(Clone)]
struct ApiState {
    config: Config,
    registry: ProgramRegistry,
    db_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    data: T,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    ok: bool,
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ApiErrorBody {
            ok: false,
            error: self.message,
        });
        (self.status, body).into_response()
    }
}

type ApiResult<T> = std::result::Result<Json<ApiResponse<T>>, ApiError>;

#[derive(Debug, Clone, Default, Deserialize)]
struct CommandContextRequest {
    validator: Option<String>,
    rpc_url: Option<String>,
    programs: Option<Vec<String>>,
    #[serde(default)]
    metrics: MetricOverrides,
}

#[derive(Debug, Clone)]
struct EffectiveContext {
    vote_pubkey: String,
    rpc_url: String,
    programs: Vec<ProgramId>,
    metrics: MetricOverrides,
}

#[derive(Debug, Clone, Deserialize)]
struct StatusRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    #[serde(default = "default_true")]
    persist_history: bool,
}

impl Default for StatusRequest {
    fn default() -> Self {
        Self {
            context: CommandContextRequest::default(),
            persist_history: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct GapsRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ArbitrageRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    sort: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricChangeInput {
    metric: String,
    to: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct WhatIfRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    #[serde(default)]
    changes: Vec<MetricChangeInput>,
    commission: Option<f64>,
    #[serde(rename = "skip_rate")]
    skip_rate: Option<f64>,
    #[serde(rename = "mev_commission")]
    mev_commission: Option<f64>,
    #[serde(rename = "activated_stake")]
    activated_stake: Option<f64>,
    #[serde(rename = "uptime_percent")]
    uptime_percent: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct VulnerableRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    program: Option<String>,
    margin: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DriftRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct HistoryRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    epochs: Option<usize>,
    program: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OptimizeRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    top: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct WatchRequest {
    #[serde(flatten)]
    context: CommandContextRequest,
    interval_secs: Option<u64>,
    vulnerability_interval_secs: Option<u64>,
    drift_interval_secs: Option<u64>,
    iterations: Option<u32>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    validator: String,
    results: Vec<EligibilityResult>,
}

#[derive(Debug, Serialize)]
struct ArbitrageResponse {
    opportunities: Vec<ArbitrageOpportunity>,
}

#[derive(Debug, Serialize)]
struct VulnerableResponse {
    vulnerable_validators: Vec<VulnerableValidator>,
}

#[derive(Debug, Serialize)]
struct DriftResponse {
    drifts: Vec<CriteriaDrift>,
}

#[derive(Debug, Serialize)]
struct HistoryResponse {
    summary: String,
    records: Vec<EligibilityRecord>,
}

#[derive(Debug, Serialize)]
struct OptimizeResponse {
    recommendations: Vec<OptimizationRecommendation>,
}

#[derive(Debug, Serialize)]
struct WatchIteration {
    iteration: u32,
    results: Vec<EligibilityResult>,
    drifts: Vec<CriteriaDrift>,
    vulnerabilities: Vec<VulnerableValidator>,
    alerts: Vec<AlertEvent>,
}

#[derive(Debug, Serialize)]
struct WatchResponse {
    iterations: Vec<WatchIteration>,
}

pub async fn run_server(config: Config, bind: SocketAddr) -> Result<()> {
    let state = ApiState {
        db_path: config.resolved_db_path(),
        config,
        registry: ProgramRegistry::with_defaults(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/status", post(status))
        .route("/v1/gaps", post(gaps))
        .route("/v1/arbitrage", post(arbitrage))
        .route("/v1/whatif", post(whatif))
        .route("/v1/vulnerable", post(vulnerable))
        .route("/v1/drift", post(drift))
        .route("/v1/history", post(history))
        .route("/v1/optimize", post(optimize))
        .route("/v1/watch", post(watch))
        .route("/v1/config", get(show_config))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    info!("REST API listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<ApiResponse<HealthResponse>> {
    ok(HealthResponse { status: "ok" })
}

async fn show_config(State(state): State<ApiState>) -> Json<ApiResponse<Config>> {
    ok(state.config)
}

async fn status(
    State(state): State<ApiState>,
    Json(request): Json<StatusRequest>,
) -> ApiResult<StatusResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let metrics = collect_metrics(&effective).await?;
    let (results, _, _) =
        evaluate_selected_programs(&state.registry, &effective.programs, &metrics).await?;

    if request.persist_history {
        let store = open_store(&state)?;
        persist_eligibility_history(&store, &metrics.vote_pubkey, &results)
            .map_err(ApiError::internal)?;
    }

    Ok(ok(StatusResponse {
        validator: metrics.vote_pubkey,
        results,
    }))
}

async fn gaps(
    State(state): State<ApiState>,
    Json(request): Json<GapsRequest>,
) -> ApiResult<StatusResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let metrics = collect_metrics(&effective).await?;
    let (results, _, _) =
        evaluate_selected_programs(&state.registry, &effective.programs, &metrics).await?;

    Ok(ok(StatusResponse {
        validator: metrics.vote_pubkey,
        results,
    }))
}

async fn arbitrage(
    State(state): State<ApiState>,
    Json(request): Json<ArbitrageRequest>,
) -> ApiResult<ArbitrageResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let metrics = collect_metrics(&effective).await?;
    let (results, _, estimate_by_program) =
        evaluate_selected_programs(&state.registry, &effective.programs, &metrics).await?;
    let mut opportunities = build_arbitrage_opportunities(&results, &estimate_by_program);
    if request
        .sort
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("effort"))
        .unwrap_or(false)
    {
        opportunities.sort_by(|a, b| a.total_effort.cmp(&b.total_effort));
    }

    Ok(ok(ArbitrageResponse { opportunities }))
}

async fn whatif(
    State(state): State<ApiState>,
    Json(request): Json<WhatIfRequest>,
) -> ApiResult<WhatIfResult> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let metrics = collect_metrics(&effective).await?;

    let mut changes = request
        .changes
        .into_iter()
        .map(|change| {
            MetricKey::from_str(&change.metric)
                .map(|metric| (metric, change.to))
                .map_err(|error| ApiError::bad_request(error.to_string()))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if let Some(value) = request.commission {
        changes.push((MetricKey::Commission, value));
    }
    if let Some(value) = request.skip_rate {
        changes.push((MetricKey::SkipRate, value));
    }
    if let Some(value) = request.mev_commission {
        changes.push((MetricKey::MevCommission, value));
    }
    if let Some(value) = request.activated_stake {
        changes.push((MetricKey::ActivatedStake, value));
    }
    if let Some(value) = request.uptime_percent {
        changes.push((MetricKey::UptimePercent, value));
    }
    if changes.is_empty() {
        return Err(ApiError::bad_request(
            "at least one metric change is required",
        ));
    }
    dedupe_metric_changes(&mut changes);

    let result = simulate_whatif(
        &state.registry,
        &metrics,
        &changes,
        Some(effective.programs.as_slice()),
    )
    .await
    .map_err(ApiError::internal)?;
    Ok(ok(result))
}

async fn vulnerable(
    State(state): State<ApiState>,
    Json(request): Json<VulnerableRequest>,
) -> ApiResult<VulnerableResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let metrics = collect_metrics(&effective).await?;
    let margin = request
        .margin
        .unwrap_or(state.config.analysis.vulnerability_margin_pct)
        .max(0.1);

    let target_programs = if let Some(program) = request.program {
        vec![ProgramId::from_str(&program).map_err(|e| ApiError::bad_request(e.to_string()))?]
    } else {
        effective.programs.clone()
    };

    let competitors = sample_competitors(&metrics);
    let mut vulnerable_validators = Vec::new();
    for program_id in &target_programs {
        let Some(program_impl) = state.registry.by_id(*program_id) else {
            continue;
        };
        let criteria = program_impl
            .fetch_criteria()
            .await
            .map_err(ApiError::internal)?;
        vulnerable_validators.extend(analyze_vulnerabilities(
            *program_id,
            &criteria,
            &competitors,
            margin,
        ));
    }

    Ok(ok(VulnerableResponse {
        vulnerable_validators,
    }))
}

async fn drift(
    State(state): State<ApiState>,
    Json(request): Json<DriftRequest>,
) -> ApiResult<DriftResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let metrics = collect_metrics(&effective).await?;
    let store = open_store(&state)?;
    let drifts = run_drift_detection(&state.registry, &store, &effective.programs, &metrics)
        .await
        .map_err(ApiError::internal)?;
    Ok(ok(DriftResponse { drifts }))
}

async fn history(
    State(state): State<ApiState>,
    Json(request): Json<HistoryRequest>,
) -> ApiResult<HistoryResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let vote_pubkey = if effective.vote_pubkey.trim().is_empty() {
        "DemoVote11111111111111111111111111111111111".to_string()
    } else {
        effective.vote_pubkey.clone()
    };
    let epochs = request.epochs.unwrap_or(50).max(1);
    let program_filter = request
        .program
        .as_deref()
        .map(ProgramId::from_str)
        .transpose()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let store = open_store(&state)?;
    let records = store
        .load_history(&vote_pubkey, program_filter, epochs)
        .map_err(ApiError::internal)?;
    let summary = summarize_timeline(&records, program_filter);

    Ok(ok(HistoryResponse { summary, records }))
}

async fn optimize(
    State(state): State<ApiState>,
    Json(request): Json<OptimizeRequest>,
) -> ApiResult<OptimizeResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let top = request.top.unwrap_or(5).max(1);
    let metrics = collect_metrics(&effective).await?;
    let (results, criteria_sets, estimate_by_program) =
        evaluate_selected_programs(&state.registry, &effective.programs, &metrics).await?;
    let opportunities = build_arbitrage_opportunities(&results, &estimate_by_program);
    let conflicts = detect_conflicts(&criteria_sets);
    let recommendations = build_recommendations(&opportunities, &conflicts, top);

    Ok(ok(OptimizeResponse { recommendations }))
}

async fn watch(
    State(state): State<ApiState>,
    Json(request): Json<WatchRequest>,
) -> ApiResult<WatchResponse> {
    let effective = resolve_effective_context(&state, &request.context)?;
    let iterations = request.iterations.unwrap_or(1).clamp(1, 100);
    let status_interval = Duration::from_secs(request.interval_secs.unwrap_or(60).max(1));
    let vulnerability_interval = Duration::from_secs(
        request
            .vulnerability_interval_secs
            .unwrap_or(status_interval.as_secs() * 5)
            .max(1),
    );
    let default_drift_interval = u64::from(state.config.analysis.drift_check_interval_hours)
        .saturating_mul(3600)
        .max(status_interval.as_secs());
    let drift_interval = Duration::from_secs(
        request
            .drift_interval_secs
            .unwrap_or(default_drift_interval)
            .max(1),
    );

    let store = open_store(&state)?;
    let mut run_results = Vec::new();
    let mut previous_results: Option<Vec<EligibilityResult>> = None;
    let mut last_vulnerability_scan: Option<Instant> = None;
    let mut last_drift_scan: Option<Instant> = None;

    for iteration in 0..iterations {
        let mut live_metrics = collect_metrics(&effective).await?;
        normalize_metrics(&mut live_metrics);

        let (results, criteria_sets, _) =
            evaluate_selected_programs(&state.registry, &effective.programs, &live_metrics).await?;
        persist_eligibility_history(&store, &live_metrics.vote_pubkey, &results)
            .map_err(ApiError::internal)?;

        let now = Instant::now();
        let run_vulnerability = last_vulnerability_scan
            .map(|last| now.duration_since(last) >= vulnerability_interval)
            .unwrap_or(true);
        let vulnerabilities = if run_vulnerability {
            last_vulnerability_scan = Some(now);
            let competitors = sample_competitors(&live_metrics);
            let mut out = Vec::new();
            for criteria in &criteria_sets {
                out.extend(analyze_vulnerabilities(
                    criteria.program,
                    criteria,
                    &competitors,
                    state.config.analysis.vulnerability_margin_pct,
                ));
            }
            out
        } else {
            Vec::new()
        };

        let run_drift = last_drift_scan
            .map(|last| now.duration_since(last) >= drift_interval)
            .unwrap_or(true);
        let drifts = if run_drift {
            last_drift_scan = Some(now);
            run_drift_detection(&state.registry, &store, &effective.programs, &live_metrics)
                .await
                .map_err(ApiError::internal)?
        } else {
            Vec::new()
        };

        let alerts = apply_alert_rules(
            evaluate_alerts(
                previous_results.as_deref(),
                &results,
                &drifts,
                &vulnerabilities,
            ),
            &state.config,
        );

        run_results.push(WatchIteration {
            iteration: iteration + 1,
            results: results.clone(),
            drifts,
            vulnerabilities,
            alerts,
        });

        previous_results = Some(results);
        if iteration + 1 < iterations {
            tokio::time::sleep(status_interval).await;
        }
    }

    Ok(ok(WatchResponse {
        iterations: run_results,
    }))
}

fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse { ok: true, data })
}

fn default_true() -> bool {
    true
}

fn open_store(state: &ApiState) -> std::result::Result<SnapshotStore, ApiError> {
    SnapshotStore::open(&state.db_path).map_err(ApiError::internal)
}

fn resolve_effective_context(
    state: &ApiState,
    context: &CommandContextRequest,
) -> std::result::Result<EffectiveContext, ApiError> {
    let vote_pubkey = context
        .validator
        .clone()
        .unwrap_or_else(|| state.config.validator.vote_pubkey.clone());
    let rpc_url = context
        .rpc_url
        .clone()
        .unwrap_or_else(|| state.config.rpc.url.clone());

    let program_names = if let Some(programs) = &context.programs {
        programs.clone()
    } else if !state.config.programs.enabled.is_empty() {
        state.config.programs.enabled.clone()
    } else {
        ProgramId::ALL
            .iter()
            .map(|id| id.as_slug().to_string())
            .collect::<Vec<_>>()
    };
    let programs = parse_programs(&program_names)?;

    Ok(EffectiveContext {
        vote_pubkey,
        rpc_url,
        programs,
        metrics: context.metrics.clone(),
    })
}

fn parse_programs(raw_programs: &[String]) -> std::result::Result<Vec<ProgramId>, ApiError> {
    let mut parsed = Vec::new();
    for program in raw_programs {
        parsed.push(
            ProgramId::from_str(program)
                .map_err(|error| ApiError::bad_request(error.to_string()))?,
        );
    }
    if parsed.is_empty() {
        return Err(ApiError::bad_request("program list cannot be empty"));
    }
    parsed.sort();
    parsed.dedup();
    Ok(parsed)
}

async fn collect_metrics(
    context: &EffectiveContext,
) -> std::result::Result<crate::metrics::ValidatorMetrics, ApiError> {
    let vote_arg = if context.vote_pubkey.trim().is_empty() {
        None
    } else {
        Some(context.vote_pubkey.as_str())
    };
    let mut metrics = collect_validator_metrics(vote_arg, &context.rpc_url, &context.metrics)
        .await
        .map_err(ApiError::internal)?;
    normalize_metrics(&mut metrics);
    Ok(metrics)
}

async fn evaluate_selected_programs(
    registry: &ProgramRegistry,
    selected: &[ProgramId],
    metrics: &crate::metrics::ValidatorMetrics,
) -> std::result::Result<
    (
        Vec<EligibilityResult>,
        Vec<CriteriaSet>,
        BTreeMap<ProgramId, f64>,
    ),
    ApiError,
> {
    let mut results = Vec::new();
    let mut criteria_sets = Vec::new();
    let mut estimate_by_program = BTreeMap::new();
    for id in selected {
        let Some(program) = registry.by_id(*id) else {
            warn!("program not found in registry: {id}");
            continue;
        };
        let criteria = program.fetch_criteria().await.map_err(ApiError::internal)?;
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
    your_metrics: &crate::metrics::ValidatorMetrics,
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

fn dedupe_metric_changes(changes: &mut Vec<(MetricKey, f64)>) {
    let mut seen = BTreeMap::new();
    for (metric, to) in changes.iter() {
        seen.insert(metric.clone(), *to);
    }
    let deduped = seen
        .into_iter()
        .map(|(metric, to)| (metric, to))
        .collect::<Vec<_>>();
    *changes = deduped;
}

fn apply_alert_rules(alerts: Vec<AlertEvent>, config: &Config) -> Vec<AlertEvent> {
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

#[cfg(test)]
mod tests {
    use super::parse_programs;

    #[test]
    fn parses_program_names() {
        let programs = parse_programs(&["sfdp".to_string(), "jito".to_string()])
            .expect("failed to parse programs");
        assert_eq!(programs.len(), 2);
    }
}

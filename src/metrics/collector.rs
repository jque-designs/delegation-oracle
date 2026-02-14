use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::metrics::ValidatorMetrics;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricOverrides {
    pub commission: Option<f64>,
    pub activated_stake: Option<f64>,
    pub skip_rate: Option<f64>,
    pub vote_credits: Option<f64>,
    pub uptime_percent: Option<f64>,
    pub solana_version: Option<String>,
    pub datacenter_concentration: Option<f64>,
    pub superminority_status: Option<bool>,
    pub mev_commission: Option<f64>,
    pub stake_concentration: Option<f64>,
    pub infrastructure_diversity: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CompetitorSnapshot {
    pub metrics: ValidatorMetrics,
    pub previous_metrics: Option<ValidatorMetrics>,
    pub current_delegation_sol: f64,
}

pub async fn collect_validator_metrics(
    vote_pubkey: Option<&str>,
    _rpc_url: &str,
    overrides: &MetricOverrides,
) -> Result<ValidatorMetrics> {
    let mut metrics = ValidatorMetrics::sample(
        vote_pubkey.unwrap_or("DemoVote11111111111111111111111111111111111"),
    );
    apply_overrides(&mut metrics, overrides);
    Ok(metrics)
}

pub fn apply_overrides(metrics: &mut ValidatorMetrics, overrides: &MetricOverrides) {
    if let Some(v) = overrides.commission {
        metrics.commission = v;
    }
    if let Some(v) = overrides.activated_stake {
        metrics.activated_stake = v;
    }
    if let Some(v) = overrides.skip_rate {
        metrics.skip_rate = v;
    }
    if let Some(v) = overrides.vote_credits {
        metrics.vote_credits = v;
    }
    if let Some(v) = overrides.uptime_percent {
        metrics.uptime_percent = v;
    }
    if let Some(v) = &overrides.solana_version {
        metrics.solana_version = v.clone();
    }
    if let Some(v) = overrides.datacenter_concentration {
        metrics.datacenter_concentration = v;
    }
    if let Some(v) = overrides.superminority_status {
        metrics.superminority_status = v;
    }
    if let Some(v) = overrides.mev_commission {
        metrics.mev_commission = v;
    }
    if let Some(v) = overrides.stake_concentration {
        metrics.stake_concentration = v;
    }
    if let Some(v) = overrides.infrastructure_diversity {
        metrics.infrastructure_diversity = v;
    }
}

pub fn sample_competitors(base: &ValidatorMetrics) -> Vec<CompetitorSnapshot> {
    let mut out = Vec::new();
    for idx in 0..8u32 {
        let mut metrics = base.clone();
        metrics.vote_pubkey = format!("CompetitorVote{idx:02}");
        metrics.skip_rate += (idx as f64) * 0.25;
        metrics.commission += ((idx % 3) as f64) * 0.5;
        metrics.mev_commission += ((idx % 4) as f64) * 0.6;
        metrics.uptime_percent -= (idx as f64) * 0.15;
        metrics.activated_stake += (idx as f64) * 8_000.0;

        let mut previous = metrics.clone();
        previous.skip_rate -= 0.15;
        previous.uptime_percent += 0.1;

        out.push(CompetitorSnapshot {
            metrics,
            previous_metrics: Some(previous),
            current_delegation_sol: 7_500.0 + (idx as f64) * 2_500.0,
        });
    }
    out
}

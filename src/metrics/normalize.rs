use crate::metrics::ValidatorMetrics;

pub fn normalize_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

pub fn normalize_ratio(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

pub fn normalize_metrics(metrics: &mut ValidatorMetrics) {
    metrics.commission = normalize_percent(metrics.commission);
    metrics.skip_rate = normalize_percent(metrics.skip_rate);
    metrics.vote_credits = normalize_percent(metrics.vote_credits);
    metrics.uptime_percent = normalize_percent(metrics.uptime_percent);
    metrics.datacenter_concentration = normalize_percent(metrics.datacenter_concentration);
    metrics.mev_commission = normalize_percent(metrics.mev_commission);
    metrics.stake_concentration = normalize_ratio(metrics.stake_concentration);
    metrics.infrastructure_diversity = normalize_ratio(metrics.infrastructure_diversity);
    metrics.activated_stake = metrics.activated_stake.max(0.0);
}

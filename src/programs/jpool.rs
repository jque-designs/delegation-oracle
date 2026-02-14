use anyhow::Result;
use async_trait::async_trait;
use tracing::debug;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::http::{
    collect_numeric_samples, fetch_json, lamports_to_sol_if_needed, parse_eligible_validators,
    percentile, sha256_json,
};
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct JPoolProgram;

const JPOOL_VALIDATORS_URL: &str = "https://api.jpool.one/validators";
const MAX_ELIGIBLE_ITEMS: usize = 180;

#[async_trait]
impl DelegationProgram for JPoolProgram {
    fn id(&self) -> ProgramId {
        ProgramId::JPool
    }

    fn name(&self) -> &str {
        "JPool"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        let mut criteria = default_criteria();
        let mut external_hash = None;

        match fetch_json(JPOOL_VALIDATORS_URL).await {
            Ok(payload) => {
                external_hash = Some(sha256_json(&payload));

                let commission_values = collect_numeric_samples(&payload, &["commission"], 2_500);
                if let Some(max_commission) = percentile(&commission_values, 0.70) {
                    set_max(
                        &mut criteria,
                        &MetricKey::Commission,
                        max_commission.clamp(5.0, 12.0),
                    );
                }

                let uptime_values = collect_numeric_samples(&payload, &["uptime"], 2_500);
                if let Some(min_uptime) = percentile(&uptime_values, 0.20) {
                    set_min(
                        &mut criteria,
                        &MetricKey::UptimePercent,
                        min_uptime.clamp(95.0, 99.9),
                    );
                }

                let activated_stake =
                    collect_numeric_samples(&payload, &["activated_stake", "active_stake"], 2_500);
                if let Some(min_stake_lamports) = percentile(&activated_stake, 0.08) {
                    let min_stake_sol = lamports_to_sol_if_needed(min_stake_lamports);
                    set_min(
                        &mut criteria,
                        &MetricKey::ActivatedStake,
                        min_stake_sol.clamp(10_000.0, 150_000.0),
                    );
                }
            }
            Err(error) => debug!("jpool criteria fetch failed, using fallback: {error}"),
        }

        let mut set = CriteriaSet::with_hash(self.id(), JPOOL_VALIDATORS_URL, criteria);
        if let Some(hash) = external_hash {
            set.raw_hash = hash;
        }
        Ok(set)
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        if let Ok(payload) = fetch_json(JPOOL_VALIDATORS_URL).await {
            let mut parsed = parse_eligible_validators(
                &payload,
                &["vote_account", "voteAccount", "vote_pubkey"],
                &["jpool_score", "score", "wiz_score", "va_score"],
                &["jpool_stake", "activated_stake", "active_stake"],
                MAX_ELIGIBLE_ITEMS,
            )
            .into_iter()
            .filter_map(|mut v| {
                v.delegated_sol = v.delegated_sol.map(lamports_to_sol_if_needed);
                if v.delegated_sol.unwrap_or(0.0) > 0.0 {
                    Some(v)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
            if !parsed.is_empty() {
                parsed.sort_by(|a, b| {
                    b.delegated_sol
                        .unwrap_or(0.0)
                        .total_cmp(&a.delegated_sol.unwrap_or(0.0))
                });
                return Ok(parsed);
            }
        }

        Ok(fallback_eligible_set())
    }

    fn evaluate(&self, validator: &ValidatorMetrics, criteria: &CriteriaSet) -> EligibilityResult {
        evaluate_validator(
            self.id(),
            validator,
            criteria,
            self.estimate_delegation(validator, criteria),
        )
    }

    fn estimate_delegation(
        &self,
        validator: &ValidatorMetrics,
        _criteria: &CriteriaSet,
    ) -> Option<f64> {
        let base = 8_000.0;
        let performance = (validator.uptime_percent + validator.vote_credits) * 55.0;
        Some(base + performance)
    }
}

fn default_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: "Commission".to_string(),
            metric: MetricKey::Commission,
            constraint: Constraint::Max(10.0),
            weight: Some(1.2),
            description: "Fee competitiveness".to_string(),
        },
        Criterion {
            name: "Skip rate".to_string(),
            metric: MetricKey::SkipRate,
            constraint: Constraint::Max(3.5),
            weight: Some(1.8),
            description: "Reliability signal".to_string(),
        },
        Criterion {
            name: "Uptime".to_string(),
            metric: MetricKey::UptimePercent,
            constraint: Constraint::Min(98.0),
            weight: Some(1.8),
            description: "Operational availability".to_string(),
        },
        Criterion {
            name: "Vote credits".to_string(),
            metric: MetricKey::VoteCredits,
            constraint: Constraint::Min(97.0),
            weight: Some(1.6),
            description: "Consensus participation".to_string(),
        },
        Criterion {
            name: "Min stake".to_string(),
            metric: MetricKey::ActivatedStake,
            constraint: Constraint::Min(25_000.0),
            weight: Some(0.8),
            description: "Small viability floor".to_string(),
        },
    ]
}

fn fallback_eligible_set() -> Vec<EligibleValidator> {
    vec![
        EligibleValidator {
            vote_pubkey: "JpoolSet01".to_string(),
            score: Some(0.8),
            delegated_sol: Some(14_200.0),
        },
        EligibleValidator {
            vote_pubkey: "JpoolSet02".to_string(),
            score: Some(0.77),
            delegated_sol: Some(12_900.0),
        },
    ]
}

fn set_max(criteria: &mut [Criterion], metric: &MetricKey, value: f64) {
    for criterion in criteria {
        if &criterion.metric == metric {
            criterion.constraint = Constraint::Max(value);
            break;
        }
    }
}

fn set_min(criteria: &mut [Criterion], metric: &MetricKey, value: f64) {
    for criterion in criteria {
        if &criterion.metric == metric {
            criterion.constraint = Constraint::Min(value);
            break;
        }
    }
}

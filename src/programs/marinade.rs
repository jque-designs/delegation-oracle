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
pub struct MarinadeProgram;

const MARINADE_VALIDATORS_URL: &str = "https://validators-api.marinade.finance/validators";
const MAX_ELIGIBLE_ITEMS: usize = 160;

#[async_trait]
impl DelegationProgram for MarinadeProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Marinade
    }

    fn name(&self) -> &str {
        "Marinade"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        let mut criteria = default_criteria();
        let mut external_hash = None;

        match fetch_json(MARINADE_VALIDATORS_URL).await {
            Ok(payload) => {
                external_hash = Some(sha256_json(&payload));

                let commission_values = collect_numeric_samples(
                    &payload,
                    &[
                        "commission_effective",
                        "commission_advertised",
                        "commission_aggregated",
                    ],
                    2_000,
                );
                if let Some(max_commission) = percentile(&commission_values, 0.70) {
                    let normalized = max_commission.clamp(5.0, 12.0);
                    set_max(&mut criteria, &MetricKey::Commission, normalized);
                }

                let uptime_values = collect_numeric_samples(&payload, &["avg_uptime_pct"], 2_000);
                if let Some(min_uptime) = percentile(&uptime_values, 0.20) {
                    let normalized = if min_uptime <= 1.0 {
                        (min_uptime * 100.0).clamp(95.0, 99.9)
                    } else {
                        min_uptime.clamp(95.0, 99.9)
                    };
                    set_min(&mut criteria, &MetricKey::UptimePercent, normalized);
                }
            }
            Err(error) => {
                debug!("marinade criteria fetch failed, using fallback: {error}");
            }
        }

        let mut set = CriteriaSet::with_hash(self.id(), MARINADE_VALIDATORS_URL, criteria);
        if let Some(hash) = external_hash {
            set.raw_hash = hash;
        }
        Ok(set)
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        if let Ok(payload) = fetch_json(MARINADE_VALIDATORS_URL).await {
            let mut parsed = parse_eligible_validators(
                &payload,
                &["vote_account", "voteAccount", "vote_pubkey"],
                &["score", "marinade_score", "validator_score"],
                &[
                    "marinade_native_stake",
                    "marinade_stake",
                    "activated_stake",
                    "active_stake",
                ],
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
        let performance = (validator.vote_credits + validator.uptime_percent) / 2.0;
        let commission_bonus = (10.0 - validator.commission).max(0.0) * 900.0;
        Some(18_000.0 + performance * 190.0 + commission_bonus)
    }
}

fn default_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: "Commission".to_string(),
            metric: MetricKey::Commission,
            constraint: Constraint::Max(8.0),
            weight: Some(1.4),
            description: "Competitive validator fees improve score".to_string(),
        },
        Criterion {
            name: "Skip rate".to_string(),
            metric: MetricKey::SkipRate,
            constraint: Constraint::Max(3.0),
            weight: Some(2.0),
            description: "Block production quality signal".to_string(),
        },
        Criterion {
            name: "Vote credits".to_string(),
            metric: MetricKey::VoteCredits,
            constraint: Constraint::Min(97.5),
            weight: Some(2.2),
            description: "Consensus participation score".to_string(),
        },
        Criterion {
            name: "Uptime".to_string(),
            metric: MetricKey::UptimePercent,
            constraint: Constraint::Min(98.5),
            weight: Some(1.8),
            description: "Runtime reliability baseline".to_string(),
        },
        Criterion {
            name: "Datacenter concentration".to_string(),
            metric: MetricKey::DatacenterConcentration,
            constraint: Constraint::Max(60.0),
            weight: Some(1.0),
            description: "Diversity pressure against centralization".to_string(),
        },
        Criterion {
            name: "Infrastructure diversity".to_string(),
            metric: MetricKey::InfrastructureDiversity,
            constraint: Constraint::Min(0.6),
            weight: Some(1.0),
            description: "Redundancy and independent failover".to_string(),
        },
    ]
}

fn fallback_eligible_set() -> Vec<EligibleValidator> {
    vec![
        EligibleValidator {
            vote_pubkey: "MarinadeSet01".to_string(),
            score: Some(0.88),
            delegated_sol: Some(41_000.0),
        },
        EligibleValidator {
            vote_pubkey: "MarinadeSet02".to_string(),
            score: Some(0.84),
            delegated_sol: Some(35_500.0),
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

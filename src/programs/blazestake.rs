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
pub struct BlazeStakeProgram;

const BLAZE_VALIDATORS_URL: &str = "https://stake.solblaze.org/api/v1/cls_validators";
const BLAZE_STATS_URL: &str = "https://stake.solblaze.org/api/v1/stats";
const MAX_ELIGIBLE_ITEMS: usize = 140;

#[async_trait]
impl DelegationProgram for BlazeStakeProgram {
    fn id(&self) -> ProgramId {
        ProgramId::BlazeStake
    }

    fn name(&self) -> &str {
        "BlazeStake"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        let mut criteria = default_criteria();
        let mut source = BLAZE_VALIDATORS_URL;
        let mut external_hash = None;

        match fetch_json(BLAZE_VALIDATORS_URL).await {
            Ok(payload) => {
                external_hash = Some(sha256_json(&payload));
                let commission_values = collect_numeric_samples(&payload, &["commission"], 1_500);
                if let Some(max_commission) = percentile(&commission_values, 0.65) {
                    set_max(
                        &mut criteria,
                        &MetricKey::Commission,
                        max_commission.clamp(4.0, 10.0),
                    );
                }
            }
            Err(error) => {
                debug!("blazestake validator endpoint unavailable, using stats fallback: {error}");
                if let Ok(stats_payload) = fetch_json(BLAZE_STATS_URL).await {
                    external_hash = Some(sha256_json(&stats_payload));
                    source = BLAZE_STATS_URL;
                }
            }
        }

        let mut set = CriteriaSet::with_hash(self.id(), source, criteria);
        if let Some(hash) = external_hash {
            set.raw_hash = hash;
        }
        Ok(set)
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        if let Ok(payload) = fetch_json(BLAZE_VALIDATORS_URL).await {
            let mut parsed = parse_eligible_validators(
                &payload,
                &["vote_account", "voteAccount", "vote_pubkey", "vote"],
                &["score", "blazestake_score", "blaze_score"],
                &["delegated_stake", "pool_stake", "stake", "activated_stake"],
                MAX_ELIGIBLE_ITEMS,
            )
            .into_iter()
            .map(|mut v| {
                v.delegated_sol = v.delegated_sol.map(lamports_to_sol_if_needed);
                v
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
        let skip_penalty = (validator.skip_rate * 400.0).min(2_000.0);
        Some((22_000.0 + performance * 120.0 - skip_penalty).max(0.0))
    }
}

fn default_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: "Commission".to_string(),
            metric: MetricKey::Commission,
            constraint: Constraint::Max(7.0),
            weight: Some(1.2),
            description: "Fees must remain low for pool delegators".to_string(),
        },
        Criterion {
            name: "Skip rate strict".to_string(),
            metric: MetricKey::SkipRate,
            constraint: Constraint::Max(2.5),
            weight: Some(2.2),
            description: "BlazeStake strongly prefers low skip-rate validators".to_string(),
        },
        Criterion {
            name: "Uptime".to_string(),
            metric: MetricKey::UptimePercent,
            constraint: Constraint::Min(98.5),
            weight: Some(1.9),
            description: "Reliability SLO for delegation set".to_string(),
        },
        Criterion {
            name: "Vote credits".to_string(),
            metric: MetricKey::VoteCredits,
            constraint: Constraint::Min(97.5),
            weight: Some(1.7),
            description: "Consensus efficiency".to_string(),
        },
        Criterion {
            name: "Datacenter concentration".to_string(),
            metric: MetricKey::DatacenterConcentration,
            constraint: Constraint::Max(50.0),
            weight: Some(1.0),
            description: "Concentration risk ceiling".to_string(),
        },
        Criterion {
            name: "Infrastructure diversity".to_string(),
            metric: MetricKey::InfrastructureDiversity,
            constraint: Constraint::Min(0.7),
            weight: Some(1.0),
            description: "Hardware/network redundancy expectation".to_string(),
        },
    ]
}

fn fallback_eligible_set() -> Vec<EligibleValidator> {
    vec![
        EligibleValidator {
            vote_pubkey: "BlazeSet01".to_string(),
            score: Some(0.79),
            delegated_sol: Some(16_200.0),
        },
        EligibleValidator {
            vote_pubkey: "BlazeSet02".to_string(),
            score: Some(0.75),
            delegated_sol: Some(14_900.0),
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

use anyhow::Result;
use async_trait::async_trait;
use tracing::debug;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::http::{
    bps_to_percent_if_needed, collect_numeric_samples, fetch_json, lamports_to_sol_if_needed,
    parse_eligible_validators, percentile, sha256_json,
};
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct JitoProgram;

const JITO_VALIDATORS_URL: &str = "https://kobe.mainnet.jito.network/api/v1/validators";
const MAX_ELIGIBLE_ITEMS: usize = 180;

#[async_trait]
impl DelegationProgram for JitoProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Jito
    }

    fn name(&self) -> &str {
        "Jito"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        let mut criteria = default_criteria();
        let mut external_hash = None;

        match fetch_json(JITO_VALIDATORS_URL).await {
            Ok(payload) => {
                external_hash = Some(sha256_json(&payload));

                let mev_bps = collect_numeric_samples(&payload, &["mev_commission_bps"], 2_000)
                    .into_iter()
                    .map(bps_to_percent_if_needed)
                    .collect::<Vec<_>>();
                if let Some(dynamic_cap) = percentile(&mev_bps, 0.80) {
                    set_max(
                        &mut criteria,
                        &MetricKey::MevCommission,
                        dynamic_cap.clamp(3.0, 10.0),
                    );
                }
            }
            Err(error) => debug!("jito criteria fetch failed, using fallback: {error}"),
        }

        let mut set = CriteriaSet::with_hash(self.id(), JITO_VALIDATORS_URL, criteria);
        if let Some(hash) = external_hash {
            set.raw_hash = hash;
        }
        Ok(set)
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        if let Ok(payload) = fetch_json(JITO_VALIDATORS_URL).await {
            let mut parsed = parse_eligible_validators(
                &payload,
                &["vote_account", "voteAccount", "vote_pubkey"],
                &["score"],
                &["jito_directed_stake_lamports", "active_stake"],
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
        let mev_bonus = (8.0 - validator.mev_commission).max(0.0) * 600.0;
        Some(6_500.0 + mev_bonus + validator.vote_credits * 35.0)
    }
}

fn default_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: "MEV commission cap".to_string(),
            metric: MetricKey::MevCommission,
            constraint: Constraint::Max(8.0),
            weight: Some(2.0),
            description: "Jito set favors low MEV fee settings".to_string(),
        },
        Criterion {
            name: "Skip rate".to_string(),
            metric: MetricKey::SkipRate,
            constraint: Constraint::Max(3.0),
            weight: Some(1.8),
            description: "Execution quality baseline".to_string(),
        },
        Criterion {
            name: "Vote credits".to_string(),
            metric: MetricKey::VoteCredits,
            constraint: Constraint::Min(97.8),
            weight: Some(1.8),
            description: "Consensus participation threshold".to_string(),
        },
    ]
}

fn fallback_eligible_set() -> Vec<EligibleValidator> {
    vec![
        EligibleValidator {
            vote_pubkey: "JitoSet01".to_string(),
            score: None,
            delegated_sol: Some(8_500.0),
        },
        EligibleValidator {
            vote_pubkey: "JitoSet02".to_string(),
            score: None,
            delegated_sol: Some(7_800.0),
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

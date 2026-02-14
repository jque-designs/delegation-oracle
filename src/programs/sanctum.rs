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
pub struct SanctumProgram;

const SANCTUM_PRIMARY_URL: &str = "https://sanctum-s-api.fly.dev/v1/validator/list";
const SANCTUM_FALLBACK_URLS: [&str; 3] = [
    "https://sanctum-s-api.fly.dev/v1/validators",
    "https://api.sanctum.so/v1/validators",
    "https://api.sanctum.so/v1/validator/list",
];
const MAX_ELIGIBLE_ITEMS: usize = 140;

#[async_trait]
impl DelegationProgram for SanctumProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Sanctum
    }

    fn name(&self) -> &str {
        "Sanctum"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        let mut criteria = default_criteria();
        let mut source_url = SANCTUM_PRIMARY_URL;
        let mut external_hash = None;

        for url in std::iter::once(SANCTUM_PRIMARY_URL).chain(SANCTUM_FALLBACK_URLS.iter().copied())
        {
            match fetch_json(url).await {
                Ok(payload) => {
                    source_url = url;
                    external_hash = Some(sha256_json(&payload));

                    let mev_values = collect_numeric_samples(
                        &payload,
                        &["mev_commission_bps", "mev_commission"],
                        1_500,
                    )
                    .into_iter()
                    .map(bps_to_percent_if_needed)
                    .collect::<Vec<_>>();
                    if let Some(mev_cap) = percentile(&mev_values, 0.75) {
                        set_max(
                            &mut criteria,
                            &MetricKey::MevCommission,
                            mev_cap.clamp(3.0, 10.0),
                        );
                    }

                    let stake_values = collect_numeric_samples(
                        &payload,
                        &[
                            "active_stake",
                            "activated_stake",
                            "stake",
                            "delegated_stake",
                        ],
                        1_500,
                    );
                    if let Some(min_stake_lamports) = percentile(&stake_values, 0.20) {
                        let min_stake_sol = lamports_to_sol_if_needed(min_stake_lamports);
                        set_min(
                            &mut criteria,
                            &MetricKey::ActivatedStake,
                            min_stake_sol.clamp(50_000.0, 300_000.0),
                        );
                    }
                    break;
                }
                Err(error) => {
                    debug!("sanctum criteria fetch failed for {url}: {error}");
                }
            }
        }

        let mut set = CriteriaSet::with_hash(self.id(), source_url, criteria);
        if let Some(hash) = external_hash {
            set.raw_hash = hash;
        }
        Ok(set)
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        for url in std::iter::once(SANCTUM_PRIMARY_URL).chain(SANCTUM_FALLBACK_URLS.iter().copied())
        {
            if let Ok(payload) = fetch_json(url).await {
                let mut parsed = parse_eligible_validators(
                    &payload,
                    &["vote_account", "vote_pubkey", "vote", "validator_vote"],
                    &["score", "sanctum_score"],
                    &[
                        "delegated_stake",
                        "active_stake",
                        "activated_stake",
                        "stake",
                    ],
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
        let stake_factor = (validator.activated_stake / 1_000.0) * 35.0;
        let mev_factor = (6.0 - validator.mev_commission).max(0.0) * 800.0;
        Some(10_000.0 + stake_factor + mev_factor)
    }
}

fn default_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: "Minimum activated stake".to_string(),
            metric: MetricKey::ActivatedStake,
            constraint: Constraint::Min(200_000.0),
            weight: Some(1.5),
            description: "LST integrations prefer deeper liquidity".to_string(),
        },
        Criterion {
            name: "MEV commission".to_string(),
            metric: MetricKey::MevCommission,
            constraint: Constraint::Max(5.0),
            weight: Some(1.7),
            description: "Fee competitiveness for orderflow".to_string(),
        },
        Criterion {
            name: "Commission".to_string(),
            metric: MetricKey::Commission,
            constraint: Constraint::Max(7.0),
            weight: Some(1.1),
            description: "User-facing fee baseline".to_string(),
        },
        Criterion {
            name: "Infrastructure diversity".to_string(),
            metric: MetricKey::InfrastructureDiversity,
            constraint: Constraint::Min(0.7),
            weight: Some(1.2),
            description: "Diversity requirement for resilient routing".to_string(),
        },
    ]
}

fn fallback_eligible_set() -> Vec<EligibleValidator> {
    vec![
        EligibleValidator {
            vote_pubkey: "SanctumSet01".to_string(),
            score: None,
            delegated_sol: Some(19_000.0),
        },
        EligibleValidator {
            vote_pubkey: "SanctumSet02".to_string(),
            score: None,
            delegated_sol: Some(17_400.0),
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

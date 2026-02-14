use anyhow::Result;
use async_trait::async_trait;
use tracing::debug;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::http::{
    fetch_json, fetch_text, lamports_to_sol_if_needed, parse_eligible_validators, sha256_hex,
};
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct SfdpProgram;

const SFDP_LEGACY_ENDPOINT: &str = "https://kyc-api.vercel.app/api/validators/list";
const SFDP_CONFIG_SOURCE: &str =
    "https://raw.githubusercontent.com/solana-foundation/stake-o-matic/master/stake-o-matic.sh";
const MAX_ELIGIBLE_ITEMS: usize = 120;

#[async_trait]
impl DelegationProgram for SfdpProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Sfdp
    }

    fn name(&self) -> &str {
        "SFDP"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        let mut criteria = default_criteria();
        let mut external_hash = None;

        match fetch_text(SFDP_CONFIG_SOURCE).await {
            Ok(script) => {
                external_hash = Some(sha256_hex(&script));
                if let Some(max_commission) = extract_cli_flag_value(&script, "--max-commission") {
                    set_max(
                        &mut criteria,
                        &MetricKey::Commission,
                        max_commission.clamp(5.0, 15.0),
                    );
                }
                if let Some(quality_pct) =
                    extract_cli_flag_value(&script, "--quality-block-producer-percentage")
                {
                    let min_uptime = (90.0 + (quality_pct / 10.0)).clamp(95.0, 99.0);
                    set_min(&mut criteria, &MetricKey::UptimePercent, min_uptime);
                }
            }
            Err(error) => debug!("sfdp config fetch failed, using fallback criteria: {error}"),
        }

        let mut set = CriteriaSet::with_hash(self.id(), SFDP_CONFIG_SOURCE, criteria);
        if let Some(hash) = external_hash {
            set.raw_hash = hash;
        }
        Ok(set)
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        if let Ok(payload) = fetch_json(SFDP_LEGACY_ENDPOINT).await {
            let mut parsed = parse_eligible_validators(
                &payload,
                &["vote_account", "vote_pubkey", "voteAccount", "vote"],
                &["score"],
                &["delegated_sol", "delegation", "stake", "activated_stake"],
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
        let base = 40_000.0 + validator.activated_stake * 0.06;
        Some(base.min(120_000.0))
    }
}

fn default_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: "Commission cap".to_string(),
            metric: MetricKey::Commission,
            constraint: Constraint::Max(7.0),
            weight: None,
            description: "Validator commission must remain competitive".to_string(),
        },
        Criterion {
            name: "Minimum activated stake".to_string(),
            metric: MetricKey::ActivatedStake,
            constraint: Constraint::Min(50_000.0),
            weight: None,
            description: "Minimum stake floor for program inclusion".to_string(),
        },
        Criterion {
            name: "Skip rate maximum".to_string(),
            metric: MetricKey::SkipRate,
            constraint: Constraint::Max(4.0),
            weight: None,
            description: "High skip-rate validators are excluded".to_string(),
        },
        Criterion {
            name: "Uptime minimum".to_string(),
            metric: MetricKey::UptimePercent,
            constraint: Constraint::Min(97.0),
            weight: None,
            description: "Reliability requirement".to_string(),
        },
        Criterion {
            name: "Outside superminority".to_string(),
            metric: MetricKey::SuperminorityStatus,
            constraint: Constraint::Boolean(false),
            weight: None,
            description: "Superminority risk mitigation".to_string(),
        },
        Criterion {
            name: "Supported Solana release".to_string(),
            metric: MetricKey::SolanaVersion,
            constraint: Constraint::OneOf(vec![
                "1.18.26".to_string(),
                "1.18.27".to_string(),
                "1.19.0".to_string(),
            ]),
            weight: None,
            description: "Version must match approved release window".to_string(),
        },
    ]
}

fn fallback_eligible_set() -> Vec<EligibleValidator> {
    vec![
        EligibleValidator {
            vote_pubkey: "SfdpEligible01".to_string(),
            score: None,
            delegated_sol: Some(50_000.0),
        },
        EligibleValidator {
            vote_pubkey: "SfdpEligible02".to_string(),
            score: None,
            delegated_sol: Some(43_500.0),
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

fn extract_cli_flag_value(script: &str, flag: &str) -> Option<f64> {
    let mut tokens = script.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        if token == flag {
            if let Some(value) = tokens.peek() {
                let cleaned = value.trim_matches(|c: char| c == '\\' || c == '"' || c == '\'');
                if let Ok(parsed) = cleaned.parse::<f64>() {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

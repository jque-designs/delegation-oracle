use anyhow::Result;
use async_trait::async_trait;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct SfdpProgram;

#[async_trait]
impl DelegationProgram for SfdpProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Sfdp
    }

    fn name(&self) -> &str {
        "SFDP"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        Ok(CriteriaSet::with_hash(
            self.id(),
            "https://kyc-api.vercel.app/api/validators/list",
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
            ],
        ))
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        Ok(vec![
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
        ])
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

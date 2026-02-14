use anyhow::Result;
use async_trait::async_trait;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct SanctumProgram;

#[async_trait]
impl DelegationProgram for SanctumProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Sanctum
    }

    fn name(&self) -> &str {
        "Sanctum"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        Ok(CriteriaSet::with_hash(
            self.id(),
            "https://sanctum-s-api.fly.dev/v1/validator/list",
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
            ],
        ))
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        Ok(vec![
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
        let stake_factor = (validator.activated_stake / 1_000.0) * 35.0;
        let mev_factor = (6.0 - validator.mev_commission).max(0.0) * 800.0;
        Some(10_000.0 + stake_factor + mev_factor)
    }
}

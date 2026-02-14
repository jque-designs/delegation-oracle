use anyhow::Result;
use async_trait::async_trait;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct JitoProgram;

#[async_trait]
impl DelegationProgram for JitoProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Jito
    }

    fn name(&self) -> &str {
        "Jito"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        Ok(CriteriaSet::with_hash(
            self.id(),
            "https://kobe.mainnet.jito.network/api/v1/validators",
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
            ],
        ))
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        Ok(vec![
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
        let mev_bonus = (8.0 - validator.mev_commission).max(0.0) * 600.0;
        Some(6_500.0 + mev_bonus + validator.vote_credits * 35.0)
    }
}

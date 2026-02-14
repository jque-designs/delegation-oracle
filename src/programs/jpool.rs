use anyhow::Result;
use async_trait::async_trait;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct JPoolProgram;

#[async_trait]
impl DelegationProgram for JPoolProgram {
    fn id(&self) -> ProgramId {
        ProgramId::JPool
    }

    fn name(&self) -> &str {
        "JPool"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        Ok(CriteriaSet::with_hash(
            self.id(),
            "https://api.jpool.one/validators",
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
            ],
        ))
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        Ok(vec![
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
        let base = 8_000.0;
        let performance = (validator.uptime_percent + validator.vote_credits) * 55.0;
        Some(base + performance)
    }
}

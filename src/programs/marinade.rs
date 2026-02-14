use anyhow::Result;
use async_trait::async_trait;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct MarinadeProgram;

#[async_trait]
impl DelegationProgram for MarinadeProgram {
    fn id(&self) -> ProgramId {
        ProgramId::Marinade
    }

    fn name(&self) -> &str {
        "Marinade"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        Ok(CriteriaSet::with_hash(
            self.id(),
            "https://validators-api.marinade.finance/validators",
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
            ],
        ))
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        Ok(vec![
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
        let performance = (validator.vote_credits + validator.uptime_percent) / 2.0;
        let commission_bonus = (10.0 - validator.commission).max(0.0) * 900.0;
        Some(18_000.0 + performance * 190.0 + commission_bonus)
    }
}

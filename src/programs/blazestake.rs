use anyhow::Result;
use async_trait::async_trait;

use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, ProgramId};
use crate::eligibility::evaluator::evaluate_validator;
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::{DelegationProgram, EligibleValidator};

#[derive(Debug, Clone, Copy)]
pub struct BlazeStakeProgram;

#[async_trait]
impl DelegationProgram for BlazeStakeProgram {
    fn id(&self) -> ProgramId {
        ProgramId::BlazeStake
    }

    fn name(&self) -> &str {
        "BlazeStake"
    }

    async fn fetch_criteria(&self) -> Result<CriteriaSet> {
        Ok(CriteriaSet::with_hash(
            self.id(),
            "https://stake.solblaze.org/api/v1/cls_validators",
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
            ],
        ))
    }

    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>> {
        Ok(vec![
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
        let skip_penalty = (validator.skip_rate * 400.0).min(2_000.0);
        Some((22_000.0 + performance * 120.0 - skip_penalty).max(0.0))
    }
}

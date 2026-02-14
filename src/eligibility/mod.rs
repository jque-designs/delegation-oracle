pub mod arbitrage;
pub mod evaluator;
pub mod history;
pub mod vulnerability;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::criteria::{Constraint, MetricKey, MetricValue, ProgramId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityResult {
    pub program: ProgramId,
    pub eligible: bool,
    pub score: Option<f64>,
    pub criterion_results: Vec<CriterionResult>,
    pub estimated_delegation_sol: Option<f64>,
}

impl EligibilityResult {
    pub fn passed_count(&self) -> usize {
        self.criterion_results.iter().filter(|c| c.passed).count()
    }

    pub fn failed_count(&self) -> usize {
        self.criterion_results
            .len()
            .saturating_sub(self.passed_count())
    }

    pub fn marginal_count(&self, epsilon_ratio: f64) -> usize {
        self.criterion_results
            .iter()
            .filter_map(|c| c.gap.as_ref())
            .filter(|gap| {
                if gap.required_value == 0.0 {
                    return false;
                }
                (gap.delta.abs() / gap.required_value.abs()) <= epsilon_ratio
            })
            .count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion_name: String,
    pub metric_key: MetricKey,
    pub your_value: MetricValue,
    pub required: Constraint,
    pub passed: bool,
    pub gap: Option<GapDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapDetail {
    pub metric_key: MetricKey,
    pub current_value: f64,
    pub required_value: f64,
    pub delta: f64,
    pub effort_estimate: EffortLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EffortLevel {
    Trivial,
    Moderate,
    Hard,
    Impossible,
}

impl EffortLevel {
    pub fn score(self) -> f64 {
        match self {
            Self::Trivial => 1.0,
            Self::Moderate => 2.0,
            Self::Hard => 4.0,
            Self::Impossible => 1_000.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub program: ProgramId,
    pub current_eligible: bool,
    pub gaps: Vec<GapDetail>,
    pub total_effort: EffortLevel,
    pub estimated_delegation_gain_sol: f64,
    pub roi_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerableValidator {
    pub vote_pubkey: String,
    pub program: ProgramId,
    pub metrics_at_risk: Vec<AtRiskMetric>,
    pub epochs_until_likely_loss: Option<u32>,
    pub current_delegation_sol: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtRiskMetric {
    pub metric: MetricKey,
    pub current_value: f64,
    pub threshold: f64,
    pub margin: f64,
    pub trend: TrendDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Improving,
    Stable,
    Deteriorating,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityRecord {
    pub vote_pubkey: String,
    pub program: ProgramId,
    pub epoch: u64,
    pub eligible: bool,
    pub score: Option<f64>,
    pub delegation_sol: Option<f64>,
    pub captured_at: DateTime<Utc>,
}

pub mod conflicts;
pub mod recommendations;
pub mod whatif;

use serde::{Deserialize, Serialize};

use crate::criteria::{Constraint, MetricKey, ProgramId};
use crate::eligibility::EligibilityResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricChange {
    pub metric: MetricKey,
    pub from: f64,
    pub to: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfResult {
    pub changes_applied: Vec<MetricChange>,
    pub before: Vec<EligibilityResult>,
    pub after: Vec<EligibilityResult>,
    pub programs_gained: Vec<ProgramId>,
    pub programs_lost: Vec<ProgramId>,
    pub net_delegation_change_sol: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramConflict {
    pub metric: MetricKey,
    pub program_a: ProgramId,
    pub program_a_wants: Constraint,
    pub program_b: ProgramId,
    pub program_b_wants: Constraint,
    pub conflict_type: ConflictType,
    pub recommendation: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    DirectContradiction,
    TensionZone,
    IndirectImpact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    pub priority: usize,
    pub title: String,
    pub rationale: String,
    pub expected_gain_sol: f64,
    pub effort: String,
}

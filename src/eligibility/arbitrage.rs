use std::collections::BTreeMap;

use crate::criteria::ProgramId;
use crate::eligibility::{ArbitrageOpportunity, EffortLevel, EligibilityResult, GapDetail};

pub fn build_arbitrage_opportunities(
    results: &[EligibilityResult],
    estimate_by_program: &BTreeMap<ProgramId, f64>,
) -> Vec<ArbitrageOpportunity> {
    let mut opportunities = Vec::new();
    for result in results {
        if result.eligible {
            continue;
        }

        let gaps: Vec<GapDetail> = result
            .criterion_results
            .iter()
            .filter_map(|c| c.gap.clone())
            .collect();
        if gaps.is_empty() {
            continue;
        }

        let total_effort = gaps
            .iter()
            .map(|g| g.effort_estimate)
            .max()
            .unwrap_or(EffortLevel::Impossible);

        let estimated_delegation_gain_sol = estimate_by_program
            .get(&result.program)
            .copied()
            .or(result.estimated_delegation_sol)
            .unwrap_or(0.0);

        let roi_score = if total_effort.score() > 0.0 {
            estimated_delegation_gain_sol / total_effort.score()
        } else {
            0.0
        };

        opportunities.push(ArbitrageOpportunity {
            program: result.program,
            current_eligible: result.eligible,
            gaps,
            total_effort,
            estimated_delegation_gain_sol,
            roi_score,
        });
    }

    opportunities.sort_by(|a, b| b.roi_score.total_cmp(&a.roi_score));
    opportunities
}

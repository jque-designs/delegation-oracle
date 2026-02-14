use crate::eligibility::{ArbitrageOpportunity, EffortLevel};
use crate::optimizer::{ConflictType, OptimizationRecommendation, ProgramConflict};

pub fn build_recommendations(
    opportunities: &[ArbitrageOpportunity],
    conflicts: &[ProgramConflict],
    max_items: usize,
) -> Vec<OptimizationRecommendation> {
    let mut recommendations = Vec::new();
    let mut rank = 1usize;

    for opportunity in opportunities.iter().take(max_items) {
        let actions = opportunity
            .gaps
            .iter()
            .map(|gap| format!("{} (delta {:.3})", gap.metric_key, gap.delta))
            .collect::<Vec<_>>()
            .join(", ");

        recommendations.push(OptimizationRecommendation {
            priority: rank,
            title: format!("Target {}", opportunity.program),
            rationale: format!(
                "Estimated +{:.0} SOL if eligible. Required changes: {}.",
                opportunity.estimated_delegation_gain_sol, actions
            ),
            expected_gain_sol: opportunity.estimated_delegation_gain_sol,
            effort: effort_label(opportunity.total_effort).to_string(),
        });
        rank += 1;
    }

    for conflict in conflicts
        .iter()
        .filter(|c| matches!(c.conflict_type, ConflictType::DirectContradiction))
        .take(max_items.saturating_sub(recommendations.len()))
    {
        recommendations.push(OptimizationRecommendation {
            priority: rank,
            title: format!(
                "Resolve {} conflict ({} vs {})",
                conflict.metric, conflict.program_a, conflict.program_b
            ),
            rationale: conflict.recommendation.clone(),
            expected_gain_sol: 0.0,
            effort: "high".to_string(),
        });
        rank += 1;
    }

    recommendations
}

fn effort_label(effort: EffortLevel) -> &'static str {
    match effort {
        EffortLevel::Trivial => "trivial",
        EffortLevel::Moderate => "moderate",
        EffortLevel::Hard => "hard",
        EffortLevel::Impossible => "impossible",
    }
}

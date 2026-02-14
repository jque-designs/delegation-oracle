use crate::criteria::{Constraint, CriteriaSet, MetricKey, ProgramId};
use crate::optimizer::{ConflictType, ProgramConflict};

#[derive(Debug, Clone)]
struct MetricConstraintRef {
    program: ProgramId,
    metric: MetricKey,
    constraint: Constraint,
}

pub fn detect_conflicts(criteria_sets: &[CriteriaSet]) -> Vec<ProgramConflict> {
    let mut refs = Vec::new();
    for set in criteria_sets {
        for criterion in &set.criteria {
            refs.push(MetricConstraintRef {
                program: set.program,
                metric: criterion.metric.clone(),
                constraint: criterion.constraint.clone(),
            });
        }
    }

    let mut conflicts = Vec::new();
    for i in 0..refs.len() {
        for j in (i + 1)..refs.len() {
            let a = &refs[i];
            let b = &refs[j];
            if a.metric != b.metric || a.program == b.program {
                continue;
            }

            if let Some(conflict) = compare_constraints(a, b) {
                conflicts.push(conflict);
            }
        }
    }

    conflicts
}

fn compare_constraints(
    a: &MetricConstraintRef,
    b: &MetricConstraintRef,
) -> Option<ProgramConflict> {
    let recommendation = format!(
        "Track {} in a shared target window to satisfy {} and {}.",
        a.metric, a.program, b.program
    );

    let build = |conflict_type: ConflictType, recommendation: String| ProgramConflict {
        metric: a.metric.clone(),
        program_a: a.program,
        program_a_wants: a.constraint.clone(),
        program_b: b.program,
        program_b_wants: b.constraint.clone(),
        conflict_type,
        recommendation,
    };

    match (&a.constraint, &b.constraint) {
        (Constraint::Min(min), Constraint::Max(max))
        | (Constraint::Max(max), Constraint::Min(min)) => {
            if min > max {
                return Some(build(
                    ConflictType::DirectContradiction,
                    format!(
                        "{} requires >= {min}, but {} requires <= {max}. Prioritize program ROI and pick one target.",
                        a.program, b.program
                    ),
                ));
            }
            if (max - min) <= (min.abs().max(1.0) * 0.1) {
                return Some(build(
                    ConflictType::TensionZone,
                    format!(
                        "Constraint window is narrow ({min}..{max}). Introduce monitoring guardrails before changing {}.",
                        a.metric
                    ),
                ));
            }
        }
        (
            Constraint::Range {
                min: a_min,
                max: a_max,
            },
            Constraint::Range {
                min: b_min,
                max: b_max,
            },
        ) => {
            let overlap_min = a_min.max(*b_min);
            let overlap_max = a_max.min(*b_max);
            if overlap_min > overlap_max {
                return Some(build(
                    ConflictType::DirectContradiction,
                    "No overlapping range between programs for this metric.".to_string(),
                ));
            }
            if (overlap_max - overlap_min) <= (a_max - a_min).abs().min((b_max - b_min).abs()) * 0.2
            {
                return Some(build(
                    ConflictType::TensionZone,
                    format!("Shared feasible range is tight: [{overlap_min}, {overlap_max}]"),
                ));
            }
        }
        _ => {}
    }

    match a.metric {
        MetricKey::Commission | MetricKey::MevCommission => Some(build(
            ConflictType::IndirectImpact,
            format!(
                "Lower {} may improve eligibility but can reduce validator revenue; model infra budget impact.",
                a.metric
            ),
        )),
        _ => {
            let _ = recommendation;
            None
        }
    }
}

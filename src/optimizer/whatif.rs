use anyhow::Result;

use crate::criteria::{MetricKey, ProgramId};
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::optimizer::{MetricChange, WhatIfResult};
use crate::programs::ProgramRegistry;

pub async fn evaluate_all_programs(
    registry: &ProgramRegistry,
    validator: &ValidatorMetrics,
    filter: Option<&[ProgramId]>,
) -> Result<Vec<EligibilityResult>> {
    let mut out = Vec::new();
    for program in registry.programs() {
        if let Some(ids) = filter {
            if !ids.contains(&program.id()) {
                continue;
            }
        }
        let criteria = program.fetch_criteria().await?;
        out.push(program.evaluate(validator, &criteria));
    }
    Ok(out)
}

pub async fn simulate_whatif(
    registry: &ProgramRegistry,
    current_metrics: &ValidatorMetrics,
    target_changes: &[(MetricKey, f64)],
    filter: Option<&[ProgramId]>,
) -> Result<WhatIfResult> {
    let before = evaluate_all_programs(registry, current_metrics, filter).await?;

    let mut changed = current_metrics.clone();
    let mut changes_applied = Vec::new();
    for (metric, to) in target_changes {
        if let Some(from) = changed.numeric_metric(metric) {
            if changed.apply_numeric_change(metric, *to) {
                changes_applied.push(MetricChange {
                    metric: metric.clone(),
                    from,
                    to: *to,
                });
            }
        }
    }

    let after = evaluate_all_programs(registry, &changed, filter).await?;
    let programs_gained = gained_programs(&before, &after);
    let programs_lost = lost_programs(&before, &after);
    let net_delegation_change_sol = delegation_sum(&after) - delegation_sum(&before);

    Ok(WhatIfResult {
        changes_applied,
        before,
        after,
        programs_gained,
        programs_lost,
        net_delegation_change_sol,
    })
}

fn gained_programs(before: &[EligibilityResult], after: &[EligibilityResult]) -> Vec<ProgramId> {
    let mut out = Vec::new();
    for old in before {
        if let Some(new) = after.iter().find(|n| n.program == old.program) {
            if !old.eligible && new.eligible {
                out.push(new.program);
            }
        }
    }
    out
}

fn lost_programs(before: &[EligibilityResult], after: &[EligibilityResult]) -> Vec<ProgramId> {
    let mut out = Vec::new();
    for old in before {
        if let Some(new) = after.iter().find(|n| n.program == old.program) {
            if old.eligible && !new.eligible {
                out.push(new.program);
            }
        }
    }
    out
}

fn delegation_sum(results: &[EligibilityResult]) -> f64 {
    results
        .iter()
        .filter_map(|r| r.estimated_delegation_sol)
        .sum::<f64>()
}

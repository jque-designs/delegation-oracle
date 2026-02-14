use crate::criteria::{Constraint, CriteriaSet, Criterion, MetricKey, MetricValue, ProgramId};
use crate::eligibility::{CriterionResult, EffortLevel, EligibilityResult, GapDetail};
use crate::metrics::ValidatorMetrics;

pub fn evaluate_validator(
    program: ProgramId,
    validator: &ValidatorMetrics,
    criteria_set: &CriteriaSet,
    estimated_delegation_if_eligible: Option<f64>,
) -> EligibilityResult {
    let mut criterion_results = Vec::with_capacity(criteria_set.criteria.len());
    let mut weighted_pass = 0.0;
    let mut weighted_total = 0.0;

    for criterion in &criteria_set.criteria {
        let result = evaluate_criterion(validator, criterion);
        let weight = criterion.weight.unwrap_or(1.0).max(0.0);
        weighted_total += weight;
        if result.passed {
            weighted_pass += weight;
        }
        criterion_results.push(result);
    }

    let eligible = criterion_results.iter().all(|r| r.passed);
    let score = if weighted_total > 0.0 {
        Some(weighted_pass / weighted_total)
    } else {
        None
    };

    EligibilityResult {
        program,
        eligible,
        score,
        criterion_results,
        estimated_delegation_sol: if eligible {
            estimated_delegation_if_eligible
        } else {
            None
        },
    }
}

pub fn evaluate_criterion(validator: &ValidatorMetrics, criterion: &Criterion) -> CriterionResult {
    let your_value = validator
        .metric_value(&criterion.metric)
        .unwrap_or(MetricValue::Text("unknown".to_string()));
    let (passed, gap) = match (&your_value, &criterion.constraint) {
        (MetricValue::Numeric(v), Constraint::Min(required)) => {
            if v >= required {
                (true, None)
            } else {
                let delta = required - v;
                (
                    false,
                    Some(GapDetail {
                        metric_key: criterion.metric.clone(),
                        current_value: *v,
                        required_value: *required,
                        delta,
                        effort_estimate: estimate_effort(&criterion.metric, delta, *required),
                    }),
                )
            }
        }
        (MetricValue::Numeric(v), Constraint::Max(required)) => {
            if v <= required {
                (true, None)
            } else {
                let delta = v - required;
                (
                    false,
                    Some(GapDetail {
                        metric_key: criterion.metric.clone(),
                        current_value: *v,
                        required_value: *required,
                        delta,
                        effort_estimate: estimate_effort(&criterion.metric, delta, *required),
                    }),
                )
            }
        }
        (MetricValue::Numeric(v), Constraint::Range { min, max }) => {
            if (*min..=*max).contains(v) {
                (true, None)
            } else if v < min {
                let delta = min - v;
                (
                    false,
                    Some(GapDetail {
                        metric_key: criterion.metric.clone(),
                        current_value: *v,
                        required_value: *min,
                        delta,
                        effort_estimate: estimate_effort(&criterion.metric, delta, *min),
                    }),
                )
            } else {
                let delta = v - max;
                (
                    false,
                    Some(GapDetail {
                        metric_key: criterion.metric.clone(),
                        current_value: *v,
                        required_value: *max,
                        delta,
                        effort_estimate: estimate_effort(&criterion.metric, delta, *max),
                    }),
                )
            }
        }
        (MetricValue::Text(v), Constraint::Equals(required)) => (v == required, None),
        (MetricValue::Text(v), Constraint::OneOf(required)) => (required.contains(v), None),
        (MetricValue::Bool(v), Constraint::Boolean(required)) => (*v == *required, None),
        (_, Constraint::Custom(_)) => (true, None),
        _ => (false, None),
    };

    CriterionResult {
        criterion_name: criterion.name.clone(),
        metric_key: criterion.metric.clone(),
        your_value,
        required: criterion.constraint.clone(),
        passed,
        gap,
    }
}

pub fn estimate_effort(metric: &MetricKey, delta: f64, required: f64) -> EffortLevel {
    match metric {
        MetricKey::Commission | MetricKey::MevCommission => EffortLevel::Trivial,
        MetricKey::SkipRate | MetricKey::VoteCredits | MetricKey::UptimePercent => {
            if delta <= 1.0 {
                EffortLevel::Moderate
            } else {
                EffortLevel::Hard
            }
        }
        MetricKey::ActivatedStake => {
            if required <= 0.0 {
                EffortLevel::Impossible
            } else {
                let ratio = delta / required;
                if ratio <= 0.1 {
                    EffortLevel::Moderate
                } else if ratio <= 0.35 {
                    EffortLevel::Hard
                } else {
                    EffortLevel::Impossible
                }
            }
        }
        MetricKey::DatacenterConcentration
        | MetricKey::InfrastructureDiversity
        | MetricKey::StakeConcentration => EffortLevel::Hard,
        MetricKey::SolanaVersion => EffortLevel::Trivial,
        MetricKey::SuperminorityStatus => EffortLevel::Impossible,
        MetricKey::Custom(_) => EffortLevel::Moderate,
    }
}

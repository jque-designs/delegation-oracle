use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use crate::criteria::schema::{Constraint, CriteriaSet, ProgramId};
use crate::eligibility::EligibilityResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaDrift {
    pub program: ProgramId,
    pub detected_at: DateTime<Utc>,
    pub changes: Vec<CriterionChange>,
    pub impact_on_you: DriftImpact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionChange {
    pub criterion_name: String,
    pub change_type: ChangeType,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Removed,
    ThresholdChanged,
    WeightChanged,
    DescriptionChanged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftImpact {
    NowEligible,
    StillEligible,
    AtRisk,
    NowIneligible,
    NotApplicable,
}

pub fn diff_criteria(old_set: &CriteriaSet, new_set: &CriteriaSet) -> Vec<CriterionChange> {
    let mut old_map = BTreeMap::new();
    let mut new_map = BTreeMap::new();

    for c in &old_set.criteria {
        old_map.insert(c.name.clone(), c);
    }
    for c in &new_set.criteria {
        new_map.insert(c.name.clone(), c);
    }

    let mut names = BTreeSet::new();
    names.extend(old_map.keys().cloned());
    names.extend(new_map.keys().cloned());

    let mut changes = Vec::new();
    for name in names {
        match (old_map.get(&name), new_map.get(&name)) {
            (None, Some(new_c)) => changes.push(CriterionChange {
                criterion_name: name,
                change_type: ChangeType::Added,
                old_value: None,
                new_value: Some(format_constraint(&new_c.constraint)),
            }),
            (Some(old_c), None) => changes.push(CriterionChange {
                criterion_name: name,
                change_type: ChangeType::Removed,
                old_value: Some(format_constraint(&old_c.constraint)),
                new_value: None,
            }),
            (Some(old_c), Some(new_c)) => {
                if old_c.constraint != new_c.constraint {
                    changes.push(CriterionChange {
                        criterion_name: name.clone(),
                        change_type: ChangeType::ThresholdChanged,
                        old_value: Some(format_constraint(&old_c.constraint)),
                        new_value: Some(format_constraint(&new_c.constraint)),
                    });
                }
                if old_c.weight != new_c.weight {
                    changes.push(CriterionChange {
                        criterion_name: name.clone(),
                        change_type: ChangeType::WeightChanged,
                        old_value: old_c.weight.map(|v| format!("{v:.4}")),
                        new_value: new_c.weight.map(|v| format!("{v:.4}")),
                    });
                }
                if old_c.description != new_c.description {
                    changes.push(CriterionChange {
                        criterion_name: name,
                        change_type: ChangeType::DescriptionChanged,
                        old_value: Some(old_c.description.clone()),
                        new_value: Some(new_c.description.clone()),
                    });
                }
            }
            (None, None) => {}
        }
    }

    changes
}

pub fn classify_drift_impact(
    before: Option<&EligibilityResult>,
    after: Option<&EligibilityResult>,
) -> DriftImpact {
    let (Some(before), Some(after)) = (before, after) else {
        return DriftImpact::NotApplicable;
    };

    match (before.eligible, after.eligible) {
        (false, true) => DriftImpact::NowEligible,
        (true, true) => {
            if after.failed_count() > 0 || after.marginal_count(0.03) > 0 {
                DriftImpact::AtRisk
            } else {
                DriftImpact::StillEligible
            }
        }
        (true, false) => DriftImpact::NowIneligible,
        (false, false) => DriftImpact::NotApplicable,
    }
}

pub fn build_drift_report(
    old_set: &CriteriaSet,
    new_set: &CriteriaSet,
    before: Option<&EligibilityResult>,
    after: Option<&EligibilityResult>,
) -> Option<CriteriaDrift> {
    if old_set.raw_hash == new_set.raw_hash {
        return None;
    }

    let changes = diff_criteria(old_set, new_set);
    let impact_on_you = classify_drift_impact(before, after);
    Some(CriteriaDrift {
        program: new_set.program,
        detected_at: Utc::now(),
        changes,
        impact_on_you,
    })
}

pub fn textual_diff(old_set: &CriteriaSet, new_set: &CriteriaSet) -> String {
    let old_str = serde_json::to_string_pretty(old_set).unwrap_or_default();
    let new_str = serde_json::to_string_pretty(new_set).unwrap_or_default();
    let diff = TextDiff::from_lines(&old_str, &new_str);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let symbol = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        out.push_str(symbol);
        out.push_str(change.value());
    }
    out
}

fn format_constraint(constraint: &Constraint) -> String {
    constraint.to_string()
}

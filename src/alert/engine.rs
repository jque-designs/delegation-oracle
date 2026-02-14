use serde::{Deserialize, Serialize};

use crate::alert::rules::AlertEventKind;
use crate::criteria::CriteriaDrift;
use crate::eligibility::{EligibilityResult, VulnerableValidator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub kind: AlertEventKind,
    pub title: String,
    pub body: String,
}

pub fn evaluate_alerts(
    previous: Option<&[EligibilityResult]>,
    current: &[EligibilityResult],
    drifts: &[CriteriaDrift],
    vulnerabilities: &[VulnerableValidator],
) -> Vec<AlertEvent> {
    let mut events = Vec::new();

    for drift in drifts {
        events.push(AlertEvent {
            kind: AlertEventKind::CriteriaDrift,
            title: format!("Criteria drift detected in {}", drift.program),
            body: format!(
                "{} changes detected; impact: {:?}",
                drift.changes.len(),
                drift.impact_on_you
            ),
        });
    }

    for item in vulnerabilities {
        events.push(AlertEvent {
            kind: AlertEventKind::VulnerabilityDetected,
            title: format!("Validator {} is vulnerable", item.vote_pubkey),
            body: format!(
                "{} at-risk metrics in {} with {:.0} SOL delegated",
                item.metrics_at_risk.len(),
                item.program,
                item.current_delegation_sol
            ),
        });
    }

    if let Some(previous) = previous {
        for before in previous {
            if let Some(after) = current.iter().find(|item| item.program == before.program) {
                if before.eligible && !after.eligible {
                    events.push(AlertEvent {
                        kind: AlertEventKind::EligibilityLost,
                        title: format!("Eligibility lost in {}", after.program),
                        body: "One or more criteria no longer pass.".to_string(),
                    });
                } else if !before.eligible && after.eligible {
                    events.push(AlertEvent {
                        kind: AlertEventKind::EligibilityGained,
                        title: format!("Eligibility gained in {}", after.program),
                        body: "Validator now qualifies for delegation.".to_string(),
                    });
                }
            }
        }
    }

    events
}

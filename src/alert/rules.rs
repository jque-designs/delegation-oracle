use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertRule {
    CriteriaDrift,
    VulnerabilityDetected,
    EligibilityLost,
    EligibilityGained,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertEventKind {
    CriteriaDrift,
    VulnerabilityDetected,
    EligibilityLost,
    EligibilityGained,
}

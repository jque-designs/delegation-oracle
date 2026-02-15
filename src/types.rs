use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramStatus {
    pub name: String,
    pub display_name: String,
    pub status: RegistrationStatus,
    pub current_stake_sol: f64,
    pub potential_stake_sol: f64,
    pub gap_sol: f64,
    pub registration_url: Option<String>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    Active,
    Eligible,
    NotRegistered,
    Ineligible,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub validator: String,
    pub scanned_at: DateTime<Utc>,
    pub programs: Vec<ProgramStatus>,
    pub summary: ScanSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub total_current_sol: f64,
    pub total_potential_sol: f64,
    pub missed_revenue_sol: f64,
    pub missed_revenue_usd: f64,
    pub action_items: Vec<ActionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub program: String,
    pub action: String,
    pub potential_gain_sol: f64,
    pub url: Option<String>,
    pub difficulty: Difficulty,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl ProgramStatus {
    pub fn new(name: &str, display_name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            status: RegistrationStatus::Unknown,
            current_stake_sol: 0.0,
            potential_stake_sol: 0.0,
            gap_sol: 0.0,
            registration_url: None,
            details: serde_json::Value::Null,
        }
    }
    
    pub fn with_status(mut self, status: RegistrationStatus) -> Self {
        self.status = status;
        self
    }
    
    pub fn with_stake(mut self, current: f64, potential: f64) -> Self {
        self.current_stake_sol = current;
        self.potential_stake_sol = potential;
        self.gap_sol = potential - current;
        self
    }
    
    pub fn with_registration_url(mut self, url: &str) -> Self {
        self.registration_url = Some(url.to_string());
        self
    }
    
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
}

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ProgramId {
    Sfdp,
    Marinade,
    JPool,
    BlazeStake,
    Jito,
    Sanctum,
}

impl ProgramId {
    pub const ALL: [ProgramId; 6] = [
        ProgramId::Sfdp,
        ProgramId::Marinade,
        ProgramId::JPool,
        ProgramId::BlazeStake,
        ProgramId::Jito,
        ProgramId::Sanctum,
    ];

    pub fn as_slug(&self) -> &'static str {
        match self {
            Self::Sfdp => "sfdp",
            Self::Marinade => "marinade",
            Self::JPool => "jpool",
            Self::BlazeStake => "blazestake",
            Self::Jito => "jito",
            Self::Sanctum => "sanctum",
        }
    }
}

impl Display for ProgramId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            Self::Sfdp => "SFDP",
            Self::Marinade => "Marinade",
            Self::JPool => "JPool",
            Self::BlazeStake => "BlazeStake",
            Self::Jito => "Jito",
            Self::Sanctum => "Sanctum",
        };
        write!(f, "{display}")
    }
}

#[derive(Debug, Error)]
#[error("unknown program id: {0}")]
pub struct ProgramParseError(pub String);

impl FromStr for ProgramId {
    type Err = ProgramParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "sfdp" | "solana-foundation" => Ok(Self::Sfdp),
            "marinade" => Ok(Self::Marinade),
            "jpool" | "j_pool" => Ok(Self::JPool),
            "blazestake" | "blaze" => Ok(Self::BlazeStake),
            "jito" => Ok(Self::Jito),
            "sanctum" => Ok(Self::Sanctum),
            _ => Err(ProgramParseError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CriteriaSet {
    pub program: ProgramId,
    pub fetched_at: DateTime<Utc>,
    pub source_url: String,
    pub criteria: Vec<Criterion>,
    pub raw_hash: String,
}

impl CriteriaSet {
    pub fn with_hash(
        program: ProgramId,
        source_url: impl Into<String>,
        criteria: Vec<Criterion>,
    ) -> Self {
        let source_url = source_url.into();
        let canonical = serde_json::to_string(&criteria).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let raw_hash = format!("{:x}", hasher.finalize());
        Self {
            program,
            fetched_at: Utc::now(),
            source_url,
            criteria,
            raw_hash,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Criterion {
    pub name: String,
    pub metric: MetricKey,
    pub constraint: Constraint,
    pub weight: Option<f64>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Constraint {
    Min(f64),
    Max(f64),
    Range { min: f64, max: f64 },
    Equals(String),
    OneOf(Vec<String>),
    Boolean(bool),
    Custom(String),
}

impl Display for Constraint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Constraint::Min(v) => write!(f, ">= {v}"),
            Constraint::Max(v) => write!(f, "<= {v}"),
            Constraint::Range { min, max } => write!(f, "[{min}, {max}]"),
            Constraint::Equals(v) => write!(f, "== {v}"),
            Constraint::OneOf(values) => write!(f, "one of {values:?}"),
            Constraint::Boolean(v) => write!(f, "== {v}"),
            Constraint::Custom(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MetricKey {
    Commission,
    ActivatedStake,
    SkipRate,
    VoteCredits,
    UptimePercent,
    SolanaVersion,
    DatacenterConcentration,
    SuperminorityStatus,
    MevCommission,
    StakeConcentration,
    InfrastructureDiversity,
    Custom(String),
}

impl Display for MetricKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commission => write!(f, "commission"),
            Self::ActivatedStake => write!(f, "activated_stake"),
            Self::SkipRate => write!(f, "skip_rate"),
            Self::VoteCredits => write!(f, "vote_credits"),
            Self::UptimePercent => write!(f, "uptime_percent"),
            Self::SolanaVersion => write!(f, "solana_version"),
            Self::DatacenterConcentration => write!(f, "datacenter_concentration"),
            Self::SuperminorityStatus => write!(f, "superminority_status"),
            Self::MevCommission => write!(f, "mev_commission"),
            Self::StakeConcentration => write!(f, "stake_concentration"),
            Self::InfrastructureDiversity => write!(f, "infrastructure_diversity"),
            Self::Custom(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Error)]
#[error("unknown metric key: {0}")]
pub struct MetricKeyParseError(pub String);

impl FromStr for MetricKey {
    type Err = MetricKeyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('-', "_");
        let key = match normalized.as_str() {
            "commission" => MetricKey::Commission,
            "activated_stake" | "stake" | "active_stake" => MetricKey::ActivatedStake,
            "skip_rate" => MetricKey::SkipRate,
            "vote_credits" | "credits" => MetricKey::VoteCredits,
            "uptime_percent" | "uptime" => MetricKey::UptimePercent,
            "solana_version" | "version" => MetricKey::SolanaVersion,
            "datacenter_concentration" | "dc_concentration" => MetricKey::DatacenterConcentration,
            "superminority_status" | "superminority" => MetricKey::SuperminorityStatus,
            "mev_commission" => MetricKey::MevCommission,
            "stake_concentration" => MetricKey::StakeConcentration,
            "infrastructure_diversity" | "infra_diversity" => MetricKey::InfrastructureDiversity,
            _ => {
                if normalized.is_empty() {
                    return Err(MetricKeyParseError(s.to_string()));
                }
                MetricKey::Custom(normalized)
            }
        };
        Ok(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MetricValue {
    Numeric(f64),
    Text(String),
    Bool(bool),
}

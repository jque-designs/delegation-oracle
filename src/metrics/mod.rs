pub mod cache;
pub mod collector;
pub mod normalize;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::criteria::{MetricKey, MetricValue};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorMetrics {
    pub vote_pubkey: String,
    pub commission: f64,
    pub activated_stake: f64,
    pub skip_rate: f64,
    pub vote_credits: f64,
    pub uptime_percent: f64,
    pub solana_version: String,
    pub datacenter_concentration: f64,
    pub superminority_status: bool,
    pub mev_commission: f64,
    pub stake_concentration: f64,
    pub infrastructure_diversity: f64,
    #[serde(default)]
    pub custom_numeric: BTreeMap<String, f64>,
}

impl ValidatorMetrics {
    pub fn sample(vote_pubkey: impl Into<String>) -> Self {
        Self {
            vote_pubkey: vote_pubkey.into(),
            commission: 5.0,
            activated_stake: 156_000.0,
            skip_rate: 3.2,
            vote_credits: 98.5,
            uptime_percent: 99.1,
            solana_version: "1.18.26".to_string(),
            datacenter_concentration: 56.0,
            superminority_status: false,
            mev_commission: 8.0,
            stake_concentration: 0.18,
            infrastructure_diversity: 0.65,
            custom_numeric: BTreeMap::new(),
        }
    }

    pub fn metric_value(&self, key: &MetricKey) -> Option<MetricValue> {
        let value = match key {
            MetricKey::Commission => MetricValue::Numeric(self.commission),
            MetricKey::ActivatedStake => MetricValue::Numeric(self.activated_stake),
            MetricKey::SkipRate => MetricValue::Numeric(self.skip_rate),
            MetricKey::VoteCredits => MetricValue::Numeric(self.vote_credits),
            MetricKey::UptimePercent => MetricValue::Numeric(self.uptime_percent),
            MetricKey::SolanaVersion => MetricValue::Text(self.solana_version.clone()),
            MetricKey::DatacenterConcentration => {
                MetricValue::Numeric(self.datacenter_concentration)
            }
            MetricKey::SuperminorityStatus => MetricValue::Bool(self.superminority_status),
            MetricKey::MevCommission => MetricValue::Numeric(self.mev_commission),
            MetricKey::StakeConcentration => MetricValue::Numeric(self.stake_concentration),
            MetricKey::InfrastructureDiversity => {
                MetricValue::Numeric(self.infrastructure_diversity)
            }
            MetricKey::Custom(name) => {
                MetricValue::Numeric(*self.custom_numeric.get(name).unwrap_or(&0.0))
            }
        };
        Some(value)
    }

    pub fn numeric_metric(&self, key: &MetricKey) -> Option<f64> {
        match self.metric_value(key)? {
            MetricValue::Numeric(v) => Some(v),
            _ => None,
        }
    }

    pub fn apply_numeric_change(&mut self, key: &MetricKey, to: f64) -> bool {
        match key {
            MetricKey::Commission => self.commission = to,
            MetricKey::ActivatedStake => self.activated_stake = to,
            MetricKey::SkipRate => self.skip_rate = to,
            MetricKey::VoteCredits => self.vote_credits = to,
            MetricKey::UptimePercent => self.uptime_percent = to,
            MetricKey::DatacenterConcentration => self.datacenter_concentration = to,
            MetricKey::MevCommission => self.mev_commission = to,
            MetricKey::StakeConcentration => self.stake_concentration = to,
            MetricKey::InfrastructureDiversity => self.infrastructure_diversity = to,
            MetricKey::Custom(name) => {
                self.custom_numeric.insert(name.clone(), to);
            }
            MetricKey::SolanaVersion | MetricKey::SuperminorityStatus => return false,
        }
        true
    }
}

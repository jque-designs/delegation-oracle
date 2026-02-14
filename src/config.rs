use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub validator: ValidatorConfig,
    #[serde(default)]
    pub rpc: RpcConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub programs: ProgramsConfig,
    #[serde(default)]
    pub analysis: AnalysisConfig,
    #[serde(default)]
    pub optimizer: OptimizerConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidatorConfig {
    #[serde(default)]
    pub vote_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    #[serde(default = "default_rpc_url")]
    pub url: String,
    #[serde(default = "default_requests_per_second")]
    pub requests_per_second: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramsConfig {
    #[serde(default = "default_programs_enabled")]
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    #[serde(default = "default_vulnerability_margin")]
    pub vulnerability_margin_pct: f64,
    #[serde(default = "default_lookback_epochs")]
    pub lookback_epochs: u32,
    #[serde(default = "default_drift_hours")]
    pub drift_check_interval_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    #[serde(default = "default_revenue_per_sol_per_epoch")]
    pub revenue_per_sol_per_epoch: f64,
    #[serde(default = "default_monthly_infra_cost_usd")]
    pub monthly_infra_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default)]
    pub discord_webhook: String,
    #[serde(default)]
    pub telegram_bot_token: String,
    #[serde(default)]
    pub telegram_chat_id: String,
    #[serde(default = "default_enable_stdout")]
    pub enable_stdout: bool,
    #[serde(default)]
    pub rules: AlertRulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRulesConfig {
    #[serde(default = "default_true")]
    pub criteria_drift: bool,
    #[serde(default = "default_true")]
    pub vulnerability_detected: bool,
    #[serde(default = "default_true")]
    pub eligibility_lost: bool,
    #[serde(default = "default_true")]
    pub eligibility_gained: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub vote_pubkey: Option<String>,
    pub rpc_url: Option<String>,
    pub enabled_programs: Option<Vec<String>>,
}

impl Config {
    pub fn default_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".config/delegation-oracle/config.toml")
    }

    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_path);
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(&path)
            .with_context(|| format!("failed reading config: {}", path.display()))?;
        let parsed: Self = toml::from_str(&data)
            .with_context(|| format!("failed parsing TOML config: {}", path.display()))?;
        Ok(parsed)
    }

    pub fn apply_overrides(&mut self, overrides: ConfigOverrides) {
        if let Some(vote_pubkey) = overrides.vote_pubkey {
            self.validator.vote_pubkey = vote_pubkey;
        }
        if let Some(rpc_url) = overrides.rpc_url {
            self.rpc.url = rpc_url;
        }
        if let Some(programs) = overrides.enabled_programs {
            self.programs.enabled = programs;
        }
    }

    pub fn write_template(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed creating config directory: {}", parent.display())
            })?;
        }
        fs::write(path, Self::default_template())
            .with_context(|| format!("failed writing config template: {}", path.display()))
    }

    pub fn resolved_db_path(&self) -> PathBuf {
        expand_tilde(&self.storage.db_path)
    }

    pub fn default_template() -> String {
        let template = r#"[validator]
vote_pubkey = "YourVoteAccountPubkeyHere"

[rpc]
url = "https://api.mainnet-beta.solana.com"
requests_per_second = 5

[storage]
db_path = "~/.local/share/delegation-oracle/oracle.db"

[programs]
enabled = ["sfdp", "marinade", "jpool", "blazestake", "jito", "sanctum"]

[analysis]
vulnerability_margin_pct = 5.0
lookback_epochs = 20
drift_check_interval_hours = 6

[optimizer]
revenue_per_sol_per_epoch = 0.0001
monthly_infra_cost_usd = 800.0

[alerts]
discord_webhook = ""
telegram_bot_token = ""
telegram_chat_id = ""
enable_stdout = true

[alerts.rules]
criteria_drift = true
vulnerability_detected = true
eligibility_lost = true
eligibility_gained = true
"#;
        template.to_string()
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            validator: ValidatorConfig::default(),
            rpc: RpcConfig::default(),
            storage: StorageConfig::default(),
            programs: ProgramsConfig::default(),
            analysis: AnalysisConfig::default(),
            optimizer: OptimizerConfig::default(),
            alerts: AlertsConfig::default(),
        }
    }
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            url: default_rpc_url(),
            requests_per_second: default_requests_per_second(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
        }
    }
}

impl Default for ProgramsConfig {
    fn default() -> Self {
        Self {
            enabled: default_programs_enabled(),
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            vulnerability_margin_pct: default_vulnerability_margin(),
            lookback_epochs: default_lookback_epochs(),
            drift_check_interval_hours: default_drift_hours(),
        }
    }
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            revenue_per_sol_per_epoch: default_revenue_per_sol_per_epoch(),
            monthly_infra_cost_usd: default_monthly_infra_cost_usd(),
        }
    }
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            discord_webhook: String::new(),
            telegram_bot_token: String::new(),
            telegram_chat_id: String::new(),
            enable_stdout: default_enable_stdout(),
            rules: AlertRulesConfig::default(),
        }
    }
}

impl Default for AlertRulesConfig {
    fn default() -> Self {
        Self {
            criteria_drift: true,
            vulnerability_detected: true,
            eligibility_lost: true,
            eligibility_gained: true,
        }
    }
}

fn default_rpc_url() -> String {
    "https://api.mainnet-beta.solana.com".to_string()
}

fn default_requests_per_second() -> u32 {
    5
}

fn default_db_path() -> String {
    "~/.local/share/delegation-oracle/oracle.db".to_string()
}

fn default_programs_enabled() -> Vec<String> {
    vec![
        "sfdp".to_string(),
        "marinade".to_string(),
        "jpool".to_string(),
        "blazestake".to_string(),
        "jito".to_string(),
        "sanctum".to_string(),
    ]
}

fn default_vulnerability_margin() -> f64 {
    5.0
}

fn default_lookback_epochs() -> u32 {
    20
}

fn default_drift_hours() -> u32 {
    6
}

fn default_revenue_per_sol_per_epoch() -> f64 {
    0.0001
}

fn default_monthly_infra_cost_usd() -> f64 {
    800.0
}

fn default_enable_stdout() -> bool {
    true
}

fn default_true() -> bool {
    true
}

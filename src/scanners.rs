//! Program scanners - each scanner queries a specific delegation program

use anyhow::Result;
use chrono::Utc;
use serde_json::json;

use crate::types::*;

const SOL_PRICE_USD: f64 = 200.0; // TODO: Fetch live price

/// Scan a validator across all (or specific) programs
pub async fn scan_validator(validator: &str, program: Option<&str>) -> Result<ScanResult> {
    let programs = match program {
        Some("marinade") => vec![scan_marinade(validator).await?],
        Some("jito") => vec![scan_jito(validator).await?],
        Some("blaze") => vec![scan_blaze(validator).await?],
        Some("sanctum") => vec![scan_sanctum(validator).await?],
        Some("sfdp") => vec![scan_sfdp(validator).await?],
        Some(p) => anyhow::bail!("Unknown program: {}", p),
        None => {
            // Scan all programs concurrently
            let (marinade, jito, blaze, sanctum, sfdp) = tokio::join!(
                scan_marinade(validator),
                scan_jito(validator),
                scan_blaze(validator),
                scan_sanctum(validator),
                scan_sfdp(validator),
            );
            vec![
                marinade?,
                jito?,
                blaze?,
                sanctum?,
                sfdp?,
            ]
        }
    };
    
    // Calculate summary
    let total_current: f64 = programs.iter().map(|p| p.current_stake_sol).sum();
    let total_potential: f64 = programs.iter().map(|p| p.potential_stake_sol).sum();
    let missed = total_potential - total_current;
    
    // Generate action items for programs with gaps
    let action_items: Vec<ActionItem> = programs
        .iter()
        .filter(|p| p.gap_sol > 0.0 && p.status != RegistrationStatus::Ineligible)
        .map(|p| ActionItem {
            program: p.name.clone(),
            action: format!("Register with {}", p.display_name),
            potential_gain_sol: p.gap_sol,
            url: p.registration_url.clone(),
            difficulty: match p.name.as_str() {
                "jito" => Difficulty::Easy,
                "marinade" => Difficulty::Easy,
                "blaze" => Difficulty::Medium,
                "sanctum" => Difficulty::Medium,
                "sfdp" => Difficulty::Hard,
                _ => Difficulty::Medium,
            },
        })
        .collect();
    
    Ok(ScanResult {
        validator: validator.to_string(),
        scanned_at: Utc::now(),
        programs,
        summary: ScanSummary {
            total_current_sol: total_current,
            total_potential_sol: total_potential,
            missed_revenue_sol: missed,
            missed_revenue_usd: missed * SOL_PRICE_USD,
            action_items,
        },
    })
}

/// Scan Marinade Finance
async fn scan_marinade(validator: &str) -> Result<ProgramStatus> {
    let client = reqwest::Client::new();
    
    let resp = client
        .get("https://validators-api.marinade.finance/validators")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    
    if !resp.status().is_success() {
        return Ok(ProgramStatus::new("marinade", "Marinade")
            .with_status(RegistrationStatus::Unknown));
    }
    
    let validators: Vec<serde_json::Value> = resp.json().await?;
    
    // Find our validator
    let found = validators.iter().find(|v| {
        v.get("vote_account")
            .and_then(|v| v.as_str())
            .map(|s| s == validator)
            .unwrap_or(false)
    });
    
    match found {
        Some(v) => {
            let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            let stake = v.get("marinade_stake").and_then(|s| s.as_f64()).unwrap_or(0.0);
            let eligible = v.get("eligible_stake_algo").and_then(|s| s.as_bool()).unwrap_or(false);
            
            Ok(ProgramStatus::new("marinade", "Marinade")
                .with_status(if stake > 0.0 { RegistrationStatus::Active } 
                             else if eligible { RegistrationStatus::Eligible }
                             else { RegistrationStatus::Ineligible })
                .with_stake(stake, stake) // Already receiving what they're eligible for
                .with_details(json!({
                    "score": score,
                    "eligible": eligible,
                })))
        }
        None => {
            // Not in Marinade set - estimate potential
            Ok(ProgramStatus::new("marinade", "Marinade")
                .with_status(RegistrationStatus::NotRegistered)
                .with_stake(0.0, 500.0) // Estimate for new validators
                .with_registration_url("https://marinade.finance/validators")
                .with_details(json!({
                    "note": "Validator not found in Marinade set"
                })))
        }
    }
}

/// Scan Jito StakeNet
async fn scan_jito(validator: &str) -> Result<ProgramStatus> {
    // TODO: Implement actual Jito API call
    // For now, return a placeholder that indicates checking is needed
    
    Ok(ProgramStatus::new("jito", "Jito StakeNet")
        .with_status(RegistrationStatus::Unknown)
        .with_stake(0.0, 800.0) // Estimate based on typical Jito stake
        .with_registration_url("https://jito.network/stakenet")
        .with_details(json!({
            "note": "Manual verification required - check jito.network",
            "mev_share_pct": 8,
        })))
}

/// Scan SolBlaze
async fn scan_blaze(validator: &str) -> Result<ProgramStatus> {
    // TODO: Implement actual Blaze API call
    
    Ok(ProgramStatus::new("blaze", "SolBlaze")
        .with_status(RegistrationStatus::Unknown)
        .with_stake(0.0, 400.0) // Estimate
        .with_registration_url("https://stake.solblaze.org")
        .with_details(json!({
            "note": "Manual verification required - check stake.solblaze.org"
        })))
}

/// Scan Sanctum Gauge
async fn scan_sanctum(validator: &str) -> Result<ProgramStatus> {
    // TODO: Implement Sanctum API/on-chain check
    
    Ok(ProgramStatus::new("sanctum", "Sanctum Gauge")
        .with_status(RegistrationStatus::Unknown)
        .with_stake(0.0, 1000.0) // Estimate based on gauge participation
        .with_registration_url("https://app.sanctum.so")
        .with_details(json!({
            "note": "Check Sanctum validator portal for gauge eligibility"
        })))
}

/// Scan Solana Foundation Delegation Program
async fn scan_sfdp(validator: &str) -> Result<ProgramStatus> {
    // TODO: Check on-chain SFDP status
    
    Ok(ProgramStatus::new("sfdp", "SFDP")
        .with_status(RegistrationStatus::Unknown)
        .with_stake(0.0, 25000.0) // SFDP delegations are typically large
        .with_registration_url("https://solana.org/delegation-program")
        .with_details(json!({
            "note": "Check Solana Foundation for delegation status"
        })))
}

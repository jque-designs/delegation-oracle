# Delegation Oracle - Implementation Spec

## Overview

Build a Rust CLI + REST API that scans a Solana validator's eligibility across all major stake delegation programs and quantifies missed revenue.

## Core Concept

Validators leave money on the table by not being registered with all available stake programs. This tool:
1. Checks eligibility for each program
2. Shows current vs potential stake
3. Calculates the revenue gap
4. Provides actionable registration links

## Programs to Support

### 1. Marinade Finance
**API:** `https://validators-api.marinade.finance/validators`
**What to check:**
- Is validator in the set?
- What's their score?
- Are they MNDE-eligible?
- Current stake from Marinade

**Eligibility criteria:**
- Commission ≤ 10%
- Uptime > 95%
- No concentration issues

### 2. Jito StakeNet
**API:** `https://jito.network/api/v1/stakenet/validators` (verify endpoint)
**What to check:**
- Is validator registered?
- MEV share percentage
- Current Jito stake

**Registration:** https://jito.network/stakenet

### 3. Blaze Stake (SolBlaze)
**API:** `https://stake.solblaze.org/api/v1/validators` (verify endpoint)
**What to check:**
- Is validator in bSOL set?
- BLZE reward eligibility
- Current stake

**Registration:** Apply via SolBlaze Discord/form

### 4. Sanctum (LST Gauge)
**Data:** On-chain gauge program + Sanctum API
**What to check:**
- Is validator in vSOL gauge?
- Current veVOTE weight
- Projected gauge stake

**Registration:** Via Sanctum validator portal

### 5. Solana Foundation Delegation Program (SFDP)
**Data:** On-chain + Foundation records
**What to check:**
- Active SFDP delegation?
- Delegation tier
- Compliance status

**Registration:** https://solana.org/delegation-program

## Data Structures

```rust
#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub enum RegistrationStatus {
    Active,           // Registered and receiving stake
    Eligible,         // Could register but hasn't
    NotRegistered,    // Not in program
    Ineligible,       // Doesn't meet requirements
    Unknown,          // Couldn't determine
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub validator: String,
    pub scanned_at: DateTime<Utc>,
    pub programs: Vec<ProgramStatus>,
    pub summary: ScanSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanSummary {
    pub total_current_sol: f64,
    pub total_potential_sol: f64,
    pub missed_revenue_sol: f64,
    pub missed_revenue_usd: f64,
    pub action_items: Vec<ActionItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionItem {
    pub program: String,
    pub action: String,
    pub potential_gain_sol: f64,
    pub url: Option<String>,
    pub difficulty: Difficulty,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,    // Just click register
    Medium,  // Fill out application
    Hard,    // Requires approval process
}
```

## API Endpoints

### `GET /api/health`
Health check.

### `GET /api/programs`
List all supported programs with metadata.

### `GET /api/scan?validator=<PUBKEY>`
Full scan of validator across all programs.

### `GET /api/scan?validator=<PUBKEY>&program=marinade`
Scan specific program only.

## CLI Commands

```bash
# Full scan
delegation-oracle scan <VALIDATOR_PUBKEY>

# Specific program
delegation-oracle scan <VALIDATOR_PUBKEY> --program marinade

# Output formats
delegation-oracle scan <VALIDATOR_PUBKEY> --output json
delegation-oracle scan <VALIDATOR_PUBKEY> --output table

# Start API server
delegation-oracle serve --port 3003
```

## Implementation Steps

### Phase 1: Core Engine
1. Create Rust project structure
2. Implement data types
3. Add HTTP client with retry logic
4. Create trait `ProgramScanner` for each program

### Phase 2: Program Scanners
1. Implement `MarinadeScanner`
2. Implement `JitoScanner`
3. Implement `BlazeScanner`
4. Implement `SanctumScanner`
5. Implement `SfdpScanner`

### Phase 3: CLI
1. Add clap for argument parsing
2. Implement `scan` command
3. Implement `serve` command
4. Add table/json output formatting

### Phase 4: REST API
1. Add Axum server
2. Implement endpoints
3. Add CORS for web UI
4. Add rate limiting

### Phase 5: Web UI (Next.js)
1. Create scan form
2. Display results as cards
3. Show action items prominently
4. Add "copy to clipboard" for registration links

## Estimating Potential Stake

For programs where validator isn't registered, estimate potential based on:

**Marinade:** Use their scoring formula
- Get validator's theoretical score
- Compare to similar validators' actual stake
- Estimate: `avg_stake_for_score_range`

**Jito:** Based on validator size
- Small validators: ~5% of current stake as Jito stake
- Medium: ~8%
- Large: ~10%

**Blaze:** Fixed tiers
- If eligible: estimate 200-500 SOL based on tier

**Sanctum:** Based on gauge voting
- Calculate what stake you'd get with minimum veV purchase

## Error Handling

- API timeouts: Return `Unknown` status with error message
- Invalid pubkey: Return 400 error
- Rate limits: Implement backoff and caching

## Caching

- Cache program data for 5 minutes
- Cache full validator scans for 1 minute
- Use SQLite for persistence

## Testing

1. Unit tests for each scanner
2. Integration tests with mock APIs
3. E2E test with real Solana mainnet

## Deployment

- Build as static binary
- Deploy API to same server as other LSF tools
- Port 3003
- Add to LSF tools test page

## Success Metrics

- Scan completes in < 5 seconds
- All 5 programs checked
- Clear actionable output
- Revenue gap quantified in SOL and USD

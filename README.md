# DELEGATION-ORACLE: Unified Multi-Program Eligibility Intelligence

> **Status:** Spec v1.0 — Codex-ready
> **Crate:** `delegation-oracle`
> **Binary:** `delegation-oracle`
> **License:** MIT/Apache-2.0

## What This Is

A single pane of glass across **every** Solana delegation program that answers one question: **"How do I maximize the stake I receive from automated programs?"**

Today, validators manually check 5-6 different dashboards, each with different criteria, different scoring, and different update cadences. Delegation-Oracle unifies them, cross-references them, and finds the **arbitrage** — programs you're missing by a hair, competitors about to fall out, criteria that just changed.

Nothing like this exists. Every current tool tracks one program at a time.

---

## Novel Differentiators

1. **Eligibility Arbitrage Engine** — Computes the exact delta between your current metrics and each program's thresholds. Ranks programs by "effort to qualify" so you fix the cheapest gaps first.
2. **Competitor Vulnerability Radar** — Identifies validators within each program whose metrics are deteriorating toward disqualification. Their loss is your gain.
3. **Criteria Drift Detection** — Programs change rules without warning. The oracle diffs each program's criteria against its last-known state and fires alerts on any change.
4. **What-If Optimizer** — Simulates metric changes ("lower commission by 0.5%", "improve skip rate by 1%") and shows cascading eligibility impact across all programs simultaneously.
5. **Historical Eligibility Timeline** — Tracks who was eligible for what, when, and correlates eligibility changes with stake flow events.
6. **Cross-Program Conflict Detection** — Some optimizations for one program hurt you in another (e.g., lowering commission helps BlazeStake but reduces revenue needed for infra upgrades that affect performance scores). The oracle surfaces these trade-offs.

---

## Architecture

```
delegation-oracle/
├── Cargo.toml
├── src/
│   ├── main.rs                     # CLI entrypoint
│   ├── lib.rs                      # Public API surface
│   ├── config.rs                   # TOML config + CLI merge
│   ├── programs/
│   │   ├── mod.rs                  # Program trait + registry
│   │   ├── sfdp.rs                 # Solana Foundation Delegation Program
│   │   ├── marinade.rs             # Marinade native staking
│   │   ├── jpool.rs                # JPool delegation
│   │   ├── blazestake.rs           # BlazeStake delegation
│   │   ├── jito.rs                 # JitoSOL validator set
│   │   └── sanctum.rs              # Sanctum LST programs
│   ├── criteria/
│   │   ├── mod.rs
│   │   ├── schema.rs               # Unified criteria model
│   │   ├── fetcher.rs              # Pull criteria from APIs/docs
│   │   ├── differ.rs               # Detect criteria changes
│   │   └── store.rs                # SQLite criteria history
│   ├── eligibility/
│   │   ├── mod.rs
│   │   ├── evaluator.rs            # Evaluate validator against criteria
│   │   ├── arbitrage.rs            # Gap analysis + ranking
│   │   ├── vulnerability.rs        # Competitor disqualification radar
│   │   └── history.rs              # Eligibility timeline tracking
│   ├── optimizer/
│   │   ├── mod.rs
│   │   ├── whatif.rs               # What-if simulation engine
│   │   ├── conflicts.rs            # Cross-program conflict detection
│   │   └── recommendations.rs      # Actionable optimization plan
│   ├── metrics/
│   │   ├── mod.rs
│   │   ├── collector.rs            # Unified metric collection
│   │   ├── normalize.rs            # Cross-program metric normalization
│   │   └── cache.rs                # Metric snapshot cache
│   ├── alert/
│   │   ├── mod.rs
│   │   ├── engine.rs               # Alert evaluation
│   │   ├── rules.rs                # Alert rule definitions
│   │   └── sink.rs                 # Webhook, Discord, stdout
│   ├── snapshot/
│   │   ├── mod.rs
│   │   ├── store.rs                # SQLite state persistence
│   │   └── migrations.rs           # Schema versioning
│   └── output/
│       ├── mod.rs
│       ├── table.rs                # Terminal rendering
│       ├── json.rs                 # JSON output
│       └── csv.rs                  # CSV export
```

### Key Dependencies

```toml
[dependencies]
solana-client = "2.1"
solana-sdk = "2.1"
solana-account-decoder = "2.1"
borsh = "1.5"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
clap = { version = "4", features = ["derive"] }
comfy-table = "7"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
similar = "2"              # text diffing for criteria drift
```

---

## Data Types

### Program Abstraction

```rust
/// Every delegation program implements this trait
#[async_trait]
pub trait DelegationProgram: Send + Sync {
    fn id(&self) -> ProgramId;
    fn name(&self) -> &str;

    /// Fetch the current criteria set from the program's source of truth
    async fn fetch_criteria(&self) -> Result<CriteriaSet>;

    /// Fetch the current eligible validator set
    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>>;

    /// Evaluate a single validator against this program's criteria
    fn evaluate(&self, validator: &ValidatorMetrics, criteria: &CriteriaSet) -> EligibilityResult;

    /// Estimate delegation amount if eligible
    fn estimate_delegation(&self, validator: &ValidatorMetrics, criteria: &CriteriaSet) -> Option<f64>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProgramId {
    Sfdp,
    Marinade,
    JPool,
    BlazeStake,
    Jito,
    Sanctum,
}
```

### Criteria Model

```rust
/// A set of criteria for one delegation program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaSet {
    pub program: ProgramId,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub source_url: String,
    pub criteria: Vec<Criterion>,
    pub raw_hash: String,                      // SHA-256 of raw source for drift detection
}

/// A single eligibility criterion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Criterion {
    pub name: String,
    pub metric: MetricKey,
    pub constraint: Constraint,
    pub weight: Option<f64>,                   // for scored programs (not pass/fail)
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Min(f64),                                  // metric >= value
    Max(f64),                                  // metric <= value
    Range { min: f64, max: f64 },
    Equals(String),                            // exact match (e.g., version)
    OneOf(Vec<String>),                        // must be one of these
    Boolean(bool),                             // must be true/false
    Custom(String),                            // human-readable, not auto-evaluated
}

/// Standardized metric keys across all programs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
```

### Eligibility & Gap Analysis

```rust
/// Result of evaluating one validator against one program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityResult {
    pub program: ProgramId,
    pub eligible: bool,
    pub score: Option<f64>,                    // for scored programs
    pub criterion_results: Vec<CriterionResult>,
    pub estimated_delegation_sol: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion_name: String,
    pub metric_key: MetricKey,
    pub your_value: MetricValue,
    pub required: Constraint,
    pub passed: bool,
    pub gap: Option<GapDetail>,                // None if passed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Numeric(f64),
    Text(String),
    Bool(bool),
}

/// How far you are from meeting a criterion you're failing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapDetail {
    pub metric_key: MetricKey,
    pub current_value: f64,
    pub required_value: f64,
    pub delta: f64,
    pub effort_estimate: EffortLevel,          // how hard is this to fix?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EffortLevel {
    Trivial,    // config change (e.g., lower commission)
    Moderate,   // operational change (e.g., improve skip rate)
    Hard,       // infrastructure change (e.g., relocate datacenter)
    Impossible, // can't be changed (e.g., stake too high for small-validator program)
}
```

### Arbitrage & Optimization

```rust
/// A program you could qualify for with specific changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub program: ProgramId,
    pub current_eligible: bool,
    pub gaps: Vec<GapDetail>,
    pub total_effort: EffortLevel,             // worst effort among gaps
    pub estimated_delegation_gain_sol: f64,
    pub roi_score: f64,                        // delegation_gain / effort
}

/// What-if simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfResult {
    pub changes_applied: Vec<MetricChange>,
    pub before: Vec<EligibilityResult>,        // per-program
    pub after: Vec<EligibilityResult>,         // per-program
    pub programs_gained: Vec<ProgramId>,
    pub programs_lost: Vec<ProgramId>,
    pub net_delegation_change_sol: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricChange {
    pub metric: MetricKey,
    pub from: f64,
    pub to: f64,
}

/// Cross-program conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramConflict {
    pub metric: MetricKey,
    pub program_a: ProgramId,
    pub program_a_wants: Constraint,
    pub program_b: ProgramId,
    pub program_b_wants: Constraint,
    pub conflict_type: ConflictType,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    DirectContradiction,  // one wants min, other wants max
    TensionZone,          // both achievable but narrow window
    IndirectImpact,       // optimizing A degrades a metric B cares about
}
```

### Competitor Vulnerability

```rust
/// A validator at risk of losing eligibility in a program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerableValidator {
    pub vote_pubkey: Pubkey,
    pub program: ProgramId,
    pub metrics_at_risk: Vec<AtRiskMetric>,
    pub epochs_until_likely_loss: Option<u32>,
    pub current_delegation_sol: f64,           // stake that would be redistributed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtRiskMetric {
    pub metric: MetricKey,
    pub current_value: f64,
    pub threshold: f64,
    pub margin: f64,               // how close to failing (negative = already failing)
    pub trend: TrendDirection,     // is it getting worse?
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Deteriorating,
}
```

### Criteria Drift

```rust
/// A detected change in a program's criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaDrift {
    pub program: ProgramId,
    pub detected_at: chrono::DateTime<chrono::Utc>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Removed,
    ThresholdChanged,
    WeightChanged,
    DescriptionChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DriftImpact {
    NowEligible,          // criteria relaxed in your favor
    StillEligible,        // no effect on you
    AtRisk,               // you were passing, now marginal
    NowIneligible,        // criteria tightened past your metrics
    NotApplicable,        // you weren't in this program anyway
}

/// Historical eligibility record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityRecord {
    pub vote_pubkey: Pubkey,
    pub program: ProgramId,
    pub epoch: u64,
    pub eligible: bool,
    pub score: Option<f64>,
    pub delegation_sol: Option<f64>,
}
```

---

## Data Sources & API Calls

### On-Chain (Solana RPC)

| RPC Method | Purpose | Frequency |
|---|---|---|
| `getVoteAccounts` | Validator metrics: stake, commission, credits, skip rate | Every epoch + hourly |
| `getEpochInfo` | Epoch boundary detection | Every 30s |
| `getVersion` | Node version per validator | Every epoch |
| `getClusterNodes` | Datacenter/ASN inference | Every epoch |

### Delegation Program APIs

| Program | Endpoint | Data Pulled |
|---|---|---|
| **SFDP** | `https://kyc-api.vercel.app/api/validators/list` | Eligible set, program rules |
| **SFDP** | GitHub: `solana-labs/stake-o-matic` config | Criteria thresholds (machine-readable) |
| **Marinade** | `https://validators-api.marinade.finance/validators` | Scores, weights, eligible set |
| **Marinade** | `https://scoring.marinade.finance/v1/scores` | Individual scoring components |
| **Marinade** | GitHub: `marinade-finance/delegation-strategy-2` | Scoring algorithm source of truth |
| **JPool** | `https://api.jpool.one/validators` | Eligible set, scoring |
| **BlazeStake** | `https://stake.solblaze.org/api/v1/cls_validators` | Eligible set, criteria |
| **BlazeStake** | GitHub: `solblaze-org/stake-pool-cli` | Criteria implementation |
| **Jito** | `https://kobe.mainnet.jito.network/api/v1/validators` | MEV commission, stake set |
| **Sanctum** | `https://sanctum-s-api.fly.dev/v1/validator/list` | LST validator sets |

### Supplementary

| Source | Purpose |
|---|---|
| **validators.app API** | Datacenter mapping, version data, supplementary metrics |
| **StakeWiz API** | Validator metadata, name resolution |
| Local SQLite | Criteria history, eligibility timeline, metric snapshots |

---

## CLI Design

```
delegation-oracle — Unified delegation program intelligence

USAGE:
    delegation-oracle [OPTIONS] <COMMAND>

COMMANDS:
    status        Show eligibility status across all programs
    gaps          Show what you're missing for each program
    arbitrage     Rank programs by ease-of-qualification
    whatif         Simulate metric changes across all programs
    vulnerable    Find competitors about to lose eligibility
    drift         Check for criteria changes across programs
    history       View eligibility timeline
    optimize      Generate actionable optimization plan
    watch         Continuous monitoring with alerts
    config        Manage configuration

OPTIONS:
    -v, --validator <VOTE_PUBKEY>    Your validator vote account
    -c, --config <PATH>             Config file [default: ~/.config/delegation-oracle/config.toml]
    -r, --rpc <URL>                 Solana RPC endpoint
    -o, --output <FORMAT>           Output: table, json, csv [default: table]
    -p, --programs <LIST>           Filter to specific programs [default: all]
```

### Example Commands

```bash
# Am I eligible everywhere? Where am I not?
delegation-oracle status -v <VOTE_KEY>

# What exactly am I failing for each program?
delegation-oracle gaps -v <VOTE_KEY>

# Which programs are easiest to qualify for?
delegation-oracle arbitrage -v <VOTE_KEY> --sort roi

# What if I drop commission from 5% to 4%?
delegation-oracle whatif -v <VOTE_KEY> --commission 4

# What if I improve skip rate AND lower commission?
delegation-oracle whatif -v <VOTE_KEY> --commission 4 --skip-rate 1.5

# Who is about to lose Marinade eligibility?
delegation-oracle vulnerable --program marinade --margin 5

# Did any program change their rules recently?
delegation-oracle drift --since 5

# Show my eligibility history over last 50 epochs
delegation-oracle history -v <VOTE_KEY> --epochs 50

# Give me a prioritized action plan
delegation-oracle optimize -v <VOTE_KEY>

# Continuous monitoring
delegation-oracle watch -v <VOTE_KEY> --alert-webhook https://discord.com/api/webhooks/...
```

### Sample Output: `status`

```
╔══════════════════════════════════════════════════════════════════════════════╗
║              DELEGATION ORACLE — MULTI-PROGRAM STATUS                      ║
║  Validator: LSF (7xKp..3nRd)                    Epoch: 742                 ║
╠══════════════════════════════════════════════════════════════════════════════╣
║ PROGRAM     │ ELIGIBLE │ SCORE  │ DELEGATION (SOL) │ CRITERIA MET         ║
╠═════════════╪══════════╪════════╪══════════════════╪══════════════════════╣
║ SFDP        │ ✓ YES    │  —     │ 50,000           │ 6/6                  ║
║ Marinade    │ ✓ YES    │ 0.823  │ 38,200           │ 8/8                  ║
║ JPool       │ ✓ YES    │ 0.791  │ 12,400           │ 5/5                  ║
║ BlazeStake  │ ✗ NO     │ 0.680  │ —                │ 4/6 (2 failing)      ║
║ Jito        │ ✓ YES    │ —      │ 8,100            │ 3/3                  ║
║ Sanctum     │ ✗ NO     │ —      │ —                │ 2/4 (2 failing)      ║
╠═════════════╧══════════╧════════╧══════════════════╧══════════════════════╣
║ TOTAL DELEGATED: 108,700 SOL    POTENTIAL IF FULLY ELIGIBLE: ~142,500 SOL ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

### Sample Output: `gaps`

```
╔══════════════════════════════════════════════════════════════════════════════╗
║              DELEGATION ORACLE — GAP ANALYSIS                              ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                            ║
║  BlazeStake (2 gaps):                                                      ║
║  ├─ Skip Rate: yours 3.2%, required ≤2.5%  [gap: 0.7%]  effort: MODERATE  ║
║  └─ DC Diversity: yours ASN16509, need non-AWS  [gap: relocation]  HARD    ║
║                                                                            ║
║  Sanctum (2 gaps):                                                         ║
║  ├─ Min Stake: yours 156K, required ≥200K  [gap: 44K SOL]  MODERATE       ║
║  └─ MEV Commission: yours 8%, required ≤5%  [gap: 3%]  effort: TRIVIAL    ║
║                                                                            ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

### Sample Output: `arbitrage`

```
╔══════════════════════════════════════════════════════════════════════════════╗
║              DELEGATION ORACLE — ELIGIBILITY ARBITRAGE                     ║
║  Ranked by ROI (delegation gain per unit effort)                           ║
╠══════════════════════════════════════════════════════════════════════════════╣
║ RANK │ PROGRAM    │ EST. DELEGATION │ EFFORT   │ ACTION NEEDED             ║
╠══════╪════════════╪═════════════════╪══════════╪═══════════════════════════╣
║  1   │ Sanctum    │ +18,200 SOL     │ TRIVIAL  │ Lower MEV commission to 5%║
║  2   │ BlazeStake │ +15,600 SOL     │ MODERATE │ Improve skip rate to ≤2.5%║
╠══════╧════════════╧═════════════════╧══════════╧═══════════════════════════╣
║  NOTE: Sanctum also requires 200K min stake (currently 156K).              ║
║  Fixing MEV commission alone won't qualify — need both.                    ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

### Sample Output: `whatif`

```
$ delegation-oracle whatif -v <VOTE_KEY> --commission 4 --mev-commission 5

╔══════════════════════════════════════════════════════════════════════════════╗
║              DELEGATION ORACLE — WHAT-IF SIMULATION                        ║
║  Changes: commission 5%→4%, MEV commission 8%→5%                           ║
╠══════════════════════════════════════════════════════════════════════════════╣
║ PROGRAM     │ BEFORE    │ AFTER     │ CHANGE                               ║
╠═════════════╪═══════════╪═══════════╪══════════════════════════════════════╣
║ SFDP        │ ✓ 50,000  │ ✓ 50,000  │ —                                   ║
║ Marinade    │ ✓ 38,200  │ ✓ 41,800  │ +3,600 SOL (score ↑ 0.823→0.851)   ║
║ JPool       │ ✓ 12,400  │ ✓ 13,100  │ +700 SOL                            ║
║ BlazeStake  │ ✗ —       │ ✗ —       │ still failing skip rate              ║
║ Jito        │ ✓ 8,100   │ ✓ 8,100   │ —                                   ║
║ Sanctum     │ ✗ —       │ ✗ —       │ MEV ✓ now, still need 200K stake     ║
╠═════════════╧═══════════╧═══════════╧══════════════════════════════════════╣
║ NET IMPACT: +4,300 SOL delegation, 0 new programs (Sanctum partially fixed)║
║                                                                            ║
║ ⚠ CONFLICT: Lowering commission reduces revenue by ~$420/mo.              ║
║   Ensure this doesn't impact infra budget (affects performance scores).    ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

---

## Config File

```toml
[validator]
vote_pubkey = "YourVoteAccountPubkeyHere"

[rpc]
url = "https://api.mainnet-beta.solana.com"
requests_per_second = 5

[storage]
db_path = "~/.local/share/delegation-oracle/oracle.db"

[programs]
enabled = ["sfdp", "marinade", "jpool", "blazestake", "jito", "sanctum"]

[programs.sfdp]
criteria_source = "github"

[programs.marinade]
api_url = "https://validators-api.marinade.finance/validators"
scoring_url = "https://scoring.marinade.finance/v1/scores"

[programs.jpool]
api_url = "https://api.jpool.one/validators"

[programs.blazestake]
api_url = "https://stake.solblaze.org/api/v1/cls_validators"

[programs.jito]
api_url = "https://kobe.mainnet.jito.network/api/v1/validators"

[programs.sanctum]
api_url = "https://sanctum-s-api.fly.dev/v1/validator/list"

[analysis]
vulnerability_margin_pct = 5.0
lookback_epochs = 20
drift_check_interval_hours = 6

[optimizer]
revenue_per_sol_per_epoch = 0.0001    # for ROI calculations
monthly_infra_cost_usd = 800.0        # for conflict analysis

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
```

---

## Analysis Algorithms

### Eligibility Arbitrage

For each program you're NOT eligible for:
1. Enumerate failing criteria
2. For each failing criterion, compute the `GapDetail` (how far, what effort level)
3. Estimate the delegation you'd receive if eligible (from pool's published allocation model or historical averages)
4. Compute `roi_score = estimated_delegation / max(effort_levels)` — higher is better
5. Sort programs by ROI score descending
6. Flag dependencies: some gaps require multiple fixes simultaneously (e.g., Sanctum needs both MEV commission AND min stake)

### What-If Simulation

1. Clone your current `ValidatorMetrics`
2. Apply the user's proposed changes (commission, skip rate, etc.)
3. Re-evaluate against every program's `CriteriaSet`
4. Diff before/after eligibility and estimated delegation
5. Run conflict detection: check if any changed metric crosses a threshold in the wrong direction for any other program
6. Compute net delegation change and revenue impact

### Criteria Drift Detection

Each program's criteria is fetched periodically and hashed. When the hash changes:
1. Deserialize both old and new `CriteriaSet`
2. Diff at the criterion level: added, removed, threshold changed, weight changed
3. Re-evaluate your eligibility against the new criteria
4. Classify impact: `NowEligible`, `StillEligible`, `AtRisk`, `NowIneligible`
5. Fire alert with full diff and impact assessment

### Competitor Vulnerability

For each program's eligible set:
1. Fetch all eligible validators with their metrics
2. For each validator, compute margin to each criterion threshold: `margin = (value - threshold) / threshold`
3. Track margin over lookback window to determine trend
4. Flag validators where margin < `vulnerability_margin_pct` AND trend is `Deteriorating`
5. Estimate redistributable stake: if validator loses eligibility, their delegation gets redistributed among remaining eligible validators

### Cross-Program Conflict Detection

Build a metric constraint graph:
1. For each program, record what direction each metric needs to move (lower commission, higher uptime, etc.)
2. Detect direct contradictions (program A wants commission ≤ 3%, program B uses commission in scoring where higher commission = more revenue = better infra)
3. Detect tension zones (both achievable but the acceptable range is very narrow)
4. Detect indirect impacts (lowering commission reduces revenue, which might force infra downgrades, which affects performance metrics other programs care about)
5. Surface conflicts with concrete recommendations

---

## Implementation Phases

### Phase 1: Foundation
- `DelegationProgram` trait and registry pattern
- SFDP and Marinade implementations (most documented programs)
- Basic eligibility evaluation
- SQLite storage for metrics and eligibility snapshots
- CLI skeleton with `status` and `gaps` commands

### Phase 2: Multi-Program
- JPool, BlazeStake, Jito, Sanctum program implementations
- Unified metric collection across all programs
- Arbitrage engine with ROI scoring
- Table/JSON/CSV output renderers

### Phase 3: Intelligence
- What-if simulation engine
- Cross-program conflict detection
- Competitor vulnerability radar
- Criteria drift detection with hash-based diffing
- Historical eligibility timeline

### Phase 4: Operations
- `watch` mode with polling intervals per program
- Alert engine with per-program and cross-program rules
- Discord/Telegram alert sinks
- `optimize` command: full prioritized action plan generation
- Config migration and validation

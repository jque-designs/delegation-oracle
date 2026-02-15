# Delegation Oracle

**Multi-program delegation scanner for Solana validators**

Find missed revenue opportunities across stake pools and delegation programs.

## The Problem

Solana validators can earn stake from multiple programs:
- **Marinade** (native stake + mSOL)
- **Jito** (MEV rewards + jitoSOL)
- **Blaze** (bSOL + BLZE rewards)
- **Sanctum** (LST gauge voting)
- **Foundation Delegation Program** (SFDP)

Most validators are only registered with 1-2 programs, leaving money on the table.

## What This Tool Does

1. **Scans your validator** against all major stake programs
2. **Shows eligibility status** for each program
3. **Calculates missed revenue** from programs you're not in
4. **Provides registration links** to capture that revenue

## Example Output

```
Validator: Lua298Woc4rgcswL64yfWAL4EW44FgBZeLsKforf6tJ

┌─────────────────────────────────────────────────────────────────┐
│ PROGRAM          │ STATUS      │ CURRENT    │ POTENTIAL │ GAP  │
├─────────────────────────────────────────────────────────────────┤
│ Marinade         │ ✅ Active   │ 2,400 SOL  │ 2,400 SOL │ -    │
│ Jito             │ ❌ Not Reg  │ 0 SOL      │ ~800 SOL  │ +800 │
│ Blaze            │ ⚠️ Eligible │ 0 SOL      │ ~500 SOL  │ +500 │
│ Sanctum Gauge    │ ✅ Active   │ 1,200 SOL  │ 1,200 SOL │ -    │
│ SFDP             │ ✅ Active   │ 25K SOL    │ 25K SOL   │ -    │
└─────────────────────────────────────────────────────────────────┘

MISSED REVENUE: ~1,300 SOL/year ($260K at $200/SOL)

ACTION ITEMS:
1. Register with Jito StakeNet → [link]
2. Apply for Blaze delegation → [link]
```

## Architecture

```
┌─────────────────┐
│  CLI / Web UI   │
└────────┬────────┘
         │
┌────────▼────────┐
│  Oracle Engine  │ ← Queries all program APIs
└────────┬────────┘
         │
    ┌────┴────┬─────────┬─────────┬─────────┐
    ▼         ▼         ▼         ▼         ▼
┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐
│Marinade│ │ Jito  │ │ Blaze │ │Sanctum│ │ SFDP  │
└───────┘ └───────┘ └───────┘ └───────┘ └───────┘
```

## Data Sources

| Program | API/Data Source | What We Query |
|---------|-----------------|---------------|
| Marinade | `validators-api.marinade.finance` | Score, eligibility, current stake |
| Jito | `jito.network/stakenet` | Registration status, MEV share |
| Blaze | `stake.solblaze.org` | Validator set, BLZE rewards |
| Sanctum | `sanctum.so` | Gauge votes, vSOL eligibility |
| SFDP | On-chain program | Delegation status |

## Tech Stack

- **Rust** (core engine)
- **Axum** (REST API)
- **Next.js** (web UI, deployed to Vercel)

## Getting Started

```bash
# CLI
cargo run -- check <VALIDATOR_PUBKEY>

# API Server
cargo run -- serve --port 3003

# API Endpoints
GET /api/scan?validator=<PUBKEY>
GET /api/programs
GET /api/eligibility?validator=<PUBKEY>&program=marinade
```

## API Response

```json
{
  "validator": "Lua298...",
  "scannedAt": "2026-02-15T09:00:00Z",
  "programs": [
    {
      "name": "marinade",
      "status": "active",
      "currentStakeSol": 2400,
      "potentialStakeSol": 2400,
      "gapSol": 0,
      "registrationUrl": null,
      "details": {
        "score": 0.847,
        "rank": 142,
        "mndeEligible": true
      }
    },
    {
      "name": "jito",
      "status": "not_registered",
      "currentStakeSol": 0,
      "potentialStakeSol": 800,
      "gapSol": 800,
      "registrationUrl": "https://jito.network/stakenet/register",
      "details": {
        "mevSharePct": 8,
        "estimatedMevSol": 12
      }
    }
  ],
  "summary": {
    "totalCurrentSol": 28600,
    "totalPotentialSol": 29900,
    "missedRevenueSol": 1300,
    "missedRevenueUsd": 260000
  }
}
```

## Why This Matters

| Metric | Without Oracle | With Oracle |
|--------|----------------|-------------|
| Programs registered | 2-3 | 5+ |
| Annual missed revenue | $100K-500K | $0 |
| Time to find gaps | Hours of research | 30 seconds |

## License

MIT

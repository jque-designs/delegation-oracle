# AGENTS.md - Instructions for Codex

## Project Overview
Delegation Oracle scans a Solana validator's eligibility across all major stake delegation programs and quantifies missed revenue.

## What's Done
- ✅ Project structure (Cargo.toml, src/)
- ✅ CLI commands (scan, serve, programs)
- ✅ REST API with Axum (port 3003)
- ✅ Type definitions
- ✅ Marinade scanner (partially working)

## What Needs Building

### Priority 1: Complete Scanners
Each scanner in `src/scanners.rs` needs to query the actual program API:

1. **Marinade** (partially done) - Uses their public API
2. **Jito** - Find their StakeNet API endpoint
3. **Blaze** - Find SolBlaze validator API
4. **Sanctum** - Check gauge voting data
5. **SFDP** - Check on-chain delegation status

### Priority 2: Estimate Logic
For validators NOT registered with a program, we need to estimate potential stake:
- Use similar validators as reference
- Consider validator size/performance
- Don't just use arbitrary numbers

### Priority 3: Testing
- Add integration tests with real mainnet data
- Test with known validators (Lua Sol, Helius, etc.)

## API Endpoints to Verify
```
GET /api/health
GET /api/programs
GET /api/scan?validator=Lua298Woc4rgcswL64yfWAL4EW44FgBZeLsKforf6tJ
GET /api/scan?validator=<PUBKEY>&program=marinade
```

## Build & Run
```bash
cargo build --release
./target/release/delegation-oracle scan Lua298Woc4rgcswL64yfWAL4EW44FgBZeLsKforf6tJ
./target/release/delegation-oracle serve --port 3003
```

## Key Files
- `SPEC.md` - Full implementation specification
- `src/main.rs` - CLI entry point
- `src/scanners.rs` - **Where most work is needed**
- `src/api.rs` - REST API (mostly complete)
- `src/types.rs` - Data structures

## Notes
- Use `reqwest` for HTTP calls (already in Cargo.toml)
- Handle API failures gracefully (return Unknown status)
- Keep scan time under 5 seconds total
- Add CORS headers for web UI (already done)

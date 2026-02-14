pub const BASE_MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS criteria_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    program TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    raw_hash TEXT NOT NULL,
    source_url TEXT NOT NULL,
    criteria_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_criteria_program_fetched
    ON criteria_history(program, fetched_at DESC);

CREATE TABLE IF NOT EXISTS eligibility_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vote_pubkey TEXT NOT NULL,
    program TEXT NOT NULL,
    epoch INTEGER NOT NULL,
    eligible INTEGER NOT NULL,
    score REAL,
    delegation_sol REAL,
    captured_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_eligibility_vote_program_epoch
    ON eligibility_history(vote_pubkey, program, epoch DESC);
"#;

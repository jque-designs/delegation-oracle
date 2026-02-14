use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::criteria::schema::{CriteriaSet, ProgramId};

#[derive(Debug)]
pub struct CriteriaStore {
    conn: Connection,
}

impl CriteriaStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
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
"#,
        )?;
        Ok(())
    }

    pub fn insert_criteria(&self, set: &CriteriaSet) -> Result<()> {
        let criteria_json = serde_json::to_string(set)?;
        self.conn.execute(
            r#"
INSERT INTO criteria_history(program, fetched_at, raw_hash, source_url, criteria_json)
VALUES (?1, ?2, ?3, ?4, ?5)
"#,
            params![
                set.program.as_slug(),
                set.fetched_at.to_rfc3339(),
                set.raw_hash,
                set.source_url,
                criteria_json
            ],
        )?;
        Ok(())
    }

    pub fn latest_criteria(&self, program: ProgramId) -> Result<Option<CriteriaSet>> {
        let mut stmt = self.conn.prepare(
            r#"
SELECT criteria_json
FROM criteria_history
WHERE program = ?1
ORDER BY id DESC
LIMIT 1
"#,
        )?;
        let maybe_json: Option<String> = stmt
            .query_row(params![program.as_slug()], |row| row.get(0))
            .ok();
        match maybe_json {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
}

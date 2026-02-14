use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::criteria::{CriteriaSet, ProgramId};
use crate::eligibility::EligibilityRecord;
use crate::snapshot::migrations::BASE_MIGRATION;

pub struct SnapshotStore {
    conn: Connection,
}

impl SnapshotStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(BASE_MIGRATION)?;
        Ok(())
    }

    pub fn insert_criteria(&self, criteria: &CriteriaSet) -> Result<()> {
        self.conn.execute(
            r#"
INSERT INTO criteria_history(program, fetched_at, raw_hash, source_url, criteria_json)
VALUES (?1, ?2, ?3, ?4, ?5)
"#,
            params![
                criteria.program.as_slug(),
                criteria.fetched_at.to_rfc3339(),
                criteria.raw_hash,
                criteria.source_url,
                serde_json::to_string(criteria)?
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
        let result = stmt.query_row(params![program.as_slug()], |row| row.get::<_, String>(0));
        match result {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn insert_eligibility_record(&self, record: &EligibilityRecord) -> Result<()> {
        self.conn.execute(
            r#"
INSERT INTO eligibility_history(
    vote_pubkey, program, epoch, eligible, score, delegation_sol, captured_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
"#,
            params![
                record.vote_pubkey,
                record.program.as_slug(),
                record.epoch as i64,
                if record.eligible { 1 } else { 0 },
                record.score,
                record.delegation_sol,
                record.captured_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn load_history(
        &self,
        vote_pubkey: &str,
        program: Option<ProgramId>,
        limit: usize,
    ) -> Result<Vec<EligibilityRecord>> {
        let sql = if program.is_some() {
            r#"
SELECT vote_pubkey, program, epoch, eligible, score, delegation_sol, captured_at
FROM eligibility_history
WHERE vote_pubkey = ?1 AND program = ?2
ORDER BY epoch DESC, id DESC
LIMIT ?3
"#
        } else {
            r#"
SELECT vote_pubkey, program, epoch, eligible, score, delegation_sol, captured_at
FROM eligibility_history
WHERE vote_pubkey = ?1
ORDER BY epoch DESC, id DESC
LIMIT ?2
"#
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(program) = program {
            stmt.query_map(
                params![vote_pubkey, program.as_slug(), limit as i64],
                |row| row_to_eligibility_record(row),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![vote_pubkey, limit as i64], |row| {
                row_to_eligibility_record(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn next_epoch_hint(&self) -> Result<u64> {
        let mut stmt = self
            .conn
            .prepare("SELECT COALESCE(MAX(epoch), 0) FROM eligibility_history")?;
        let max_epoch: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok((max_epoch as u64) + 1)
    }
}

fn row_to_eligibility_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<EligibilityRecord> {
    let program_raw: String = row.get(1)?;
    let parsed_program = program_raw.parse::<ProgramId>().unwrap_or(ProgramId::Sfdp);
    let captured_at_raw: String = row.get(6)?;
    let captured_at = DateTime::parse_from_rfc3339(&captured_at_raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Ok(EligibilityRecord {
        vote_pubkey: row.get(0)?,
        program: parsed_program,
        epoch: row.get::<_, i64>(2)? as u64,
        eligible: row.get::<_, i64>(3)? != 0,
        score: row.get(4)?,
        delegation_sol: row.get(5)?,
        captured_at,
    })
}

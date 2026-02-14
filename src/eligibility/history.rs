use chrono::Utc;

use crate::criteria::ProgramId;
use crate::eligibility::{EligibilityRecord, EligibilityResult};

pub fn record_from_result(
    vote_pubkey: impl Into<String>,
    epoch: u64,
    result: &EligibilityResult,
) -> EligibilityRecord {
    EligibilityRecord {
        vote_pubkey: vote_pubkey.into(),
        program: result.program,
        epoch,
        eligible: result.eligible,
        score: result.score,
        delegation_sol: result.estimated_delegation_sol,
        captured_at: Utc::now(),
    }
}

pub fn summarize_timeline(records: &[EligibilityRecord], program: Option<ProgramId>) -> String {
    if records.is_empty() {
        return "No history records found.".to_string();
    }

    let mut eligible_count = 0usize;
    let mut total = 0usize;
    for rec in records {
        if let Some(program) = program {
            if rec.program != program {
                continue;
            }
        }
        total += 1;
        if rec.eligible {
            eligible_count += 1;
        }
    }

    if total == 0 {
        return "No matching records for selected program.".to_string();
    }

    format!(
        "Eligibility ratio: {eligible_count}/{total} ({:.1}%)",
        (eligible_count as f64 / total as f64) * 100.0
    )
}

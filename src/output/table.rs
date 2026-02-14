use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, ContentArrangement, Row, Table};

use crate::criteria::CriteriaDrift;
use crate::eligibility::{
    ArbitrageOpportunity, EligibilityRecord, EligibilityResult, VulnerableValidator,
};
use crate::optimizer::{OptimizationRecommendation, WhatIfResult};

pub fn render_status_table(results: &[EligibilityResult]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Program",
        "Eligible",
        "Score",
        "Delegation (SOL)",
        "Criteria Met",
    ]);

    for r in results {
        let elig = if r.eligible { "YES" } else { "NO" };
        let elig_cell = if r.eligible {
            Cell::new(elig).fg(Color::Green)
        } else {
            Cell::new(elig).fg(Color::Red)
        };
        table.add_row(Row::from(vec![
            Cell::new(r.program.to_string()),
            elig_cell,
            Cell::new(
                r.score
                    .map(|s| format!("{s:.3}"))
                    .unwrap_or_else(|| "-".to_string()),
            ),
            Cell::new(
                r.estimated_delegation_sol
                    .map(|v| format!("{v:.0}"))
                    .unwrap_or_else(|| "-".to_string()),
            ),
            Cell::new(format!(
                "{}/{}",
                r.passed_count(),
                r.criterion_results.len()
            )),
        ]));
    }
    table.to_string()
}

pub fn render_gaps_table(results: &[EligibilityResult]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Program",
        "Criterion",
        "Current",
        "Required",
        "Gap",
        "Effort",
    ]);
    for r in results {
        for c in &r.criterion_results {
            let Some(gap) = &c.gap else {
                continue;
            };
            table.add_row(vec![
                r.program.to_string(),
                c.criterion_name.clone(),
                format!("{:.3}", gap.current_value),
                format!("{:.3}", gap.required_value),
                format!("{:.3}", gap.delta),
                format!("{:?}", gap.effort_estimate).to_uppercase(),
            ]);
        }
    }
    table.to_string()
}

pub fn render_arbitrage_table(opps: &[ArbitrageOpportunity]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Rank",
        "Program",
        "Est. Delegation Gain",
        "Effort",
        "ROI",
        "Action Needed",
    ]);

    for (idx, opp) in opps.iter().enumerate() {
        let action = opp
            .gaps
            .iter()
            .map(|g| format!("{} {}", g.metric_key, g.required_value))
            .collect::<Vec<_>>()
            .join(", ");
        table.add_row(vec![
            (idx + 1).to_string(),
            opp.program.to_string(),
            format!("+{:.0} SOL", opp.estimated_delegation_gain_sol),
            format!("{:?}", opp.total_effort).to_uppercase(),
            format!("{:.2}", opp.roi_score),
            action,
        ]);
    }
    table.to_string()
}

pub fn render_whatif_table(result: &WhatIfResult) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Program", "Before", "After", "Change"]);

    for before in &result.before {
        if let Some(after) = result.after.iter().find(|a| a.program == before.program) {
            let before_label = if before.eligible {
                format!("YES {:.0}", before.estimated_delegation_sol.unwrap_or(0.0))
            } else {
                "NO -".to_string()
            };
            let after_label = if after.eligible {
                format!("YES {:.0}", after.estimated_delegation_sol.unwrap_or(0.0))
            } else {
                "NO -".to_string()
            };
            let delta = after.estimated_delegation_sol.unwrap_or(0.0)
                - before.estimated_delegation_sol.unwrap_or(0.0);
            table.add_row(vec![
                before.program.to_string(),
                before_label,
                after_label,
                format!("{delta:+.0} SOL"),
            ]);
        }
    }

    let mut footer = String::new();
    footer.push_str(&table.to_string());
    footer.push_str(&format!(
        "\nNet delegation impact: {:+.2} SOL\nPrograms gained: {:?}\nPrograms lost: {:?}",
        result.net_delegation_change_sol, result.programs_gained, result.programs_lost
    ));
    footer
}

pub fn render_vulnerability_table(items: &[VulnerableValidator]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Validator",
        "Program",
        "At-risk metrics",
        "Epochs to likely loss",
        "Delegation SOL",
    ]);
    for item in items {
        let metrics = item
            .metrics_at_risk
            .iter()
            .map(|m| format!("{} ({:.2}% margin)", m.metric, m.margin))
            .collect::<Vec<_>>()
            .join(", ");
        table.add_row(vec![
            item.vote_pubkey.clone(),
            item.program.to_string(),
            metrics,
            item.epochs_until_likely_loss
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string()),
            format!("{:.0}", item.current_delegation_sol),
        ]);
    }
    table.to_string()
}

pub fn render_drift_table(drifts: &[CriteriaDrift]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Program", "Detected at", "Impact", "Changes"]);
    for drift in drifts {
        let changes = drift
            .changes
            .iter()
            .map(|c| format!("{}:{:?}", c.criterion_name, c.change_type))
            .collect::<Vec<_>>()
            .join(", ");
        table.add_row(vec![
            drift.program.to_string(),
            drift.detected_at.to_rfc3339(),
            format!("{:?}", drift.impact_on_you),
            changes,
        ]);
    }
    table.to_string()
}

pub fn render_history_table(records: &[EligibilityRecord]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Captured At",
        "Epoch",
        "Program",
        "Eligible",
        "Score",
        "Delegation SOL",
    ]);
    for rec in records {
        table.add_row(vec![
            rec.captured_at.to_rfc3339(),
            rec.epoch.to_string(),
            rec.program.to_string(),
            rec.eligible.to_string(),
            rec.score
                .map(|v| format!("{v:.3}"))
                .unwrap_or_else(|| "-".to_string()),
            rec.delegation_sol
                .map(|v| format!("{v:.0}"))
                .unwrap_or_else(|| "-".to_string()),
        ]);
    }
    table.to_string()
}

pub fn render_recommendations_table(items: &[OptimizationRecommendation]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Priority",
        "Title",
        "Effort",
        "Expected Gain",
        "Rationale",
    ]);
    for item in items {
        table.add_row(vec![
            item.priority.to_string(),
            item.title.clone(),
            item.effort.clone(),
            format!("{:.0}", item.expected_gain_sol),
            item.rationale.clone(),
        ]);
    }
    table.to_string()
}

use anyhow::Result;

use crate::eligibility::{ArbitrageOpportunity, EligibilityResult};

pub fn status_to_csv(results: &[EligibilityResult]) -> Result<String> {
    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record([
        "program",
        "eligible",
        "score",
        "delegation_sol",
        "criteria_passed",
        "criteria_total",
    ])?;
    for result in results {
        writer.write_record([
            result.program.to_string(),
            result.eligible.to_string(),
            result.score.map(|s| format!("{s:.4}")).unwrap_or_default(),
            result
                .estimated_delegation_sol
                .map(|d| format!("{d:.2}"))
                .unwrap_or_default(),
            result.passed_count().to_string(),
            result.criterion_results.len().to_string(),
        ])?;
    }
    let data = writer.into_inner()?;
    Ok(String::from_utf8_lossy(&data).to_string())
}

pub fn arbitrage_to_csv(opps: &[ArbitrageOpportunity]) -> Result<String> {
    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record([
        "program",
        "estimated_gain_sol",
        "effort",
        "roi",
        "gap_count",
    ])?;
    for opp in opps {
        writer.write_record([
            opp.program.to_string(),
            format!("{:.2}", opp.estimated_delegation_gain_sol),
            format!("{:?}", opp.total_effort).to_lowercase(),
            format!("{:.4}", opp.roi_score),
            opp.gaps.len().to_string(),
        ])?;
    }
    let data = writer.into_inner()?;
    Ok(String::from_utf8_lossy(&data).to_string())
}

use anyhow::Result;

use crate::criteria::schema::{CriteriaSet, ProgramId};
use crate::programs::{DelegationProgram, ProgramRegistry};

pub async fn fetch_criteria_for_program(program: &dyn DelegationProgram) -> Result<CriteriaSet> {
    program.fetch_criteria().await
}

pub async fn fetch_all_criteria(
    registry: &ProgramRegistry,
    filter: Option<&[ProgramId]>,
) -> Result<Vec<CriteriaSet>> {
    let mut out = Vec::new();
    for program in registry.programs() {
        if let Some(filter) = filter {
            if !filter.contains(&program.id()) {
                continue;
            }
        }
        out.push(program.fetch_criteria().await?);
    }
    Ok(out)
}

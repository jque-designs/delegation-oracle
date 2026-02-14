pub mod blazestake;
pub mod jito;
pub mod jpool;
pub mod marinade;
pub mod sanctum;
pub mod sfdp;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::criteria::{CriteriaSet, ProgramId};
use crate::eligibility::EligibilityResult;
use crate::metrics::ValidatorMetrics;
use crate::programs::blazestake::BlazeStakeProgram;
use crate::programs::jito::JitoProgram;
use crate::programs::jpool::JPoolProgram;
use crate::programs::marinade::MarinadeProgram;
use crate::programs::sanctum::SanctumProgram;
use crate::programs::sfdp::SfdpProgram;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibleValidator {
    pub vote_pubkey: String,
    pub score: Option<f64>,
    pub delegated_sol: Option<f64>,
}

#[async_trait]
pub trait DelegationProgram: Send + Sync {
    fn id(&self) -> ProgramId;
    fn name(&self) -> &str;
    async fn fetch_criteria(&self) -> Result<CriteriaSet>;
    async fn fetch_eligible_set(&self) -> Result<Vec<EligibleValidator>>;
    fn evaluate(&self, validator: &ValidatorMetrics, criteria: &CriteriaSet) -> EligibilityResult;
    fn estimate_delegation(
        &self,
        validator: &ValidatorMetrics,
        criteria: &CriteriaSet,
    ) -> Option<f64>;
}

#[derive(Clone)]
pub struct ProgramRegistry {
    programs: Vec<Arc<dyn DelegationProgram>>,
}

impl ProgramRegistry {
    pub fn with_defaults() -> Self {
        let programs: Vec<Arc<dyn DelegationProgram>> = vec![
            Arc::new(SfdpProgram),
            Arc::new(MarinadeProgram),
            Arc::new(JPoolProgram),
            Arc::new(BlazeStakeProgram),
            Arc::new(JitoProgram),
            Arc::new(SanctumProgram),
        ];
        Self { programs }
    }

    pub fn programs(&self) -> &[Arc<dyn DelegationProgram>] {
        &self.programs
    }

    pub fn by_id(&self, id: ProgramId) -> Option<Arc<dyn DelegationProgram>> {
        self.programs.iter().find(|p| p.id() == id).cloned()
    }

    pub fn filter(&self, ids: &[ProgramId]) -> Vec<Arc<dyn DelegationProgram>> {
        self.programs
            .iter()
            .filter(|program| ids.contains(&program.id()))
            .cloned()
            .collect()
    }
}

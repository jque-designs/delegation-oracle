use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;

use crate::metrics::ValidatorMetrics;

#[derive(Debug, Clone)]
pub struct CachedMetrics {
    pub captured_at: DateTime<Utc>,
    pub metrics: ValidatorMetrics,
}

static METRIC_CACHE: Lazy<Mutex<HashMap<String, CachedMetrics>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn put(metrics: ValidatorMetrics) {
    let key = metrics.vote_pubkey.clone();
    let value = CachedMetrics {
        captured_at: Utc::now(),
        metrics,
    };
    let mut guard = METRIC_CACHE.lock().expect("metric cache mutex poisoned");
    guard.insert(key, value);
}

pub fn get(vote_pubkey: &str) -> Option<CachedMetrics> {
    let guard = METRIC_CACHE.lock().expect("metric cache mutex poisoned");
    guard.get(vote_pubkey).cloned()
}

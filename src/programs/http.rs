use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::programs::EligibleValidator;

const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 12;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 6;
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("delegation-oracle/0.2")
        .timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
        .build()
        .expect("failed to build HTTP client")
});

pub async fn fetch_json(url: &str) -> Result<Value> {
    let response = HTTP_CLIENT
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed GET request: {url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed reading response body: {url}"))?;
    if !status.is_success() {
        let preview: String = body.chars().take(180).collect();
        return Err(anyhow!("GET {url} returned {status}: {preview}"));
    }
    serde_json::from_str(&body).with_context(|| format!("invalid JSON response: {url}"))
}

pub async fn fetch_text(url: &str) -> Result<String> {
    let response = HTTP_CLIENT
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed GET request: {url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed reading response body: {url}"))?;
    if !status.is_success() {
        let preview: String = body.chars().take(180).collect();
        return Err(anyhow!("GET {url} returned {status}: {preview}"));
    }
    Ok(body)
}

pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn sha256_json(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    sha256_hex(&serialized)
}

pub fn collect_numeric_samples(value: &Value, paths: &[&str], max: usize) -> Vec<f64> {
    let mut out = Vec::new();
    for array in candidate_object_arrays(value) {
        for entry in array {
            let Some(object) = entry.as_object() else {
                continue;
            };
            if let Some(number) = number_from_paths(object, paths) {
                out.push(number);
            }
            if out.len() >= max {
                return out;
            }
        }
    }
    out
}

pub fn parse_eligible_validators(
    value: &Value,
    vote_paths: &[&str],
    score_paths: &[&str],
    delegation_paths: &[&str],
    max_items: usize,
) -> Vec<EligibleValidator> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();

    for array in candidate_object_arrays(value) {
        for entry in array {
            let Some(object) = entry.as_object() else {
                continue;
            };
            let Some(vote_pubkey) = string_from_paths(object, vote_paths) else {
                continue;
            };
            if !seen.insert(vote_pubkey.clone()) {
                continue;
            }

            let score = number_from_paths(object, score_paths);
            let delegated_sol =
                number_from_paths(object, delegation_paths).map(lamports_to_sol_if_needed);

            out.push(EligibleValidator {
                vote_pubkey,
                score,
                delegated_sol,
            });

            if out.len() >= max_items {
                return out;
            }
        }
    }

    out
}

pub fn percentile(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect::<Vec<_>>();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let pct = percentile.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted.get(idx).copied()
}

pub fn lamports_to_sol_if_needed(value: f64) -> f64 {
    if value.abs() >= LAMPORTS_PER_SOL {
        value / LAMPORTS_PER_SOL
    } else {
        value
    }
}

pub fn bps_to_percent_if_needed(value: f64) -> f64 {
    if value.abs() > 100.0 {
        value / 100.0
    } else {
        value
    }
}

fn candidate_object_arrays(value: &Value) -> Vec<&Vec<Value>> {
    let mut arrays = Vec::new();
    if let Some(arr) = value.as_array() {
        if looks_like_object_array(arr) {
            arrays.push(arr);
        }
    }
    if let Some(object) = value.as_object() {
        for key in [
            "validators",
            "data",
            "result",
            "items",
            "list",
            "eligible_validators",
            "validator_list",
        ] {
            if let Some(v) = object_get_case_insensitive(object, key) {
                if let Some(arr) = v.as_array() {
                    if looks_like_object_array(arr) {
                        arrays.push(arr);
                    }
                } else if let Some(nested) = v.as_object() {
                    for nested_key in ["validators", "items", "list", "data"] {
                        if let Some(inner) = object_get_case_insensitive(nested, nested_key) {
                            if let Some(arr) = inner.as_array() {
                                if looks_like_object_array(arr) {
                                    arrays.push(arr);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    arrays
}

fn looks_like_object_array(arr: &[Value]) -> bool {
    arr.iter().take(5).any(Value::is_object)
}

fn string_from_paths(object: &Map<String, Value>, paths: &[&str]) -> Option<String> {
    for path in paths {
        let value = object_path_value(object, path)?;
        match value {
            Value::String(s) => {
                if !s.trim().is_empty() {
                    return Some(s.trim().to_string());
                }
            }
            Value::Number(n) => return Some(n.to_string()),
            _ => {}
        }
    }
    None
}

fn number_from_paths(object: &Map<String, Value>, paths: &[&str]) -> Option<f64> {
    for path in paths {
        let value = object_path_value(object, path)?;
        if let Some(number) = to_f64(value) {
            return Some(number);
        }
    }
    None
}

fn object_path_value<'a>(object: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut segments = path.split('.');
    let first = segments.next()?;
    let mut current = object_get_case_insensitive(object, first)?;
    for segment in segments {
        let nested = current.as_object()?;
        current = object_get_case_insensitive(nested, segment)?;
    }
    Some(current)
}

fn object_get_case_insensitive<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    object.get(key).or_else(|| {
        object
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    })
}

fn to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let sanitized = s.trim().replace(',', "").replace('%', "").replace('_', "");
            sanitized.parse::<f64>().ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::programs::http::{
        bps_to_percent_if_needed, lamports_to_sol_if_needed, parse_eligible_validators, percentile,
    };

    #[test]
    fn parses_eligible_validators_from_nested_shape() {
        let payload = json!({
            "data": {
                "validators": [
                    {
                        "vote_account": "Vote111",
                        "score": 0.91,
                        "active_stake": 1200000000000u64
                    },
                    {
                        "vote_account": "Vote222",
                        "score": 0.84,
                        "active_stake": 320000000000u64
                    }
                ]
            }
        });

        let parsed = parse_eligible_validators(
            &payload,
            &["vote_account"],
            &["score"],
            &["active_stake"],
            10,
        );
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].vote_pubkey, "Vote111");
    }

    #[test]
    fn converts_lamports_and_bps_when_needed() {
        assert!((lamports_to_sol_if_needed(2_000_000_000.0) - 2.0).abs() < 1e-9);
        assert!((bps_to_percent_if_needed(850.0) - 8.5).abs() < 1e-9);
        assert!((bps_to_percent_if_needed(8.5) - 8.5).abs() < 1e-9);
    }

    #[test]
    fn percentile_handles_empty_and_basic_values() {
        assert!(percentile(&[], 0.5).is_none());
        let values = vec![1.0, 2.0, 10.0, 4.0];
        let p50 = percentile(&values, 0.5).expect("missing percentile");
        assert!(p50 >= 2.0 && p50 <= 4.0);
    }
}

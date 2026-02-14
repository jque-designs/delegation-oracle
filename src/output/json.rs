use anyhow::Result;
use serde::Serialize;

pub fn render_json<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

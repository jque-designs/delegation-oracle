use anyhow::Result;
use async_trait::async_trait;

use crate::alert::engine::AlertEvent;

#[async_trait]
pub trait AlertSink: Send + Sync {
    async fn send(&self, event: &AlertEvent) -> Result<()>;
}

pub struct StdoutSink;

#[async_trait]
impl AlertSink for StdoutSink {
    async fn send(&self, event: &AlertEvent) -> Result<()> {
        println!("[{:?}] {} - {}", event.kind, event.title, event.body);
        Ok(())
    }
}

pub struct WebhookSink {
    url: String,
}

impl WebhookSink {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait]
impl AlertSink for WebhookSink {
    async fn send(&self, event: &AlertEvent) -> Result<()> {
        // Lightweight fallback sink that keeps runtime dependencies minimal.
        // Operators can replace this with a richer HTTP client integration.
        println!(
            "[WEBHOOK:{}] [{:?}] {} - {}",
            self.url, event.kind, event.title, event.body
        );
        Ok(())
    }
}

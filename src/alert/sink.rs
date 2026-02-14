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
    client: reqwest::Client,
    url: String,
}

impl WebhookSink {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
        }
    }
}

#[async_trait]
impl AlertSink for WebhookSink {
    async fn send(&self, event: &AlertEvent) -> Result<()> {
        self.client
            .post(&self.url)
            .json(event)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

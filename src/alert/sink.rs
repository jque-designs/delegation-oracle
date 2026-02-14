use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

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
    client: Client,
    url: String,
}

impl WebhookSink {
    pub fn new(url: impl Into<String>) -> Self {
        let client = Client::builder()
            .user_agent("delegation-oracle/0.2")
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build webhook HTTP client");
        Self {
            client,
            url: url.into(),
        }
    }
}

#[async_trait]
impl AlertSink for WebhookSink {
    async fn send(&self, event: &AlertEvent) -> Result<()> {
        let req = if self.url.contains("discord.com/api/webhooks")
            || self.url.contains("discordapp.com/api/webhooks")
        {
            let content = format!("[{:?}] {}\n{}", event.kind, event.title, event.body);
            self.client
                .post(&self.url)
                .json(&serde_json::json!({ "content": content }))
        } else {
            self.client.post(&self.url).json(event)
        };

        req.send().await?.error_for_status()?;
        Ok(())
    }
}

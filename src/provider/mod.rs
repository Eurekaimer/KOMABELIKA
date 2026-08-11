pub mod deepseek;
pub mod factory;
mod openai_compatible;
pub mod opencode_go;

use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

pub type ChatStream = Pin<Box<dyn Stream<Item = StreamEvent> + Send>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub reasoning: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    TextDelta(String),
    ReasoningDelta(String),
    Usage(TokenUsage),
    Completed,
    Failed(String),
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn capabilities(&self) -> ProviderCapabilities;

    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    async fn stream_chat(&self, request: ChatRequest) -> Result<ChatStream>;

    async fn health_check(&self) -> Result<()>;
}

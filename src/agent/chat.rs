use std::sync::{Arc, atomic::AtomicBool};

use anyhow::Result;

use crate::provider::{
    ChatMessage, ChatProvider, ChatRequest, ChatStream, ModelInfo, ProviderCapabilities, Role,
};

pub struct ChatAgent {
    provider: Arc<dyn ChatProvider>,
    model: String,
}

impl ChatAgent {
    pub fn new(provider: Arc<dyn ChatProvider>, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }

    pub fn provider_id(&self) -> &'static str {
        self.provider.id()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn capabilities(&self) -> ProviderCapabilities {
        self.provider.capabilities()
    }

    pub async fn models(&self) -> Result<Vec<ModelInfo>> {
        self.provider.list_models().await
    }

    pub async fn reply(
        &self,
        history: &[ChatMessage],
        cancelled: Arc<AtomicBool>,
    ) -> Result<ChatStream> {
        let mut messages = Vec::with_capacity(history.len() + 1);
        messages.push(ChatMessage {
            role: Role::System,
            content: crate::persona::default_context().to_owned(),
        });
        messages.extend_from_slice(history);
        self.provider
            .stream_chat(ChatRequest {
                model: self.model.clone(),
                messages,
                cancelled,
            })
            .await
    }
}

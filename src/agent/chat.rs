use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, atomic::AtomicBool},
};

use anyhow::Result;

use crate::provider::{
    ChatMessage, ChatProvider, ChatRequest, ChatStream, ModelInfo, ProviderCapabilities, Role,
};

pub type ReplyFuture = Pin<Box<dyn Future<Output = Result<ChatStream>> + Send>>;
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

    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    pub fn capabilities(&self) -> ProviderCapabilities {
        self.provider.capabilities()
    }

    pub async fn models(&self) -> Result<Vec<ModelInfo>> {
        self.provider.list_models().await
    }

    pub fn start_reply(
        &self,
        history: Vec<ChatMessage>,
        cancelled: Arc<AtomicBool>,
    ) -> ReplyFuture {
        let provider = Arc::clone(&self.provider);
        let model = self.model.clone();
        Box::pin(async move {
            let mut messages = Vec::with_capacity(history.len() + 1);
            messages.push(ChatMessage {
                role: Role::System,
                content: crate::persona::default_context().to_owned(),
            });
            messages.extend(history);
            provider
                .stream_chat(ChatRequest {
                    model,
                    messages,
                    cancelled,
                })
                .await
        })
    }
}

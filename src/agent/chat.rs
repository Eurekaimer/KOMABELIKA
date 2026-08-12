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

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use async_trait::async_trait;
    use futures_util::stream;
    use tokio::sync::Mutex;

    use super::*;
    use crate::provider::{StreamEvent, ChatMessage};

    #[derive(Default)]
    struct CapturingProvider {
        request: Mutex<Option<ChatRequest>>,
    }

    #[async_trait]
    impl ChatProvider for CapturingProvider {
        fn id(&self) -> &'static str {
            "capturing"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                streaming: true,
                reasoning: false,
            }
        }

        async fn list_models(&self) -> Result<Vec<ModelInfo>> {
            Ok(Vec::new())
        }

        async fn stream_chat(&self, request: ChatRequest) -> Result<ChatStream> {
            *self.request.lock().await = Some(request);
            Ok(Box::pin(stream::iter([StreamEvent::Completed])))
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn knowledge_question_is_sent_with_direct_answer_policy() {
        let provider = Arc::new(CapturingProvider::default());
        let agent = ChatAgent::new(provider.clone(), "test-model");
        let history = vec![ChatMessage {
            role: Role::User,
            content: "请解释快速排序算法".into(),
        }];

        agent
            .start_reply(history, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        let request = provider.request.lock().await;
        let request = request.as_ref().unwrap();
        assert_eq!(request.messages[1].content, "请解释快速排序算法");
        assert!(request.messages[0].content.contains("直接使用你已有的世界知识准确回答"));
        assert!(request.messages[0].content.contains("不要仅因问题专业"));
    }
}

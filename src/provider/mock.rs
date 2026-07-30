use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::stream;

use super::{
    ChatProvider, ChatRequest, ChatStream, ModelInfo, ProviderCapabilities, Role, StreamEvent,
    TokenUsage,
};

#[derive(Clone, Debug)]
pub struct MockProvider {
    delay: Duration,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(28),
        }
    }
}

impl MockProvider {
    #[cfg(test)]
    pub fn immediate() -> Self {
        Self {
            delay: Duration::ZERO,
        }
    }
}

#[async_trait]
impl ChatProvider for MockProvider {
    fn id(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            reasoning: false,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![ModelInfo {
            id: "komari-mock".into(),
            display_name: "Komari Mock".into(),
        }])
    }

    async fn stream_chat(&self, request: ChatRequest) -> Result<ChatStream> {
        if request.model != "komari-mock" {
            anyhow::bail!("mock model is unavailable: {}", request.model);
        }
        let input = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::User)
            .map(|message| message.content.trim())
            .unwrap_or_default();
        let response = if input.is_empty() {
            "……你什么都没写。是在试键盘吗？".to_owned()
        } else {
            format!(
                "我、我听见了。你说的是“{input}”。\n……现在还只是 Mock，不过这段对话，我会好好留着。"
            )
        };
        let delay = self.delay;
        let output_tokens = response.chars().count() as u64;
        let input_tokens = input.chars().count() as u64;
        let state = (response.chars().collect::<Vec<_>>(), 0usize, request);

        Ok(Box::pin(stream::unfold(
            state,
            move |(chars, index, request)| async move {
                if index == usize::MAX {
                    return None;
                }
                if request.cancelled.load(Ordering::Relaxed) {
                    return Some((
                        StreamEvent::Failed("cancelled".into()),
                        (chars, usize::MAX, request),
                    ));
                }
                if index < chars.len() {
                    tokio::time::sleep(delay).await;
                    let event = StreamEvent::TextDelta(chars[index].to_string());
                    return Some((event, (chars, index + 1, request)));
                }
                if index == chars.len() {
                    let usage = StreamEvent::Usage(TokenUsage {
                        input_tokens,
                        output_tokens,
                    });
                    return Some((usage, (chars, index + 1, request)));
                }
                if index == chars.len() + 1 {
                    return Some((StreamEvent::Completed, (chars, index + 1, request)));
                }
                None
            },
        )))
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool};

    use futures_util::StreamExt;

    use super::*;
    use crate::provider::{ChatMessage, Role};

    #[tokio::test]
    async fn streams_text_usage_and_completion() {
        let provider = MockProvider::immediate();
        let request = ChatRequest {
            model: "komari-mock".into(),
            messages: vec![ChatMessage {
                role: Role::User,
                content: "晚上好".into(),
            }],
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let events = provider
            .stream_chat(request)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await;

        assert!(
            events
                .iter()
                .any(|event| matches!(event, StreamEvent::TextDelta(_)))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, StreamEvent::Usage(_)))
        );
        assert_eq!(events.last(), Some(&StreamEvent::Completed));
    }

    #[tokio::test]
    async fn observes_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let request = ChatRequest {
            model: "komari-mock".into(),
            messages: Vec::new(),
            cancelled,
        };
        let mut stream = MockProvider::immediate()
            .stream_chat(request)
            .await
            .unwrap();

        assert_eq!(
            stream.next().await,
            Some(StreamEvent::Failed("cancelled".into()))
        );
    }
}

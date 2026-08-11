use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use futures_util::{StreamExt, stream};
use tokio::sync::mpsc;

use crate::provider::{ChatStream, StreamEvent, TokenUsage};

use super::{protocol::CompletionChunk, sse::SseDecoder};

pub(crate) fn into_chat_stream(
    response: reqwest::Response,
    cancelled: Arc<AtomicBool>,
    provider_name: &'static str,
) -> ChatStream {
    let (sender, receiver) = mpsc::channel(64);
    tokio::spawn(pump(response, cancelled, sender, provider_name));
    Box::pin(stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| (event, receiver))
    }))
}

async fn pump(
    response: reqwest::Response,
    cancelled: Arc<AtomicBool>,
    sender: mpsc::Sender<StreamEvent>,
    provider_name: &'static str,
) {
    let mut bytes = response.bytes_stream();
    let mut decoder = SseDecoder::default();

    while let Some(chunk) = bytes.next().await {
        if cancelled.load(Ordering::Relaxed) {
            send(&sender, StreamEvent::Failed("cancelled".into())).await;
            return;
        }
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                send(
                    &sender,
                    StreamEvent::Failed(format!("{provider_name} transport error: {error}")),
                )
                .await;
                return;
            }
        };
        let payloads = match decoder.push(&chunk) {
            Ok(payloads) => payloads,
            Err(error) => {
                send(
                    &sender,
                    StreamEvent::Failed(format!("{provider_name} stream protocol error: {error}")),
                )
                .await;
                return;
            }
        };
        for payload in payloads {
            if dispatch_payload(&sender, &payload, provider_name).await {
                return;
            }
        }
    }

    let payloads = match decoder.finish() {
        Ok(payloads) => payloads,
        Err(error) => {
            send(
                &sender,
                StreamEvent::Failed(format!("{provider_name} stream protocol error: {error}")),
            )
            .await;
            return;
        }
    };
    for payload in payloads {
        if dispatch_payload(&sender, &payload, provider_name).await {
            return;
        }
    }
    send(
        &sender,
        StreamEvent::Failed(format!("{provider_name} closed the stream before [DONE]")),
    )
    .await;
}

async fn dispatch_payload(
    sender: &mpsc::Sender<StreamEvent>,
    payload: &str,
    provider_name: &'static str,
) -> bool {
    if payload == "[DONE]" {
        send(sender, StreamEvent::Completed).await;
        return true;
    }
    let chunk = match serde_json::from_str::<CompletionChunk>(payload) {
        Ok(chunk) => chunk,
        Err(error) => {
            send(
                sender,
                StreamEvent::Failed(format!(
                    "{provider_name} stream protocol error: invalid JSON chunk: {error}"
                )),
            )
            .await;
            return true;
        }
    };
    for choice in chunk.choices {
        if let Some(reasoning) = choice
            .delta
            .reasoning_content
            .filter(|text| !text.is_empty())
        {
            send(sender, StreamEvent::ReasoningDelta(reasoning)).await;
        }
        if let Some(content) = choice.delta.content.filter(|text| !text.is_empty()) {
            send(sender, StreamEvent::TextDelta(content)).await;
        }
    }
    if let Some(usage) = chunk.usage {
        send(
            sender,
            StreamEvent::Usage(TokenUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            }),
        )
        .await;
    }
    false
}

async fn send(sender: &mpsc::Sender<StreamEvent>, event: StreamEvent) {
    let _ = sender.send(event).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dispatches_reasoning_text_usage_and_completion() {
        let (sender, mut receiver) = mpsc::channel(8);
        let payload = r#"{"choices":[{"delta":{"reasoning_content":"think","content":"answer"}}],"usage":{"prompt_tokens":7,"completion_tokens":3}}"#;
        assert!(!dispatch_payload(&sender, payload, "DeepSeek").await);
        assert!(dispatch_payload(&sender, "[DONE]", "DeepSeek").await);

        assert_eq!(
            receiver.recv().await,
            Some(StreamEvent::ReasoningDelta("think".into()))
        );
        assert_eq!(
            receiver.recv().await,
            Some(StreamEvent::TextDelta("answer".into()))
        );
        assert_eq!(
            receiver.recv().await,
            Some(StreamEvent::Usage(TokenUsage {
                input_tokens: 7,
                output_tokens: 3,
            }))
        );
        assert_eq!(receiver.recv().await, Some(StreamEvent::Completed));
    }
}

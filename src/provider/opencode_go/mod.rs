mod error;

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::header::ACCEPT;

use crate::provider::{
    ChatProvider, ChatRequest, ChatStream, ModelInfo, ProviderCapabilities,
    openai_compatible::{
        into_chat_stream,
        protocol::{CompletionRequest, ModelsResponse, StreamOptions},
    },
};

use error::{classify_response, transport};

pub struct OpenCodeGoSettings {
    pub base_url: String,
    pub timeout: Duration,
    pub max_tokens: Option<u32>,
}

pub struct OpenCodeGoProvider {
    client: reqwest::Client,
    settings: OpenCodeGoSettings,
    api_key: String,
}

impl OpenCodeGoProvider {
    pub fn new(settings: OpenCodeGoSettings, api_key: String) -> Result<Self> {
        anyhow::ensure!(
            !api_key.trim().is_empty(),
            "OpenCode Go API key cannot be empty"
        );
        anyhow::ensure!(
            settings.base_url.starts_with("https://")
                || settings.base_url.starts_with("http://127.0.0.1")
                || settings.base_url.starts_with("http://localhost"),
            "OpenCode Go base URL must use HTTPS (localhost is allowed for testing)"
        );
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(settings.timeout)
            .user_agent(concat!("komari-call/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            settings,
            api_key,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.settings.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn models_response(&self) -> Result<reqwest::Response> {
        self.client
            .get(self.endpoint("models"))
            .header(ACCEPT, "application/json")
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|error| transport(error, &self.api_key).into())
    }
}

#[async_trait]
impl ChatProvider for OpenCodeGoProvider {
    fn id(&self) -> &'static str {
        "opencode-go"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            reasoning: true,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self.models_response().await?;
        if !response.status().is_success() {
            return Err(classify_response(response, &self.api_key).await.into());
        }
        let models = response
            .json::<ModelsResponse>()
            .await
            .map_err(|error| transport(error, &self.api_key))?;
        Ok(models
            .data
            .into_iter()
            .filter(|model| supports_chat_completions(&model.id))
            .map(|model| ModelInfo {
                display_name: model.id.clone(),
                id: model.id,
            })
            .collect())
    }

    async fn stream_chat(&self, request: ChatRequest) -> Result<ChatStream> {
        anyhow::ensure!(
            !request.model.trim().is_empty(),
            "OpenCode Go model cannot be empty"
        );
        anyhow::ensure!(
            supports_chat_completions(&request.model),
            "OpenCode Go model '{}' is not compatible with /chat/completions",
            request.model
        );
        let body = CompletionRequest {
            model: request.model,
            messages: request.messages,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            thinking: None,
            max_tokens: self.settings.max_tokens,
        };
        let response = self
            .client
            .post(self.endpoint("chat/completions"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| transport(error, &self.api_key))?;
        if !response.status().is_success() {
            return Err(classify_response(response, &self.api_key).await.into());
        }
        Ok(into_chat_stream(response, request.cancelled, "OpenCode Go"))
    }

    async fn health_check(&self) -> Result<()> {
        let response = self.models_response().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(classify_response(response, &self.api_key).await.into())
        }
    }
}

fn supports_chat_completions(model: &str) -> bool {
    matches!(
        model,
        "grok-4.5"
            | "glm-5.2"
            | "glm-5.1"
            | "kimi-k3"
            | "kimi-k2.7-code"
            | "kimi-k2.6"
            | "deepseek-v4-pro"
            | "deepseek-v4-flash"
            | "mimo-v2.5"
            | "mimo-v2.5-pro"
            | "hy3"
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, atomic::AtomicBool},
        thread,
    };

    use futures_util::StreamExt;

    use super::*;
    use crate::provider::{ChatMessage, Role, StreamEvent, TokenUsage};

    fn provider(base_url: String) -> OpenCodeGoProvider {
        OpenCodeGoProvider::new(
            OpenCodeGoSettings {
                base_url,
                timeout: Duration::from_secs(5),
                max_tokens: Some(512),
            },
            "test-key".into(),
        )
        .unwrap()
    }

    fn serve_once(
        check: impl FnOnce(&str) + Send + 'static,
        content_type: &'static str,
        body: &'static str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0_u8; 4096];
                let count = socket.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(request).unwrap();
            check(&request);
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn filters_models_and_sends_bearer_authorization() {
        let base_url = serve_once(
            |request| {
                assert!(request.starts_with("GET /models HTTP/1.1"));
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("authorization: bearer test-key")
                );
            },
            "application/json",
            r#"{"data":[{"id":"deepseek-v4-flash"},{"id":"qwen3.7-plus"},{"id":"gpt-5.6-luna"},{"id":"minimax-m3"},{"id":"hy3"}]}"#,
        );
        let models = provider(base_url).list_models().await.unwrap();
        assert_eq!(
            models.into_iter().map(|model| model.id).collect::<Vec<_>>(),
            ["deepseek-v4-flash", "hy3"]
        );
    }

    #[tokio::test]
    async fn streams_chat_without_deepseek_thinking_field() {
        let base_url = serve_once(
            |request| {
                assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("authorization: bearer test-key")
                );
                let body = request.split_once("\r\n\r\n").unwrap().1;
                let json: serde_json::Value = serde_json::from_str(body).unwrap();
                assert_eq!(json["model"], "deepseek-v4-flash");
                assert_eq!(json["messages"][0]["content"], "hello");
                assert_eq!(json["stream"], true);
                assert_eq!(json["stream_options"]["include_usage"], true);
                assert_eq!(json["max_tokens"], 512);
                assert!(json.get("thinking").is_none());
            },
            "text/event-stream",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\",\"content\":\"answer\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3}}\n\ndata: [DONE]\n\n",
        );
        let request = ChatRequest {
            model: "deepseek-v4-flash".into(),
            messages: vec![ChatMessage {
                role: Role::User,
                content: "hello".into(),
            }],
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let events = provider(base_url)
            .stream_chat(request)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await;
        assert_eq!(
            events,
            [
                StreamEvent::ReasoningDelta("think".into()),
                StreamEvent::TextDelta("answer".into()),
                StreamEvent::Usage(TokenUsage {
                    input_tokens: 7,
                    output_tokens: 3,
                }),
                StreamEvent::Completed,
            ]
        );
    }

    #[tokio::test]
    async fn rejects_models_using_other_protocols_before_http() {
        for model in ["qwen3.7-plus", "gpt-5.6-luna", "minimax-m3"] {
            let request = ChatRequest {
                model: model.into(),
                messages: Vec::new(),
                cancelled: Arc::new(AtomicBool::new(false)),
            };
            let error = provider("http://127.0.0.1:9".into())
                .stream_chat(request)
                .await
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains("not compatible with /chat/completions"));
        }
    }

    #[test]
    fn validates_key_and_url() {
        let settings = OpenCodeGoSettings {
            base_url: "https://opencode.ai/zen/go/v1".into(),
            timeout: Duration::from_secs(5),
            max_tokens: None,
        };
        assert!(OpenCodeGoProvider::new(settings, "  ".into()).is_err());
        let settings = OpenCodeGoSettings {
            base_url: "http://example.com".into(),
            timeout: Duration::from_secs(5),
            max_tokens: None,
        };
        assert!(OpenCodeGoProvider::new(settings, "key".into()).is_err());
    }
}

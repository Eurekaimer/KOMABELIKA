mod error;
mod protocol;
mod sse;
mod stream;

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::header::ACCEPT;

use crate::provider::{ChatProvider, ChatRequest, ChatStream, ModelInfo, ProviderCapabilities};

use error::{classify_response, transport};
use protocol::{CompletionRequest, ModelsResponse, StreamOptions, Thinking};
use stream::into_chat_stream;

pub struct DeepSeekSettings {
    pub base_url: String,
    pub timeout: Duration,
    pub thinking: bool,
    pub max_tokens: Option<u32>,
}

pub struct DeepSeekProvider {
    client: reqwest::Client,
    settings: DeepSeekSettings,
    api_key: String,
}

impl DeepSeekProvider {
    pub fn new(settings: DeepSeekSettings, api_key: String) -> Result<Self> {
        anyhow::ensure!(
            !api_key.trim().is_empty(),
            "DeepSeek API key cannot be empty"
        );
        anyhow::ensure!(
            settings.base_url.starts_with("https://")
                || settings.base_url.starts_with("http://127.0.0.1")
                || settings.base_url.starts_with("http://localhost"),
            "DeepSeek base URL must use HTTPS (localhost is allowed for testing)"
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
impl ChatProvider for DeepSeekProvider {
    fn id(&self) -> &'static str {
        "deepseek"
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
            .map(|model| ModelInfo {
                display_name: model.id.clone(),
                id: model.id,
            })
            .collect())
    }

    async fn stream_chat(&self, request: ChatRequest) -> Result<ChatStream> {
        anyhow::ensure!(
            !request.model.trim().is_empty(),
            "DeepSeek model cannot be empty"
        );
        let body = CompletionRequest {
            model: request.model,
            messages: request.messages,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            thinking: Some(Thinking {
                kind: if self.settings.thinking {
                    "enabled"
                } else {
                    "disabled"
                },
            }),
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
        Ok(into_chat_stream(response, request.cancelled))
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

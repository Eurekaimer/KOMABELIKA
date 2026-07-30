use serde::{Deserialize, Serialize};

use crate::provider::ChatMessage;

#[derive(Serialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub stream_options: StreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Serialize)]
pub struct Thinking {
    #[serde(rename = "type")]
    pub kind: &'static str,
}

#[derive(Deserialize)]
pub struct CompletionChunk {
    #[serde(default)]
    pub choices: Vec<Choice>,
    pub usage: Option<ApiUsage>,
}

#[derive(Deserialize)]
pub struct Choice {
    #[serde(default)]
    pub delta: Delta,
}

#[derive(Default, Deserialize)]
pub struct Delta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ApiModel>,
}

#[derive(Deserialize)]
pub struct ApiModel {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ErrorEnvelope {
    pub error: Option<ApiError>,
}

#[derive(Deserialize)]
pub struct ApiError {
    pub message: Option<String>,
}

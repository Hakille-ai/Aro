use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AssistantMode, ChatMessage, ModelProviderKind, VoiceStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub model_provider: ModelProviderKind,
    pub model_id: String,
    pub model_ready: bool,
    pub voice_ready: bool,
    pub endpoint: Option<String>,
    pub detail: String,
    pub checked_at: DateTime<Utc>,
}

impl RuntimeStatus {
    pub fn unavailable(
        model_provider: ModelProviderKind,
        model_id: impl Into<String>,
        endpoint: Option<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            model_provider,
            model_id: model_id.into(),
            model_ready: false,
            voice_ready: false,
            endpoint,
            detail: detail.into(),
            checked_at: Utc::now(),
        }
    }

    pub fn set_voice_status(&mut self, voice_status: &VoiceStatus) {
        self.voice_ready = voice_status.ready;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelResponseFormat {
    #[default]
    DirectText,
    AgentActionJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelGenerationRequest {
    pub mode: AssistantMode,
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub user_input: String,
    pub temperature: f32,
    pub max_tokens: u32,
    #[serde(default)]
    pub response_format: ModelResponseFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelGeneration {
    pub content: String,
    pub provider_detail: String,
    pub token_estimate: Option<u32>,
}

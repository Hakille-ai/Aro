use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AttachmentRef, MessageAttachment};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AssistantMode {
    #[default]
    Chat,
    Think,
    Code,
    Summarize,
    Quiet,
}

impl AssistantMode {
    pub fn system_instruction(&self) -> String {
        crate::instructions::compose_mode_instruction(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    pub color: String,
    pub icon: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub icon: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub mode: AssistantMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
}

impl Conversation {
    pub fn new(title: impl Into<String>, mode: AssistantMode) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            created_at: now,
            updated_at: now,
            mode,
            project_id: None,
            folder_id: None,
            root_path: None,
        }
    }

    pub fn with_id(id: Uuid, title: impl Into<String>, mode: AssistantMode) -> Self {
        let now = Utc::now();
        Self {
            id,
            title: title.into(),
            created_at: now,
            updated_at: now,
            mode,
            project_id: None,
            folder_id: None,
            root_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<MessageAttachment>,
    pub created_at: DateTime<Utc>,
    pub token_estimate: Option<u32>,
    pub model_id: Option<String>,
    pub agent_run_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<crate::agent::AgentStep>>,
}

impl ChatMessage {
    pub fn new(conversation_id: Uuid, role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            model_id: None,
            role,
            content: content.into(),
            attachments: Vec::new(),
            created_at: Utc::now(),
            token_estimate: None,
            agent_run_id: None,
            steps: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub conversation_id: Option<Uuid>,
    pub content: String,
    pub mode: AssistantMode,
    pub system_prompt: Option<String>,
    /// Optional model override — used by Arena mode to target a specific model.
    pub model_id: Option<String>,
    /// Optional provider override (e.g. "ollama", "llama-cpp", "mock").
    pub provider: Option<String>,
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
    #[serde(default)]
    pub web_access: WebAccessMode,
    #[serde(default)]
    pub search_settings: Option<crate::settings::SearchSettings>,
    #[serde(default)]
    pub memory_settings: Option<crate::settings::MemoryConfigurationSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    pub conversation: Conversation,
    pub user_message: ChatMessage,
    pub assistant_message: ChatMessage,
    pub agent_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WebAccessMode {
    Off,
    #[default]
    Auto,
    On,
}

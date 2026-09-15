use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{AppSettings, Conversation, Membership, Organization, RuntimeStatus, User};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceMode {
    #[default]
    Local,
    Cloud,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyncHealth {
    Online,
    OfflineReadOnly,
    SyncPending,
    SessionExpired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub health: SyncHealth,
    pub pending_events: u32,
    pub last_synced_at: Option<DateTime<Utc>>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            health: SyncHealth::Online,
            pending_events: 0,
            last_synced_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    pub user_id: Uuid,
    pub theme: String,
    pub language: String,
    pub wake_word_enabled: bool,
    pub inference_mode: InferenceMode,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalityProfile {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub prompt: String,
    pub icon: Option<String>,
    pub avatar_color: Option<String>,
    pub temperature: f32,
    pub voice_id: Option<Uuid>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPromptSet {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub mode: String,
    pub identity: String,
    pub rules: String,
    pub formatting: String,
    pub compiled_prompt: String,
    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub path: String,
    pub speaker_id: Option<i32>,
    pub language: String,
    pub avatar_color: Option<String>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillGroup {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub group_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub triggers: Vec<String>,
    pub kind: String,
    pub content: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConnection {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub status: String,
    pub auth_type: String,
    pub config: Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub enabled: bool,
    pub status: String,
    pub tools: Value,
    pub resources: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookHook {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
    pub enabled: bool,
    pub last_triggered_at: Option<DateTime<Utc>>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub prompt: String,
    pub schedule_type: String,
    pub duration_minutes: Option<i32>,
    pub cron_expression: Option<String>,
    pub enabled: bool,
    pub status: String,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayloadV2 {
    pub current_user: User,
    pub active_organization: Organization,
    pub memberships: Vec<Membership>,
    pub settings: AppSettings,
    pub preferences: UserPreferences,
    pub sync_status: SyncStatus,
    pub runtime: RuntimeStatus,
    pub conversations: Vec<Conversation>,
    pub client_state: BTreeMap<String, Value>,
}

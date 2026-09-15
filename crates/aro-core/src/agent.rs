use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::AssistantMode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRunStatus {
    Queued,
    Running,
    Waiting,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentLaneStatus {
    Active,
    Paused,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRunPriority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStepKind {
    RunStarted,
    ContextBuilt,
    Model,
    Tool,
    Checkpoint,
    Final,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStepStatus {
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentActionType {
    Final,
    Tool,
    Pause,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentAction {
    #[serde(rename = "type")]
    pub action_type: AgentActionType,
    pub content: Option<String>,
    pub tool_id: Option<String>,
    #[serde(default)]
    pub input: Value,
    pub reason: Option<String>,
}

impl AgentAction {
    pub fn final_response(content: impl Into<String>) -> Self {
        Self {
            action_type: AgentActionType::Final,
            content: Some(content.into()),
            tool_id: None,
            input: Value::Null,
            reason: None,
        }
    }

    pub fn tool(tool_id: impl Into<String>, input: Value, reason: Option<String>) -> Self {
        Self {
            action_type: AgentActionType::Tool,
            content: None,
            tool_id: Some(tool_id.into()),
            input,
            reason,
        }
    }

    pub fn pause(reason: impl Into<String>) -> Self {
        Self {
            action_type: AgentActionType::Pause,
            content: None,
            tool_id: None,
            input: Value::Null,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolSource {
    BuiltIn,
    Skill,
    Mcp,
    Plugin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionCommandApproval {
    Always,
    SafeAuto,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: ToolSource,
    pub enabled: bool,
    pub dangerous: bool,
    pub input_schema: Value,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionProfile {
    pub id: Uuid,
    pub name: String,
    pub trusted_roots: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub allow_read: bool,
    pub allow_write: bool,
    pub allow_shell: bool,
    pub allow_network: bool,
    pub command_approval: PermissionCommandApproval,
    pub redact_secrets: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PermissionProfile {
    pub fn trusted_workspace(root: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: "Trusted workspace".to_string(),
            trusted_roots: vec![root.into()],
            allowed_domains: vec![
                "github.com".to_string(),
                "npmjs.com".to_string(),
                "crates.io".to_string(),
                "docs.rs".to_string(),
            ],
            allow_read: true,
            allow_write: true,
            allow_shell: true,
            allow_network: true,
            command_approval: PermissionCommandApproval::Never,
            redact_secrets: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSource {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub excerpt: String,
    pub uri: Option<String>,
    pub score: f32,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPack {
    pub id: Uuid,
    pub run_id: Uuid,
    pub goal: String,
    pub summary: String,
    pub sources: Vec<ContextSource>,
    pub tools: Vec<ToolRef>,
    pub token_estimate: u32,
    pub built_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: Uuid,
    pub lane_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub goal: String,
    pub mode: AssistantMode,
    pub status: AgentRunStatus,
    pub priority: AgentRunPriority,
    pub model_provider_id: Option<String>,
    pub model_id: Option<String>,
    pub autonomy_profile_id: Option<Uuid>,
    pub checkpoint_summary: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl AgentRun {
    pub fn new(
        goal: impl Into<String>,
        mode: AssistantMode,
        conversation_id: Option<Uuid>,
        model_provider_id: Option<String>,
        model_id: Option<String>,
        autonomy_profile_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            lane_id: None,
            conversation_id,
            goal: goal.into(),
            mode,
            status: AgentRunStatus::Running,
            priority: AgentRunPriority::Normal,
            model_provider_id,
            model_id,
            autonomy_profile_id,
            checkpoint_summary: None,
            last_error: None,
            created_at: now,
            updated_at: now,
            heartbeat_at: Some(now),
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLane {
    pub id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub title: String,
    pub status: AgentLaneStatus,
    pub priority: AgentRunPriority,
    pub max_concurrent_runs: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentLane {
    pub fn new(conversation_id: Option<Uuid>, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            title: title.into(),
            status: AgentLaneStatus::Active,
            priority: AgentRunPriority::Normal,
            max_concurrent_runs: 1,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaneView {
    pub lane: AgentLane,
    pub queued_count: u32,
    pub running_count: u32,
    pub waiting_count: u32,
    pub latest_runs: Vec<AgentRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOrchestratorSnapshot {
    pub max_global_running: u32,
    pub running_count: u32,
    pub queued_count: u32,
    pub lanes: Vec<AgentLaneView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStep {
    pub id: Uuid,
    pub run_id: Uuid,
    pub sequence: i32,
    pub kind: AgentStepKind,
    pub status: AgentStepStatus,
    pub title: String,
    pub input: Value,
    pub output: Value,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl AgentStep {
    pub fn completed(
        run_id: Uuid,
        sequence: i32,
        kind: AgentStepKind,
        title: impl Into<String>,
        input: Value,
        output: Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            run_id,
            sequence,
            kind,
            status: AgentStepStatus::Completed,
            title: title.into(),
            input,
            output,
            error: None,
            started_at: now,
            finished_at: Some(now),
        }
    }

    pub fn failed(
        run_id: Uuid,
        sequence: i32,
        kind: AgentStepKind,
        title: impl Into<String>,
        input: Value,
        error: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            run_id,
            sequence,
            kind,
            status: AgentStepStatus::Failed,
            title: title.into(),
            input,
            output: Value::Null,
            error: Some(error.into()),
            started_at: now,
            finished_at: Some(now),
        }
    }

    pub fn skipped(
        run_id: Uuid,
        sequence: i32,
        kind: AgentStepKind,
        title: impl Into<String>,
        input: Value,
        output: Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            run_id,
            sequence,
            kind,
            status: AgentStepStatus::Skipped,
            title: title.into(),
            input,
            output,
            error: None,
            started_at: now,
            finished_at: Some(now),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentArtifact {
    pub id: Uuid,
    pub run_id: Uuid,
    pub kind: String,
    pub title: String,
    pub uri: Option<String>,
    pub content: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextItem {
    pub id: Uuid,
    pub run_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub uri: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub run_id: Uuid,
    pub sequence: Option<i32>,
    pub event_type: String,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunStartRequest {
    pub run_id: Option<Uuid>,
    pub lane_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub goal: String,
    pub mode: AssistantMode,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    /// Explicit, server-validated permission profile frozen onto the run at creation time.
    #[serde(default)]
    pub autonomy_profile_id: Option<Uuid>,
    pub priority: Option<AgentRunPriority>,
    pub max_steps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunView {
    pub run: AgentRun,
    pub steps: Vec<AgentStep>,
    pub artifacts: Vec<AgentArtifact>,
    pub context_pack: Option<ContextPack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAgentDefinition {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model_provider_id: Option<String>,
    pub model_id: Option<String>,
    pub autonomy_profile_id: Option<Uuid>,
    pub enabled_tools: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelDefinition {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub owner_user_id: Uuid,
    pub provider_kind: String,
    pub provider_id: String,
    pub model_id: String,
    pub label: String,
    pub endpoint: String,
    pub api_key_vault_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

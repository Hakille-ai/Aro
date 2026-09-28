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
    #[serde(default)]
    pub thinking: Option<String>,
}

impl AgentAction {
    pub fn final_response(content: impl Into<String>) -> Self {
        Self {
            action_type: AgentActionType::Final,
            content: Some(content.into()),
            tool_id: None,
            input: Value::Null,
            reason: None,
            thinking: None,
        }
    }

    pub fn tool(tool_id: impl Into<String>, input: Value, reason: Option<String>) -> Self {
        Self {
            action_type: AgentActionType::Tool,
            content: None,
            tool_id: Some(tool_id.into()),
            input,
            reason,
            thinking: None,
        }
    }

    pub fn pause(reason: impl Into<String>) -> Self {
        Self {
            action_type: AgentActionType::Pause,
            content: None,
            tool_id: None,
            input: Value::Null,
            reason: Some(reason.into()),
            thinking: None,
        }
    }

    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
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

// ──────────────────────────────────────────────────────────
// Multi-Agent Collaboration & Cognitive Memory Persisted Models
// Aligned with packages/contracts/src/agent.ts
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageType {
    TaskDelegation,
    TaskProgress,
    TaskResult,
    ClarificationRequest,
    ClarificationResponse,
    PeerCollaboration,
    ContextQuery,
    ContextShare,
    ErrorEscalation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentParticipantKind {
    Orchestrator,
    Subagent,
    User,
    Tool,
    Broadcast,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentParticipant {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(rename = "type")]
    pub kind: AgentParticipantKind,
}

impl AgentParticipant {
    pub fn orchestrator(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role: Some("orchestrator".to_string()),
            icon: Some("🧭".to_string()),
            kind: AgentParticipantKind::Orchestrator,
        }
    }

    pub fn subagent(
        id: impl Into<String>,
        name: impl Into<String>,
        role: impl Into<String>,
        icon: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role: Some(role.into()),
            icon,
            kind: AgentParticipantKind::Subagent,
        }
    }

    pub fn user(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role: None,
            icon: None,
            kind: AgentParticipantKind::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentArtifactRef {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessagePayload {
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<AgentArtifactRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_actions: Vec<String>,
}

impl AgentMessagePayload {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            structured_data: None,
            artifacts: Vec::new(),
            suggested_actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageEnvelope {
    pub id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub sender: AgentParticipant,
    pub recipient: AgentParticipant,
    pub message_type: AgentMessageType,
    pub payload: AgentMessagePayload,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<AgentRunPriority>,
    pub timestamp: DateTime<Utc>,
}

impl AgentMessageEnvelope {
    pub fn new(
        conversation_id: impl Into<String>,
        sender: AgentParticipant,
        recipient: AgentParticipant,
        message_type: AgentMessageType,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.into(),
            parent_message_id: None,
            correlation_id: None,
            sender,
            recipient,
            message_type,
            payload: AgentMessagePayload::text(content),
            permission_profile_id: None,
            priority: Some(AgentRunPriority::Normal),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryFinding {
    pub id: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_tool: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl AgentMemoryFinding {
    pub fn new(
        summary: impl Into<String>,
        category: Option<String>,
        source_tool: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            summary: summary.into(),
            category,
            source_tool,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryContext {
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub conversation_id: String,
    #[serde(default)]
    pub scratchpad: String,
    #[serde(default)]
    pub findings: Vec<AgentMemoryFinding>,
    #[serde(default)]
    pub ledger: Vec<AgentMessageEnvelope>,
    #[serde(default)]
    pub artifacts: Vec<AgentArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl AgentMemoryContext {
    pub fn new(
        conversation_id: impl Into<String>,
        agent_id: impl Into<String>,
        agent_name: impl Into<String>,
        role: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_name: agent_name.into(),
            role: role.into(),
            conversation_id: conversation_id.into(),
            scratchpad: String::new(),
            findings: Vec::new(),
            ledger: Vec::new(),
            artifacts: Vec::new(),
            permission_profile_id: None,
            updated_at: Utc::now(),
        }
    }
}

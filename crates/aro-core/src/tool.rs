use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{AgentArtifact, AroError, AroResult, ContextSource};

pub const TOOL_WEB_SEARCH: &str = "web.search";
pub const TOOL_WEB_FETCH: &str = "web.fetch";
pub const TOOL_AGENT_DELEGATE: &str = "agent.delegate";
pub const TOOL_CORE_SEARCH_WEB: &str = "core.search.web";
pub const TOOL_CORE_WEB_PAGE_READ: &str = "core.web.page.read";
// Browser and Computer Use tools
pub const TOOL_CORE_BROWSER_NAVIGATE: &str = "core.browser.navigate";
pub const TOOL_CORE_BROWSER_ACTION: &str = "core.browser.action";
pub const TOOL_CORE_COMPUTER_USE: &str = "core.computer.use";
pub const TOOL_BROWSER_NAVIGATE: &str = "browser.navigate";
pub const TOOL_BROWSER_ACTION: &str = "browser.action";
pub const TOOL_COMPUTER_USE: &str = "computer.use";
pub const TOOL_CORE_VOLUME_CONTROL: &str = "core.computer.volume";
pub const TOOL_VOLUME_CONTROL: &str = "computer.volume";
pub const TOOL_CORE_SCREEN_CAPTURE: &str = "core.computer.screenshot";
pub const TOOL_SCREEN_CAPTURE: &str = "computer.screenshot";
pub const TOOL_CORE_SYSTEM_INFO: &str = "core.computer.info";
pub const TOOL_SYSTEM_INFO: &str = "computer.info";
pub const TOOL_CORE_APP_LAUNCH: &str = "core.computer.launch";
pub const TOOL_APP_LAUNCH: &str = "computer.launch";
pub const TOOL_CORE_NETWORK_INFO: &str = "core.computer.network";
pub const TOOL_NETWORK_INFO: &str = "computer.network";
// Workspace tools
pub const TOOL_CORE_WORKSPACE_WRITE: &str = "core.workspace.write";
pub const TOOL_CORE_WORKSPACE_READ: &str = "core.workspace.read";
pub const TOOL_CORE_WORKSPACE_LIST: &str = "core.workspace.list";
pub const TOOL_CORE_WORKSPACE_GREP: &str = "core.workspace.grep";
pub const TOOL_CORE_WORKSPACE_SEARCH: &str = "core.workspace.search";
// Shell
pub const TOOL_CORE_SHELL_EXECUTE: &str = "core.shell.execute";
// Code execution
pub const TOOL_CORE_CODE_EXECUTE: &str = "core.code.execute";
pub const TOOL_CODE_EXECUTE: &str = "code.execute";
// Document creation
pub const TOOL_CORE_DOCUMENT_CREATE: &str = "core.document.create";
pub const TOOL_DOCUMENT_CREATE: &str = "document.create";
// Memory tools
pub const TOOL_CORE_MEMORY_SAVE: &str = "core.memory.save";
pub const TOOL_CORE_MEMORY_SEARCH: &str = "core.memory.search";
pub const TOOL_CORE_MEMORY_RECALL: &str = "core.memory.recall";
pub const TOOL_CORE_MEMORY_UPDATE: &str = "core.memory.update";
pub const TOOL_CORE_MEMORY_FORGET: &str = "core.memory.forget";
pub const TOOL_CORE_MEMORY_LIST: &str = "core.memory.list";
pub const TOOL_CORE_MEMORY_DELETE: &str = "core.memory.delete";

// Snake_case memory tool aliases
pub const TOOL_MEMORY_SAVE: &str = "memory_save";
pub const TOOL_MEMORY_SEARCH: &str = "memory_search";
pub const TOOL_MEMORY_RECALL: &str = "memory_recall";
pub const TOOL_MEMORY_UPDATE: &str = "memory_update";
pub const TOOL_MEMORY_FORGET: &str = "memory_forget";
pub const TOOL_MEMORY_LIST: &str = "memory_list";
pub const TOOL_MEMORY_DELETE: &str = "memory_delete";

pub const ALIAS_MEMORY_SAVE: &str = "memory_save";
pub const ALIAS_MEMORY_SEARCH: &str = "memory_search";
pub const ALIAS_MEMORY_RECALL: &str = "memory_recall";
pub const ALIAS_MEMORY_UPDATE: &str = "memory_update";
pub const ALIAS_MEMORY_FORGET: &str = "memory_forget";
pub const ALIAS_MEMORY_LIST: &str = "memory_list";
pub const ALIAS_MEMORY_DELETE: &str = "memory_delete";

/// Canonicalizes any recognized memory or core tool identifier or alias to its canonical dotted form.
pub fn normalize_tool_id(tool_id: &str) -> &str {
    match tool_id {
        // Memory tools
        "memory_save" | "memory.save" | "memory.add" | "core.memory.save" => TOOL_CORE_MEMORY_SAVE,
        "memory_search" | "memory.search" | "core.memory.search" => TOOL_CORE_MEMORY_SEARCH,
        "memory_recall" | "memory.recall" | "core.memory.recall" => TOOL_CORE_MEMORY_RECALL,
        "memory_update" | "memory.update" | "core.memory.update" => TOOL_CORE_MEMORY_UPDATE,
        "memory_forget" | "memory.forget" | "core.memory.forget" => TOOL_CORE_MEMORY_FORGET,
        "memory_list" | "memory.list" | "core.memory.list" => TOOL_CORE_MEMORY_LIST,
        "memory_delete" | "memory.delete" | "core.memory.delete" => TOOL_CORE_MEMORY_DELETE,

        // Workspace tools
        "workspace_read" | "workspace.read" | "fs.read" | "fs.read-file" | "core.workspace.read" => {
            TOOL_CORE_WORKSPACE_READ
        }
        "workspace_write" | "workspace.write" | "fs.write" | "fs.create-file" | "core.workspace.write" => {
            TOOL_CORE_WORKSPACE_WRITE
        }
        "workspace_list" | "workspace.list" | "fs.list" | "fs.list-dir" | "core.workspace.list" => {
            TOOL_CORE_WORKSPACE_LIST
        }
        "workspace_grep" | "workspace.grep" | "fs.grep" | "core.workspace.grep" => {
            TOOL_CORE_WORKSPACE_GREP
        }
        "workspace_search" | "workspace.search" | "fs.search" | "core.workspace.search" => {
            TOOL_CORE_WORKSPACE_SEARCH
        }

        // Web tools
        "web_search" | "web.search" | "core.search.web" => TOOL_CORE_SEARCH_WEB,
        "web_fetch" | "web.fetch" | "core.web.page.read" => TOOL_CORE_WEB_PAGE_READ,

        // Browser tools
        "browser_navigate" | "browser.navigate" | "core.browser.navigate" => {
            TOOL_CORE_BROWSER_NAVIGATE
        }
        "browser_action" | "browser.action" | "core.browser.action" => {
            TOOL_CORE_BROWSER_ACTION
        }

        // Computer use tools
        "computer_use" | "computer.use" | "core.computer.use" => {
            TOOL_CORE_COMPUTER_USE
        }
        "volume_control" | "volume.control" | "computer.volume" | "core.computer.volume" | "set_volume" | "get_volume" | "volume_set" | "volume_get" | "volume_up" | "volume_down" | "mute" | "unmute" => {
            TOOL_CORE_VOLUME_CONTROL
        }
        "screen_capture" | "screenshot" | "take_screenshot" | "computer.screenshot" | "core.computer.screenshot" => {
            TOOL_CORE_SCREEN_CAPTURE
        }
        "system_info" | "system.info" | "computer.info" | "core.computer.info" | "system_status" => {
            TOOL_CORE_SYSTEM_INFO
        }
        "app_launch" | "computer.launch" | "core.computer.launch" | "system_launch" | "open_app" => {
            TOOL_CORE_APP_LAUNCH
        }
        "network_info" | "wifi_status" | "wifi" | "computer.network" | "core.computer.network" => {
            TOOL_CORE_NETWORK_INFO
        }

        // Notification & communication tools
        "notification_send" | "notification.send" | "send_notification" | "core.notification.send" => {
            TOOL_CORE_NOTIFICATION_SEND
        }
        "email_send" | "email.send" | "send_email" | "core.email.send" => {
            TOOL_CORE_EMAIL_SEND
        }
        "notification_schedule" | "notification.schedule" | "schedule_notification" | "core.notification.schedule" => {
            TOOL_CORE_NOTIFICATION_SCHEDULE
        }

        other => other,
    }
}

pub fn is_computer_tool(tool_id: &str) -> bool {
    let normalized = normalize_tool_id(tool_id);
    normalized == TOOL_CORE_COMPUTER_USE
        || normalized == TOOL_CORE_VOLUME_CONTROL
        || normalized == TOOL_CORE_SCREEN_CAPTURE
        || normalized == TOOL_CORE_SYSTEM_INFO
        || normalized == TOOL_CORE_NETWORK_INFO
        || normalized == TOOL_CORE_APP_LAUNCH
}

// Notification tools
pub const TOOL_CORE_NOTIFICATION_SEND: &str = "core.notification.send";
pub const TOOL_NOTIFICATION_SEND: &str = "notification.send";
pub const TOOL_CORE_EMAIL_SEND: &str = "core.email.send";
pub const TOOL_EMAIL_SEND: &str = "email.send";
pub const TOOL_CORE_NOTIFICATION_SCHEDULE: &str = "core.notification.schedule";
pub const TOOL_NOTIFICATION_SCHEDULE: &str = "notification.schedule";

pub fn is_notification_tool(tool_id: &str) -> bool {
    let normalized = normalize_tool_id(tool_id);
    normalized == TOOL_CORE_NOTIFICATION_SEND
        || normalized == TOOL_CORE_EMAIL_SEND
        || normalized == TOOL_CORE_NOTIFICATION_SCHEDULE
}


/// Checks if a tool identifier belongs to the memory family.
pub fn is_memory_tool(tool_id: &str) -> bool {
    let normalized = normalize_tool_id(tool_id);
    normalized.starts_with("core.memory.")
        || tool_id.starts_with("core.memory.")
        || tool_id.starts_with("memory.")
        || tool_id.starts_with("memory_")
}
// Context tools
pub const TOOL_CORE_CONTEXT_SEARCH: &str = "core.context.search";
// Orchestration / agent delegation tools
pub const TOOL_CORE_AGENT_DELEGATE: &str = "core.agent.delegate";
pub const TOOL_CORE_AGENT_SPAWN: &str = "core.agent.spawn";
pub const TOOL_CORE_AGENT_STATUS: &str = "core.agent.status";
// MCP (Model Context Protocol) tool bridge
pub const TOOL_CORE_MCP_CALL: &str = "core.mcp.call";
// Skill tools
pub const TOOL_CORE_SKILL_LIST: &str = "core.skill.list";
pub const TOOL_CORE_SKILL_INVOKE: &str = "core.skill.invoke";
// Connector/plugin tools
pub const TOOL_CORE_CONNECTOR_LIST: &str = "core.connector.list";
pub const TOOL_CORE_CONNECTOR_CALL: &str = "core.connector.call";
// Legacy 2-segment aliases (kept for backwards compat with aro-tools router)
pub const TOOL_WORKSPACE_SEARCH: &str = "workspace.search";
pub const TOOL_WORKSPACE_READ: &str = "workspace.read";
pub const TOOL_WORKSPACE_WRITE: &str = "workspace.write";
pub const TOOL_WORKSPACE_LIST: &str = "workspace.list";
pub const TOOL_WORKSPACE_GREP: &str = "workspace.grep";
pub const TOOL_WORKSPACE_DELETE: &str = "workspace.delete";
pub const TOOL_WORKSPACE_REPLACE_IN_FILES: &str = "workspace.replace_in_files";
pub const TOOL_WORKSPACE_GIT_DIFF: &str = "workspace.git_diff";
pub const TOOL_SHELL_EXECUTE: &str = "shell.execute";
pub const TOOL_ARTIFACT_CREATE: &str = "artifact.create";
pub const TOOL_DESCRIPTOR_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolCategory {
    Search,
    Web,
    Data,
    Files,
    Communication,
    Settings,
    Memory,
    Voice,
    Integration,
    Automation,
    System,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ToolPermissionEffect {
    Read,
    ReversibleWrite,
    ExternalRead,
    ExternalWrite,
    Financial,
    Delete,
    Publish,
    Administrative,
    SensitiveData,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolRiskLevel {
    None,
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolConfirmationPolicy {
    #[default]
    Never,
    Policy,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRisk {
    pub level: ToolRiskLevel,
    #[serde(default)]
    pub effects: Vec<ToolPermissionEffect>,
    pub confirmation: ToolConfirmationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionRequirement {
    pub action: String,
    pub resource: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolExecutionKind {
    Backend,
    Frontend,
    Edge,
    PluginWasm,
    PluginRemote,
    Mcp,
    Workflow,
    Admin,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolExecutionEnvironment {
    Api,
    CloudWorker,
    Desktop,
    Browser,
    Sandbox,
    Remote,
    OnPremise,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolIdempotency {
    Unsupported,
    Recommended,
    Required,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolSideEffects {
    ReadOnly,
    Reversible,
    Irreversible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionSpec {
    pub kind: ToolExecutionKind,
    pub handler: String,
    pub environment: ToolExecutionEnvironment,
    pub streaming: bool,
    pub idempotency: ToolIdempotency,
    pub side_effects: ToolSideEffects,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolRetryStrategy {
    None,
    Fixed,
    Exponential,
    ExponentialJitter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRetryPolicy {
    pub max_attempts: u16,
    pub strategy: ToolRetryStrategy,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsageLimits {
    pub max_concurrency: u32,
    pub rate_per_minute: u32,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ToolDependencyKind {
    Tool,
    Connector,
    Runtime,
    Secret,
    Model,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDependency {
    pub kind: ToolDependencyKind,
    pub id: String,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolDataCaptureMode {
    None,
    MetadataOnly,
    ReferenceOnly,
    Redacted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolObservability {
    pub record_input: ToolDataCaptureMode,
    pub record_output: ToolDataCaptureMode,
    pub metrics_namespace: String,
    pub cost_unit: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolStatus {
    Pending,
    Active,
    Disabled,
    Deprecated,
    Revoked,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolProvenanceKind {
    Core,
    Plugin,
    Mcp,
    Skill,
    User,
    Organization,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolProvenance {
    pub kind: ToolProvenanceKind,
    pub package: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolOwnerKind {
    Platform,
    Team,
    Organization,
    User,
    Publisher,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolOwner {
    pub kind: ToolOwnerKind,
    pub id: String,
}

/// Immutable, portable contract for one published tool version.
///
/// Runtime health, tenant installation state, grants and revocation timestamps deliberately live
/// outside this value. This keeps the descriptor hash stable for the lifetime of an execution
/// plan and prevents a remote source from changing a tool's contract after approval.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    pub schema_version: u16,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub input_schema: Value,
    pub output_schema: Value,
    #[serde(default)]
    pub permissions: Vec<ToolPermissionRequirement>,
    pub risk: ToolRisk,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub execution: ToolExecutionSpec,
    pub timeout_ms: u64,
    pub retry: ToolRetryPolicy,
    pub limits: ToolUsageLimits,
    #[serde(default)]
    pub dependencies: Vec<ToolDependency>,
    pub observability: ToolObservability,
    pub status: ToolStatus,
    pub provenance: ToolProvenance,
    pub owner: ToolOwner,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ToolDescriptor {
    pub fn validate(&self) -> AroResult<()> {
        if self.schema_version != TOOL_DESCRIPTOR_SCHEMA_VERSION {
            return Err(invalid_descriptor(format!(
                "unsupported schemaVersion {}; expected {TOOL_DESCRIPTOR_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_dotted_identifier(&self.id, "tool id", 3)?;
        validate_semver(&self.version)?;
        validate_bounded_text(&self.name, "tool name", 1, 120)?;
        validate_bounded_text(&self.description, "tool description", 1, 4_096)?;
        validate_json_schema(&self.input_schema, "inputSchema")?;
        validate_json_schema(&self.output_schema, "outputSchema")?;
        validate_bounded_text(&self.execution.handler, "execution handler", 1, 200)?;
        validate_bounded_text(
            &self.observability.metrics_namespace,
            "metrics namespace",
            1,
            120,
        )?;
        validate_bounded_text(&self.provenance.package, "provenance package", 1, 200)?;
        validate_bounded_text(&self.owner.id, "owner id", 1, 200)?;

        if !(1..=86_400_000).contains(&self.timeout_ms) {
            return Err(invalid_descriptor(
                "timeoutMs must be between 1 ms and 24 hours",
            ));
        }
        if !(1..=20).contains(&self.retry.max_attempts) {
            return Err(invalid_descriptor(
                "retry.maxAttempts must be between 1 and 20",
            ));
        }
        if self.retry.max_delay_ms < self.retry.base_delay_ms {
            return Err(invalid_descriptor(
                "retry.maxDelayMs must be greater than or equal to retry.baseDelayMs",
            ));
        }
        if self.limits.max_concurrency == 0
            || self.limits.rate_per_minute == 0
            || self.limits.max_input_bytes == 0
            || self.limits.max_output_bytes == 0
        {
            return Err(invalid_descriptor("tool limits must be greater than zero"));
        }

        let mut capabilities = BTreeSet::new();
        for capability in &self.capabilities {
            validate_dotted_identifier(capability, "tool capability", 2)?;
            if !capabilities.insert(capability) {
                return Err(invalid_descriptor(format!(
                    "duplicate capability `{capability}`"
                )));
            }
        }

        let mut permissions = BTreeSet::new();
        for permission in &self.permissions {
            validate_dotted_identifier(&permission.action, "permission action", 2)?;
            validate_bounded_text(&permission.resource, "permission resource", 1, 500)?;
            if !permissions.insert((&permission.action, &permission.resource)) {
                return Err(invalid_descriptor(format!(
                    "duplicate permission `{}:{}`",
                    permission.action, permission.resource
                )));
            }
        }

        let mut dependencies = BTreeSet::new();
        for dependency in &self.dependencies {
            validate_bounded_text(&dependency.id, "dependency id", 1, 200)?;
            if !dependencies.insert((&dependency.kind, &dependency.id)) {
                return Err(invalid_descriptor(format!(
                    "duplicate dependency `{:?}:{}`",
                    dependency.kind, dependency.id
                )));
            }
        }

        let mut aliases = BTreeSet::new();
        for alias in &self.aliases {
            validate_tool_alias(alias)?;
            if alias == &self.id || !aliases.insert(alias) {
                return Err(invalid_descriptor(format!(
                    "invalid or duplicate alias `{alias}`"
                )));
            }
        }

        Ok(())
    }
}

fn invalid_descriptor(message: impl Into<String>) -> AroError {
    AroError::Configuration(format!("invalid tool descriptor: {}", message.into()))
}

fn validate_bounded_text(
    value: &str,
    field: &str,
    min_bytes: usize,
    max_bytes: usize,
) -> AroResult<()> {
    if value != value.trim() || !(min_bytes..=max_bytes).contains(&value.len()) {
        return Err(invalid_descriptor(format!(
            "{field} must be trimmed and contain between {min_bytes} and {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_tool_alias(value: &str) -> AroResult<()> {
    validate_bounded_text(value, "tool alias", 1, 200)?;
    if value.contains('.') {
        validate_dotted_identifier(value, "tool alias", 2)
    } else {
        if value.is_empty()
            || !value.bytes().next().is_some_and(|byte| byte.is_ascii_lowercase())
            || !value.bytes().last().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || value.bytes().any(|byte| {
                !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-')
            })
        {
            return Err(invalid_descriptor(format!(
                "tool alias `{value}` must be a valid identifier (lowercase alphanumeric with underscores or hyphens)"
            )));
        }
        Ok(())
    }
}

fn validate_dotted_identifier(value: &str, field: &str, min_segments: usize) -> AroResult<()> {
    validate_bounded_text(value, field, min_segments * 2 - 1, 200)?;
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() < min_segments
        || segments.iter().any(|segment| {
            segment.is_empty()
                || segment.len() > 64
                || !segment
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_lowercase())
                || !segment
                    .bytes()
                    .last()
                    .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                || segment.bytes().any(|byte| {
                    !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                })
        })
    {
        return Err(invalid_descriptor(format!(
            "{field} `{value}` is not a valid dotted namespace"
        )));
    }
    Ok(())
}

fn validate_semver(value: &str) -> AroResult<()> {
    validate_bounded_text(value, "tool version", 5, 64)?;
    let (core, suffix) = value
        .split_once('-')
        .map_or((value, None), |(core, suffix)| (core, Some(suffix)));
    let parts = core.split('.').collect::<Vec<_>>();
    let valid_core = parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        });
    let valid_suffix = suffix.is_none_or(|suffix| {
        !suffix.is_empty()
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    });
    if !valid_core || !valid_suffix {
        return Err(invalid_descriptor(format!(
            "tool version `{value}` is not a supported semantic version"
        )));
    }
    Ok(())
}

fn validate_json_schema(value: &Value, field: &str) -> AroResult<()> {
    let Some(schema) = value.as_object() else {
        return Err(invalid_descriptor(format!(
            "{field} must be a JSON Schema object"
        )));
    };
    if schema.is_empty() {
        return Err(invalid_descriptor(format!("{field} must not be empty")));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolExecutionStatus {
    Running,
    Completed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionRequest {
    pub invocation_id: Uuid,
    pub run_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub tool_id: String,
    #[serde(default)]
    pub input: Value,
    pub requested_at: DateTime<Utc>,
}

impl ToolExecutionRequest {
    pub fn new(
        run_id: Uuid,
        conversation_id: Option<Uuid>,
        tool_id: impl Into<String>,
        input: Value,
    ) -> Self {
        Self {
            invocation_id: Uuid::new_v4(),
            run_id,
            conversation_id,
            tool_id: tool_id.into(),
            input,
            requested_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResult {
    pub invocation_id: Uuid,
    pub run_id: Uuid,
    pub tool_id: String,
    pub status: ToolExecutionStatus,
    pub title: String,
    #[serde(default)]
    pub output: Value,
    pub summary: String,
    #[serde(default)]
    pub context_sources: Vec<ContextSource>,
    #[serde(default)]
    pub artifacts: Vec<AgentArtifact>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub freshness_days: Option<u32>,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResponse {
    pub query: String,
    #[serde(default)]
    pub results: Vec<WebSearchResult>,
    pub provider: String,
    pub searched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub rank: usize,
    pub fetched_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebFetchRequest {
    pub url: String,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebPageSnapshot {
    pub url: String,
    pub final_url: String,
    pub title: String,
    pub excerpt: String,
    pub content: String,
    pub content_hash: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

// ============================================================================
// Agent Memory Tool Contracts (Milestone 3)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySaveInput {
    pub content: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub salience: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySaveOutput {
    pub success: bool,
    pub id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchInput {
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredMemory {
    pub id: Uuid,
    pub content: String,
    pub category: String,
    pub scope: String,
    pub score: f32,
    pub pinned: bool,
    pub salience: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchOutput {
    pub count: usize,
    pub memories: Vec<ScoredMemory>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecallInput {
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default, alias = "entity_key")]
    pub entity_key: Option<String>,
    #[serde(default, alias = "include_episodes")]
    pub include_episodes: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecallOutput {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<crate::LongTermMemory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_episodes: Option<Vec<crate::Episode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdateInput {
    pub id: Uuid,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub salience: Option<f32>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdateOutput {
    pub success: bool,
    pub id: Uuid,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryForgetInput {
    pub id: Uuid,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryForgetOutput {
    pub success: bool,
    pub id: Uuid,
    pub status: String,
}

#[cfg(test)]
mod descriptor_tests {
    use serde_json::json;

    use super::*;

    fn descriptor() -> ToolDescriptor {
        ToolDescriptor {
            schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
            id: TOOL_CORE_SEARCH_WEB.to_string(),
            version: "1.0.0".to_string(),
            name: "Search web".to_string(),
            description: "Search approved public Web providers.".to_string(),
            category: ToolCategory::Search,
            input_schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"],
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": { "results": { "type": "array" } },
                "required": ["results"],
                "additionalProperties": false
            }),
            permissions: vec![ToolPermissionRequirement {
                action: "network.read".to_string(),
                resource: "destination:${input.domains}".to_string(),
            }],
            risk: ToolRisk {
                level: ToolRiskLevel::Low,
                effects: vec![ToolPermissionEffect::ExternalRead],
                confirmation: ToolConfirmationPolicy::Never,
            },
            capabilities: vec!["search.web".to_string(), "search.citations".to_string()],
            execution: ToolExecutionSpec {
                kind: ToolExecutionKind::Backend,
                handler: "search.web.v1".to_string(),
                environment: ToolExecutionEnvironment::CloudWorker,
                streaming: true,
                idempotency: ToolIdempotency::Recommended,
                side_effects: ToolSideEffects::ReadOnly,
            },
            timeout_ms: 15_000,
            retry: ToolRetryPolicy {
                max_attempts: 3,
                strategy: ToolRetryStrategy::ExponentialJitter,
                base_delay_ms: 250,
                max_delay_ms: 4_000,
            },
            limits: ToolUsageLimits {
                max_concurrency: 20,
                rate_per_minute: 120,
                max_input_bytes: 32_768,
                max_output_bytes: 1_048_576,
            },
            dependencies: vec![ToolDependency {
                kind: ToolDependencyKind::Connector,
                id: "search-provider".to_string(),
                optional: false,
            }],
            observability: ToolObservability {
                record_input: ToolDataCaptureMode::MetadataOnly,
                record_output: ToolDataCaptureMode::ReferenceOnly,
                metrics_namespace: "aro_tool_search_web".to_string(),
                cost_unit: Some("request".to_string()),
            },
            status: ToolStatus::Active,
            provenance: ToolProvenance {
                kind: ToolProvenanceKind::Core,
                package: "aro-tools".to_string(),
                signature: None,
            },
            owner: ToolOwner {
                kind: ToolOwnerKind::Team,
                id: "platform-search".to_string(),
            },
            aliases: vec![TOOL_WEB_SEARCH.to_string()],
            tags: vec!["web".to_string(), "read-only".to_string()],
        }
    }

    #[test]
    fn tool_descriptor_v1_validates_and_round_trips() {
        let descriptor = descriptor();
        descriptor.validate().expect("valid descriptor");

        let wire = serde_json::to_value(&descriptor).expect("serialize descriptor");
        let decoded: ToolDescriptor = serde_json::from_value(wire).expect("deserialize descriptor");

        assert_eq!(decoded, descriptor);
    }

    #[test]
    fn tool_descriptor_rejects_invalid_namespace_and_duplicate_capability() {
        let mut invalid_namespace = descriptor();
        invalid_namespace.id = "Web.Search".to_string();
        assert!(invalid_namespace.validate().is_err());

        let mut duplicate = descriptor();
        duplicate.capabilities.push("search.web".to_string());
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn tool_descriptor_rejects_unbounded_retry_and_timeout() {
        let mut invalid = descriptor();
        invalid.retry.max_attempts = 0;
        assert!(invalid.validate().is_err());

        invalid.retry.max_attempts = 1;
        invalid.timeout_ms = 0;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn tool_descriptor_accepts_snake_case_and_dotted_aliases() {
        let mut desc = descriptor();
        desc.aliases = vec!["web_search".to_string(), "custom.search.v2".to_string()];
        assert!(desc.validate().is_ok());

        let mut invalid_alias = descriptor();
        invalid_alias.aliases = vec!["Invalid Space".to_string()];
        assert!(invalid_alias.validate().is_err());
    }
}

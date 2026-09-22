//! Server-side agent tool execution, shared by two entry points:
//!
//! - the REST direct-execution endpoint (`agent_tool_execute`), and
//! - the durable worker loop (`agent_runner`), which previously recorded
//!   tool *requests* without ever executing them or feeding results back.
//!
//! The worker path ([`execute_worker_tool`]) mirrors the desktop runtime
//! contract: authorize against the run's permission profile → execute with a
//! timeout → persist step/artifacts/context → return the result so the model
//! loop can reason over real outputs. Tool families that cannot run safely
//! on shared infrastructure fail closed with an explicit
//! `worker_unsupported_tool` result instead of a fake success.
//!
//! Memory tools use the same Postgres collection code as the REST API.
//! Connector/MCP tools use the same `PluginManager` as the desktop (plus the
//! curated marketplace metadata for display enrichment). Vector reindexing
//! after worker memory writes is intentionally left to the background
//! reindex path; FTS search sees new rows immediately.

use aro_core::{
    AgentRun, AgentStep, AgentStepKind, AgentStepStatus, LongTermMemory, PermissionProfile,
    ToolExecutionRequest, ToolExecutionResult, ToolExecutionStatus, WebAccessMode,
    TOOL_CORE_CONNECTOR_CALL, TOOL_CORE_CONNECTOR_LIST, TOOL_CORE_MCP_CALL,
    TOOL_CORE_MEMORY_DELETE, TOOL_CORE_MEMORY_FORGET, TOOL_CORE_MEMORY_LIST,
    TOOL_CORE_MEMORY_RECALL, TOOL_CORE_MEMORY_SAVE, TOOL_CORE_MEMORY_SEARCH,
    TOOL_CORE_MEMORY_UPDATE, TOOL_CORE_SKILL_INVOKE, TOOL_CORE_SKILL_LIST,
};
use aro_policy::{
    AuditLevel, AuthorizationEvidence, AuthorizationRequest as PolicyAuthorizationRequest,
    DataClassification, DecisionConstraints, EvaluationContext, PolicyEffect, PolicyEngine,
    PolicyLayer, PolicyRule, PolicyTarget, ResourceRef, RiskLevel, SubjectKind, SubjectRef,
};
use aro_store::{AroStore, PersistedCollection};
use aro_tools::{result_context_items, ToolExecutor, WebAccessPolicy};
use aro_vector::{MemoryVectorScope, MemoryVectorService};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::{
    auth::{ApiError, AuthContext},
    ApiState,
};

/// Per-call execution timeout for worker tools (MCP servers can be slow).
pub const WORKER_TOOL_TIMEOUT_SECS: u64 = 90;
/// Consecutive failed/blocked tool calls before the worker fails the job
/// instead of burning the whole step budget on a hopeless loop.
pub const WORKER_MAX_CONSECUTIVE_TOOL_ERRORS: u32 = 3;
/// History feedback is truncated: the model needs signal, not megabytes.
pub const WORKER_HISTORY_SNIPPET_CHARS: usize = 4000;

// ─────────────────────────────────────────────────────────────
// Shared REST machinery (moved verbatim from handlers.rs so both
// entry points run the same authorization, execution and audit).
// ─────────────────────────────────────────────────────────────

pub async fn execute_api_tool(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    request: ToolExecutionRequest,
    policy: &WebAccessPolicy,
    sequence: i32,
) -> Result<ToolExecutionResult, ApiError> {
    let (authorization_request, authorization_decision) =
        authorize_api_tool(state, auth, run, &request, policy).await?;
    let permits_execution = authorization_decision.permits_execution();
    state
        .store
        .record_permission_evaluation(
            auth.tenant_context(),
            &authorization_request,
            &authorization_decision,
            Some(if permits_execution {
                "authorized_pending_execution"
            } else {
                "blocked_before_execution"
            }),
        )
        .await?;

    if !permits_execution {
        let result = blocked_tool_result(
            &request,
            authorization_decision
                .error_code
                .as_deref()
                .unwrap_or("permission_denied"),
            &authorization_decision.reason,
        );
        persist_api_tool_execution(state, auth, run, &request, &result, sequence).await?;
        return Ok(result);
    }

    let result = match state.tools.execute(request.clone(), policy).await {
        Ok(result) => result,
        Err(err) => failed_tool_result(&request, err.to_string()),
    };
    persist_api_tool_execution(state, auth, run, &request, &result, sequence).await?;
    Ok(result)
}

async fn authorize_api_tool(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    request: &ToolExecutionRequest,
    web_policy: &WebAccessPolicy,
) -> Result<
    (
        PolicyAuthorizationRequest,
        aro_policy::AuthorizationDecision,
    ),
    ApiError,
> {
    let now = Utc::now();
    let ephemeral_agent_id = format!("agent-run:{}", run.id);
    let expires_at = now + ChronoDuration::minutes(5);
    let canonical_intent = serde_json::to_vec(&json!({
        "subjectId": ephemeral_agent_id.clone(),
        "action": "network.read",
        "resourceId": request.tool_id.clone(),
        "runId": run.id,
        "conversationId": run.conversation_id,
        "input": request.input.clone(),
    }))
    .map_err(|_| ApiError::internal("tool authorization intent could not be canonicalized"))?;
    let intent_digest = format!("sha256:{:x}", Sha256::digest(canonical_intent));
    let authorization_request = PolicyAuthorizationRequest {
        id: request.invocation_id,
        intent_digest,
        subject: SubjectRef {
            kind: SubjectKind::Agent,
            id: ephemeral_agent_id.clone(),
            actor_user_id: Some(auth.user_id),
            organization_id: auth.organization_id,
            parent_subject_id: None,
            attributes: Default::default(),
        },
        action: "network.read".to_string(),
        resource: ResourceRef {
            kind: "tool".to_string(),
            id: request.tool_id.clone(),
            organization_id: auth.organization_id,
            owner_id: None,
            classification: DataClassification::Internal,
            relations: Default::default(),
            attributes: Default::default(),
        },
        context: EvaluationContext {
            workspace_id: None,
            conversation_id: run.conversation_id,
            agent_id: Some(ephemeral_agent_id.clone()),
            run_id: Some(run.id),
            task_id: None,
            tool_id: Some(request.tool_id.clone()),
            plugin_id: None,
            skill_id: None,
            mcp_server_id: None,
            environment: state.deployment.environment.clone(),
            model_id: run.model_id.clone(),
            provider_id: run.model_provider_id.clone(),
            external_provider: true,
            risk: RiskLevel::Moderate,
            amount_minor: None,
            actions_already_used: 0,
            cost_minor: None,
            budget_already_used_minor: 0,
            device_id: None,
            ip_address: None,
            origin: "api.tool-execution".to_string(),
            attributes: Default::default(),
        },
        requested_scopes: vec!["network.read".to_string()],
        evidence: AuthorizationEvidence::default(),
        requested_at: request.requested_at,
        expires_at: Some(expires_at),
    };

    let mut policies = state
        .store
        .list_active_policy_rules(auth.tenant_context(), now)
        .await?;
    let legacy_constraints = DecisionConstraints {
        read_only: true,
        allowed_resource_ids: vec![request.tool_id.clone()],
        allowed_scopes: vec!["network.read".to_string()],
        allowed_tool_ids: vec![request.tool_id.clone()],
        allowed_environments: vec![state.deployment.environment.clone()],
        max_actions: Some(1),
        valid_until: Some(expires_at),
        external_transfer_allowed: true,
        audit_level: AuditLevel::Enhanced,
        ..DecisionConstraints::default()
    };
    let legacy_target = PolicyTarget {
        subject_ids: vec![ephemeral_agent_id],
        organization_ids: vec![auth.organization_id],
        actions: vec!["network.read".to_string()],
        resource_kinds: vec!["tool".to_string()],
        resource_ids: vec![request.tool_id.clone()],
        environments: vec![state.deployment.environment.clone()],
        tool_ids: vec![request.tool_id.clone()],
        ..PolicyTarget::default()
    };
    let legacy_allowed = web_policy.allow_network && !web_policy.allowed_domains.is_empty();
    policies.push(PolicyRule {
        id: if legacy_allowed {
            "legacy.agent-permission-profile.network-allow".to_string()
        } else {
            "legacy.agent-permission-profile.network-deny".to_string()
        },
        version: 1,
        layer: PolicyLayer::LegacyAdapter,
        priority: 0,
        effect: if legacy_allowed {
            PolicyEffect::Allow
        } else {
            PolicyEffect::Deny
        },
        target: legacy_target,
        constraints: legacy_constraints,
        reason: if legacy_allowed {
            "The legacy agent profile grants one read-only network tool call within its domain allowlist."
                .to_string()
        } else {
            "The effective legacy agent profile does not grant network access with a non-empty domain allowlist."
                .to_string()
        },
        valid_from: Some(now),
        valid_until: Some(expires_at),
        enabled: true,
    });

    let decision = PolicyEngine.evaluate(&authorization_request, &policies, now);
    Ok((authorization_request, decision))
}

pub async fn persist_api_tool_execution(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    request: &ToolExecutionRequest,
    result: &ToolExecutionResult,
    sequence: i32,
) -> Result<(), ApiError> {
    let step = tool_result_step(run, request, result, sequence);
    state
        .store
        .add_agent_step(auth.user_id, auth.organization_id, &step)
        .await?;
    for artifact in &result.artifacts {
        state
            .store
            .add_agent_artifact(auth.user_id, auth.organization_id, artifact)
            .await?;
    }
    for item in result_context_items(result, run.conversation_id) {
        state
            .store
            .add_agent_context_item(auth.user_id, auth.organization_id, &item)
            .await?;
    }
    Ok(())
}

/// Canonical `AgentStep` projection of a tool result, shared by the REST
/// and worker persistence paths so both surfaces render identically.
pub fn tool_result_step(
    run: &AgentRun,
    request: &ToolExecutionRequest,
    result: &ToolExecutionResult,
    sequence: i32,
) -> AgentStep {
    let status = match result.status {
        ToolExecutionStatus::Completed => AgentStepStatus::Completed,
        ToolExecutionStatus::Running => AgentStepStatus::Running,
        ToolExecutionStatus::Failed | ToolExecutionStatus::Blocked => AgentStepStatus::Failed,
    };
    AgentStep {
        id: request.invocation_id,
        run_id: run.id,
        sequence,
        kind: AgentStepKind::Tool,
        status,
        title: result.title.clone(),
        input: json!({
            "toolId": request.tool_id,
            "input": request.input,
            "requestedAt": request.requested_at,
        }),
        output: serde_json::to_value(result).unwrap_or(Value::Null),
        error: result.error.clone(),
        started_at: result.started_at,
        finished_at: Some(result.finished_at),
    }
}

pub fn explicit_web_policy(
    state: &ApiState,
    permission_profile: &aro_core::PermissionProfile,
    web_access: WebAccessMode,
) -> WebAccessPolicy {
    if matches!(web_access, WebAccessMode::Off)
        || !state.agent_web_access_enabled
        || !state.agent_direct_tool_execution_enabled
    {
        WebAccessPolicy::disabled()
    } else {
        WebAccessPolicy::from_permission_profile(permission_profile)
    }
}

pub fn failed_tool_result(request: &ToolExecutionRequest, error: String) -> ToolExecutionResult {
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Failed,
        title: format!("{} failed", request.tool_id),
        output: json!({ "error": error }),
        summary: error.clone(),
        context_sources: Vec::new(),
        artifacts: Vec::new(),
        error: Some(error),
        started_at: request.requested_at,
        finished_at: Utc::now(),
    }
}

pub fn blocked_tool_result(
    request: &ToolExecutionRequest,
    error_code: &str,
    reason: &str,
) -> ToolExecutionResult {
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Blocked,
        title: format!("{} blocked by policy", request.tool_id),
        output: json!({ "errorCode": error_code, "reason": reason }),
        summary: reason.to_string(),
        context_sources: Vec::new(),
        artifacts: Vec::new(),
        error: Some(format!("{error_code}: {reason}")),
        started_at: request.requested_at,
        finished_at: Utc::now(),
    }
}

pub fn memory_from_value(value: serde_json::Value) -> Result<LongTermMemory, ApiError> {
    serde_json::from_value(value).map_err(|err| ApiError::bad_request(err.to_string()))
}

// ─────────────────────────────────────────────────────────────
// Durable worker tool path.
// ─────────────────────────────────────────────────────────────

/// Dependencies the worker loop needs for real tool execution. Built once
/// in `worker()` from the same constructors as the serve path.
#[derive(Clone)]
pub struct WorkerToolDeps {
    pub store: AroStore,
    pub tools: ToolExecutor,
    pub plugins: Arc<aro_plugins::PluginManager>,
    pub vector: MemoryVectorService,
}

/// Best-effort vector indexing after a worker memory write. FTS stays
/// authoritative: a vector failure is logged, never surfaced as a tool
/// error (the memory itself is already durable in Postgres).
async fn index_memory_best_effort(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    memory: &LongTermMemory,
) {
    let scope = MemoryVectorScope::cloud(auth.organization_id, auth.user_id);
    if let Err(err) = deps.vector.index_memory(memory, &scope).await {
        tracing::warn!(?err, memory_id = %memory.id, "worker vector reindex failed");
    }
}

async fn delete_memory_best_effort(deps: &WorkerToolDeps, auth: &AuthContext, id: Uuid) {
    let scope = MemoryVectorScope::cloud(auth.organization_id, auth.user_id);
    if let Err(err) = deps.vector.delete_memory(id, &scope).await {
        tracing::warn!(?err, memory_id = %id, "worker vector delete failed");
    }
}

/// Worker-side authorization matrix. Anything not explicitly allowed fails
/// closed: the model gets a `worker_unsupported_tool` / `worker_denied_tool`
/// result it can reason over, never a fake success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerToolFamily {
    /// Web fetch/search, browser, email, connector/mcp calls: external effects.
    Network,
    /// Shell/code execution on the worker host.
    Shell,
    /// Server-side writes (workspace write/delete, documents, artifacts).
    Write,
    /// Server-side reads (workspace read/list/grep, notifications).
    Read,
    Memory,
    Connector,
    SkillList,
    Blocked,
}

fn classify_worker_tool(normalized_id: &str) -> WorkerToolFamily {
    use WorkerToolFamily::*;
    match normalized_id {
        "core.search.web" | "core.web.page.read" | "core.browser.navigate"
        | "core.browser.action" | "core.email.send" => Network,
        // Skill scripts run python/node/shell interpreters: same bar as code.
        "core.shell.execute" | "core.code.execute" | TOOL_CORE_SKILL_INVOKE
        | "skill.invoke" => Shell,
        "core.workspace.write" | "core.workspace.delete" | "core.workspace.replace_in_files"
        | "core.document.create" | "core.artifact.create" => Write,
        "core.workspace.read" | "core.workspace.list" | "core.workspace.grep"
        | "core.workspace.search" | "core.workspace.git_diff" | "core.notification.send"
        | "core.notification.schedule" => Read,
        TOOL_CORE_MEMORY_SAVE | TOOL_CORE_MEMORY_SEARCH | TOOL_CORE_MEMORY_RECALL
        | TOOL_CORE_MEMORY_UPDATE | TOOL_CORE_MEMORY_FORGET | TOOL_CORE_MEMORY_LIST
        | TOOL_CORE_MEMORY_DELETE => Memory,
        TOOL_CORE_CONNECTOR_LIST => Connector,
        TOOL_CORE_CONNECTOR_CALL | TOOL_CORE_MCP_CALL => Network,
        TOOL_CORE_SKILL_LIST => SkillList,
        _ => match normalized_id {
            // Dotted/legacy aliases the normalizer passes through.
            "connector.list" | "plugin.list" => Connector,
            "connector.call" | "plugin.call" | "integration.call" | "mcp.call" => Network,
            "skill.list" => SkillList,
            _ => Blocked,
        },
    }
}

/// A path is absolute when it cannot resolve against the worker host CWD:
/// POSIX `/…`, UNC/drive `C:\…`, `C:/…`. (`Path::is_absolute` alone is
/// platform-bound: `/tmp/…` is absolute on Linux but not on Windows.)
fn looks_absolute(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if candidate.starts_with('/') || candidate.starts_with('\\') {
        return true;
    }
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

/// Workspace inputs must name an absolute path inside one of the profile's
/// trusted roots. Relative paths resolve against the worker host CWD, which
/// is never a legitimate agent workspace: fail closed. Component-wise
/// prefix comparison (never raw string prefix: `/root-evil` must not match
/// `/root`).
fn workspace_root_allowed(profile: &PermissionProfile, input: &Value) -> Result<(), String> {
    if profile.trusted_roots.is_empty() {
        return Err("permission profile declares no trusted workspace roots".to_string());
    }
    let mut candidates = Vec::new();
    if let Some(obj) = input.as_object() {
        for key in ["path", "root", "dir", "filePath", "file", "cwd"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                if !s.trim().is_empty() {
                    candidates.push(s.trim().to_string());
                }
            }
        }
    }
    if candidates.is_empty() {
        return Err(
            "workspace tools require an explicit absolute path inside a trusted root".to_string(),
        );
    }
    for candidate in &candidates {
        if !looks_absolute(candidate) {
            return Err(format!(
                "workspace path '{candidate}' is not absolute; run it inside a trusted root"
            ));
        }
        let cand_components: Vec<_> = std::path::Path::new(candidate).components().collect();
        let confined = profile.trusted_roots.iter().any(|root| {
            if !looks_absolute(root) {
                return false;
            }
            let root_components: Vec<_> = std::path::Path::new(root).components().collect();
            cand_components.starts_with(&root_components[..])
        });
        if !confined {
            return Err(format!(
                "workspace path '{candidate}' is outside the trusted roots"
            ));
        }
    }
    Ok(())
}

/// Authorize one worker tool call. Returns the web policy to execute with,
/// or a denial reason recorded as a blocked step. `None` profile = deny
/// everything: tool execution is an explicit grant, never a default.
pub fn authorize_worker_tool(
    profile: Option<&PermissionProfile>,
    tool_id: &str,
    input: &Value,
) -> Result<WebAccessPolicy, String> {
    let profile = profile
        .ok_or_else(|| "tool execution requires an explicit permission profile on the run".to_string())?;
    let normalized = aro_core::normalize_tool_id(tool_id.trim());
    match classify_worker_tool(normalized) {
        WorkerToolFamily::Network => {
            if profile.allow_network && !profile.allowed_domains.is_empty() {
                Ok(WebAccessPolicy::from_permission_profile(profile))
            } else {
                Err("permission profile does not allow network access to any domain".to_string())
            }
        }
        WorkerToolFamily::Shell => {
            if profile.allow_shell {
                Ok(WebAccessPolicy::disabled())
            } else {
                Err("permission profile does not allow shell/code execution".to_string())
            }
        }
        WorkerToolFamily::Write => {
            if !profile.allow_write {
                return Err("permission profile does not allow writes".to_string());
            }
            workspace_root_allowed(profile, input)?;
            Ok(WebAccessPolicy::disabled())
        }
        WorkerToolFamily::Read => {
            if !profile.allow_read {
                return Err("permission profile does not allow reads".to_string());
            }
            if normalized.starts_with("core.workspace") {
                workspace_root_allowed(profile, input)?;
            }
            Ok(WebAccessPolicy::disabled())
        }
        WorkerToolFamily::Memory
        | WorkerToolFamily::Connector
        | WorkerToolFamily::SkillList => Ok(WebAccessPolicy::disabled()),
        WorkerToolFamily::Blocked => Err(format!(
            "worker_unsupported_tool: '{tool_id}' cannot run on the cloud worker (desktop-only capability or unknown tool)"
        )),
    }
}

/// Truncated, model-facing rendering of a tool result for loop history.
pub fn tool_result_history_snippet(result: &ToolExecutionResult) -> String {
    let raw = if let Some(error) = result.error.as_deref() {
        format!("failed: {error}")
    } else {
        serde_json::to_string(&result.output).unwrap_or_else(|_| "{}".to_string())
    };
    let mut out: String = raw.chars().take(WORKER_HISTORY_SNIPPET_CHARS).collect();
    if raw.chars().count() > WORKER_HISTORY_SNIPPET_CHARS {
        out.push_str("…[truncated]");
    }
    out
}

/// Execute one worker tool call end to end: authorize → run with timeout →
/// persist step/artifacts/context. Never returns `Err`: every failure mode
/// becomes a recorded `Failed`/`Blocked` result the loop can reason over.
pub async fn execute_worker_tool(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    run: &AgentRun,
    profile: Option<&PermissionProfile>,
    tool_id: &str,
    input: Value,
    sequence: i32,
) -> ToolExecutionResult {
    let policy = match authorize_worker_tool(profile, tool_id, &input) {
        Ok(policy) => policy,
        Err(reason) => {
            let denied =
                ToolExecutionRequest::new(run.id, run.conversation_id, tool_id, input);
            let result = blocked_tool_result(&denied, "worker_denied_tool", &reason);
            persist_worker_tool_execution(deps, auth, run, &denied, &result, sequence).await;
            return result;
        }
    };
    let request = ToolExecutionRequest::new(run.id, run.conversation_id, tool_id, input);
    let dispatch = dispatch_worker_tool(deps, auth, run, &request, &policy);
    let result = match tokio::time::timeout(Duration::from_secs(WORKER_TOOL_TIMEOUT_SECS), dispatch)
        .await
    {
        Ok(result) => result,
        Err(_) => failed_tool_result(
            &request,
            format!("tool execution timed out after {}s", WORKER_TOOL_TIMEOUT_SECS),
        ),
    };
    persist_worker_tool_execution(deps, auth, run, &request, &result, sequence).await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn persist_worker_tool_execution(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    run: &AgentRun,
    request: &ToolExecutionRequest,
    result: &ToolExecutionResult,
    sequence: i32,
) {
    let step = tool_result_step(run, request, result, sequence);
    if let Err(err) = deps
        .store
        .add_agent_step(auth.user_id, auth.organization_id, &step)
        .await
    {
        tracing::warn!(?err, tool_id = %request.tool_id, "worker failed to persist tool step");
        return;
    }
    for artifact in &result.artifacts {
        if let Err(err) = deps
            .store
            .add_agent_artifact(auth.user_id, auth.organization_id, artifact)
            .await
        {
            tracing::warn!(?err, "worker failed to persist tool artifact");
        }
    }
    for item in result_context_items(result, run.conversation_id) {
        if let Err(err) = deps
            .store
            .add_agent_context_item(auth.user_id, auth.organization_id, &item)
            .await
        {
            tracing::warn!(?err, "worker failed to persist tool context item");
        }
    }
}

async fn dispatch_worker_tool(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    _run: &AgentRun,
    request: &ToolExecutionRequest,
    policy: &WebAccessPolicy,
) -> ToolExecutionResult {
    let normalized = aro_core::normalize_tool_id(request.tool_id.trim()).to_string();
    let fail = |msg: String| failed_tool_result(request, msg);
    match normalized.as_str() {
        TOOL_CORE_MEMORY_SAVE
        | TOOL_CORE_MEMORY_SEARCH
        | TOOL_CORE_MEMORY_RECALL
        | TOOL_CORE_MEMORY_UPDATE
        | TOOL_CORE_MEMORY_FORGET
        | TOOL_CORE_MEMORY_LIST
        | TOOL_CORE_MEMORY_DELETE => worker_memory_tool(deps, auth, request).await,
        TOOL_CORE_CONNECTOR_LIST | "connector.list" | "plugin.list" => {
            worker_connector_list(deps, request).await
        }
        TOOL_CORE_CONNECTOR_CALL | TOOL_CORE_MCP_CALL | "connector.call" | "plugin.call"
        | "integration.call" | "mcp.call" => {
            worker_connector_call(deps, request).await
        }
        TOOL_CORE_SKILL_LIST | "skill.list" => worker_skill_list(deps, request).await,
        TOOL_CORE_SKILL_INVOKE | "skill.invoke" => {
            worker_skill_invoke(deps, request).await
        }
        _ => match deps.tools.execute(request.clone(), policy).await {
            Ok(result) => result,
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("is not implemented by the local executor") {
                    fail(format!(
                        "worker_unsupported_tool: '{}' cannot run on the cloud worker (desktop-only capability or unknown tool)",
                        request.tool_id
                    ))
                } else {
                    fail(msg)
                }
            }
        },
    }
}

// ── Worker memory tools (same Postgres code as the REST API) ──

fn pick_present(input: &Value, keys: &[&str]) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(obj) = input.as_object() {
        for key in keys {
            if let Some(value) = obj.get(*key) {
                if !value.is_null() {
                    map.insert((*key).to_string(), value.clone());
                }
            }
        }
    }
    Value::Object(map)
}

fn ok_result(request: &ToolExecutionRequest, title: String, output: Value) -> ToolExecutionResult {
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Completed,
        title: title.clone(),
        output,
        summary: title,
        context_sources: Vec::new(),
        artifacts: Vec::new(),
        error: None,
        started_at: request.requested_at,
        finished_at: Utc::now(),
    }
}

async fn worker_memory_tool(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let fail = |msg: String| failed_tool_result(request, msg);
    let normalized = aro_core::normalize_tool_id(request.tool_id.trim()).to_string();
    let store = &deps.store;
    match normalized.as_str() {
        TOOL_CORE_MEMORY_SAVE => {
            let content = request
                .input
                .get("content")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or_default();
            if content.is_empty() {
                return fail("memory_save requires a non-empty content".to_string());
            }
            let mut payload =
                pick_present(&request.input, &["category", "scope", "pinned", "salience"]);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("content".to_string(), Value::String(content.to_string()));
            }
            match store
                .create_collection_item(
                    auth.user_id,
                    auth.organization_id,
                    PersistedCollection::Memories,
                    payload,
                    None,
                )
                .await
            {
                Ok(value) => match memory_from_worker_value(value) {
                    Ok(memory) => {
                        index_memory_best_effort(deps, auth, &memory).await;
                        ok_result(
                            request,
                            "memory saved".to_string(),
                            json!({ "success": true, "id": memory.id, "content": memory.content }),
                        )
                    }
                    Err(err) => fail(err),
                },
                Err(err) => fail(err.to_string()),
            }
        }
        TOOL_CORE_MEMORY_SEARCH | TOOL_CORE_MEMORY_LIST => {
            let query = request
                .input
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let limit = request
                .input
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(10)
                .clamp(1, 50) as i64;
            match store
                .search_memories(auth.user_id, auth.organization_id, query, limit)
                .await
            {
                Ok(memories) => ok_result(
                    request,
                    format!("memory search returned {} result(s)", memories.len()),
                    json!({ "count": memories.len(), "memories": memories }),
                ),
                Err(err) => fail(err.to_string()),
            }
        }
        TOOL_CORE_MEMORY_RECALL => {
            let Some(id) = request
                .input
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return fail(
                    "memory_recall requires an id (entity-key lookup is a desktop-runtime feature)"
                        .to_string(),
                );
            };
            match store
                .get_collection_item(
                    auth.user_id,
                    auth.organization_id,
                    PersistedCollection::Memories,
                    id,
                )
                .await
            {
                Ok(None) => ok_result(
                    request,
                    "memory not found".to_string(),
                    json!({ "found": false }),
                ),
                Ok(Some(value)) => match memory_from_worker_value(value) {
                    Ok(memory) => ok_result(
                        request,
                        "memory recalled".to_string(),
                        json!({ "found": true, "memory": memory }),
                    ),
                    Err(err) => fail(err),
                },
                Err(err) => fail(err.to_string()),
            }
        }
        TOOL_CORE_MEMORY_UPDATE => {
            let Some(id) = request
                .input
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return fail("memory_update requires an id".to_string());
            };
            let payload = pick_present(
                &request.input,
                &["content", "category", "scope", "pinned", "salience"],
            );
            match store
                .update_collection_item(
                    auth.user_id,
                    auth.organization_id,
                    PersistedCollection::Memories,
                    id,
                    payload,
                    None,
                )
                .await
            {
                Ok(value) => match memory_from_worker_value(value) {
                    Ok(memory) => {
                        index_memory_best_effort(deps, auth, &memory).await;
                        ok_result(
                            request,
                            "memory updated".to_string(),
                            json!({ "success": true, "id": memory.id }),
                        )
                    }
                    Err(err) => fail(err),
                },
                Err(err) => fail(err.to_string()),
            }
        }
        TOOL_CORE_MEMORY_FORGET | TOOL_CORE_MEMORY_DELETE => {
            let Some(id) = request
                .input
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return fail("memory_forget requires an id".to_string());
            };
            // Server contract parity: REST DELETE soft-deletes the row.
            // (The desktop runtime instead flips a local `archived` status.)
            match store
                .delete_collection_item(
                    auth.user_id,
                    auth.organization_id,
                    PersistedCollection::Memories,
                    id,
                )
                .await
            {
                Ok(()) => {
                    delete_memory_best_effort(deps, auth, id).await;
                    ok_result(
                        request,
                        "memory forgotten".to_string(),
                        json!({ "success": true, "id": id, "status": "deleted" }),
                    )
                }
                Err(err) => fail(err.to_string()),
            }
        }
        _ => fail(format!(
            "worker_unsupported_tool: '{}' cannot run on the cloud worker",
            request.tool_id
        )),
    }
}

fn memory_from_worker_value(value: Value) -> Result<LongTermMemory, String> {
    serde_json::from_value(value).map_err(|err| err.to_string())
}

// ── Worker connector/MCP/skill tools (same PluginManager as desktop) ──

fn connector_fallback_icon(plugin_id: &str) -> &'static str {
    let id = plugin_id.to_lowercase();
    if id.contains("google") || id.contains("drive") {
        "📑"
    } else if id.contains("github") {
        "🐙"
    } else if id.contains("slack") {
        "💬"
    } else if id.contains("filesystem") || id.contains("workspace") {
        "📁"
    } else if id.contains("web") || id.contains("search") {
        "🌐"
    } else if id.contains("git") {
        "🐈"
    } else if id.contains("sqlite") || id.contains("database") || id.contains("sql") {
        "🗄️"
    } else if id.contains("python") || id.contains("analytic") {
        "🐍"
    } else if id.contains("code") || id.contains("review") || id.contains("secur") {
        "🛡️"
    } else if id.contains("openai") || id.contains("model") {
        "🤖"
    } else {
        "🧩"
    }
}

async fn worker_connector_list(
    deps: &WorkerToolDeps,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let installed = deps.plugins.list_installed().await;
    let curated = aro_plugins::get_curated_marketplace();
    let mut connectors: Vec<Value> = Vec::new();
    for plugin in &installed {
        let accounts = deps
            .plugins
            .list_accounts(Some(&plugin.id))
            .unwrap_or_default();
        let default_account = accounts
            .iter()
            .find(|a| a.is_default)
            .or_else(|| accounts.first());
        let accounts_json: Vec<Value> = accounts
            .iter()
            .map(|acc| {
                json!({
                    "id": acc.id,
                    "label": acc.label,
                    "accountIdentifier": acc.account_identifier,
                    "email": acc.email,
                    "displayName": acc.display_name,
                    "isDefault": acc.is_default,
                    "status": acc.status,
                    "authMethod": acc.auth_method,
                })
            })
            .collect();
        let active_account_json = default_account.map(|acc| {
            json!({
                "id": acc.id,
                "label": acc.label,
                "accountIdentifier": acc.account_identifier,
                "email": acc.email,
            })
        });
        let curated_entry = curated.iter().find(|m| m.id == plugin.id);
        let icon = curated_entry
            .map(|m| m.icon.clone())
            .or_else(|| plugin.skills.first().and_then(|s| s.icon.clone()))
            .unwrap_or_else(|| connector_fallback_icon(&plugin.id).to_string());
        for server in &plugin.mcp_servers {
            connectors.push(json!({
                "connectorId": plugin.id,
                "pluginName": plugin.name,
                "name": plugin.name,
                "displayName": curated_entry.map(|m| m.name.clone()).unwrap_or_else(|| plugin.name.clone()),
                "icon": icon,
                "description": curated_entry.map(|m| m.description.clone()).or_else(|| plugin.description.clone()).unwrap_or_default(),
                "version": curated_entry.map(|m| m.version.clone()).or_else(|| plugin.version.clone()).unwrap_or_default(),
                "category": curated_entry.map(|m| m.category.clone()).unwrap_or_default(),
                "author": plugin.author,
                "server": server.name,
                "transport": server.transport_type,
                "status": server.status,
                "enabled": plugin.enabled,
                "accounts": accounts_json,
                "activeAccount": active_account_json,
            }));
        }
    }
    let count = connectors.len();
    ok_result(
        request,
        format!("Listed {count} connector(s) from installed plugins"),
        json!({ "connectors": connectors, "count": count }),
    )
}

async fn worker_connector_call(
    deps: &WorkerToolDeps,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let fail = |msg: String| failed_tool_result(request, msg);
    let connector_id = request
        .input
        .get("connector_id")
        .or_else(|| request.input.get("connectorId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let server = request
        .input
        .get("server")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let tool = request
        .input
        .get("tool")
        .or_else(|| request.input.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if tool.is_empty() {
        return fail("connector call requires a tool (or action)".to_string());
    }
    let arguments = request
        .input
        .get("arguments")
        .or_else(|| request.input.get("input"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let installed = deps.plugins.list_installed().await;
    let mut resolved: Option<(String, String)> = None;
    for plugin in &installed {
        if plugin.id == connector_id && !server.is_empty() {
            resolved = Some((plugin.id.clone(), server.clone()));
            break;
        }
        if plugin.id == connector_id
            && server.is_empty()
            && plugin.mcp_servers.len() == 1
        {
            resolved = Some((plugin.id.clone(), plugin.mcp_servers[0].name.clone()));
            break;
        }
        if !connector_id.is_empty() && plugin.mcp_servers.iter().any(|s| s.name == connector_id)
        {
            resolved = Some((plugin.id.clone(), connector_id.clone()));
            break;
        }
        if !server.is_empty()
            && plugin.mcp_servers.iter().any(|s| {
                s.name == server && (connector_id.is_empty() || plugin.id == connector_id)
            })
        {
            resolved = Some((plugin.id.clone(), server.clone()));
            break;
        }
    }
    let Some((plugin_id, server_name)) = resolved else {
        return fail(format!(
            "Connector '{connector_id}' (server '{server}') is not registered in any installed agent plugin"
        ));
    };
    match deps
        .plugins
        .call_mcp_tool(&plugin_id, &server_name, &tool, arguments)
        .await
    {
        Ok(call_result) => {
            let is_error = call_result.is_error.unwrap_or(false);
            let plain = call_result.plain_text();
            if is_error {
                fail(plain)
            } else {
                ok_result(
                    request,
                    format!("Connector call to {server_name}/{tool} completed"),
                    json!({
                        "success": true,
                        "connectorId": plugin_id,
                        "server": server_name,
                        "tool": tool,
                        "content": call_result.content,
                        "text": plain,
                    }),
                )
            }
        }
        Err(err) => fail(format!("Connector call to {server_name}/{tool} failed: {err}")),
    }
}

/// Skill invocation runs through the aro-skills sandbox (staged copy,
/// scrubbed env, timeout, output caps) — never raw subprocess execution.
/// The `allow_shell` gate above already applied; this is belt and braces.
async fn worker_skill_invoke(
    deps: &WorkerToolDeps,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let fail = |msg: String| failed_tool_result(request, msg);
    let skill_id = request
        .input
        .get("skill_id")
        .or_else(|| request.input.get("skillId"))
        .or_else(|| request.input.get("id"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or_default();
    if skill_id.is_empty() {
        return fail("skill invoke requires a skill_id".to_string());
    }
    let args = request
        .input
        .get("input")
        .or_else(|| request.input.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let Some(skill) = deps.plugins.skill_registry().get(skill_id).await else {
        return fail(format!("unknown skill '{skill_id}'"));
    };
    match aro_skills::execute_sandboxed(&skill, &args, &aro_skills::SandboxLimits::default())
        .await
    {
        Ok(output) => {
            if output.success {
                ok_result(
                    request,
                    format!("Skill {} completed", skill.name),
                    json!({
                        "success": true,
                        "skillId": output.skill_id,
                        "output": output.output,
                    }),
                )
            } else {
                fail(output.error.unwrap_or_else(|| "skill failed".to_string()))
            }
        }
        Err(err) => fail(format!("skill sandbox failed: {err}")),
    }
}

async fn worker_skill_list(
    deps: &WorkerToolDeps,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let skills = deps.plugins.skill_registry().list().await;
    ok_result(
        request,
        format!("Listed {} skill(s) from installed plugins", skills.len()),
        json!({ "skills": skills, "count": skills.len() }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use aro_core::AssistantMode;

    fn profile_with(
        read: bool,
        write: bool,
        shell: bool,
        network: bool,
        domains: Vec<String>,
        roots: Vec<String>,
    ) -> PermissionProfile {
        PermissionProfile {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            trusted_roots: roots,
            allowed_domains: domains,
            allow_read: read,
            allow_write: write,
            allow_shell: shell,
            allow_network: network,
            command_approval: aro_core::PermissionCommandApproval::Never,
            redact_secrets: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn open_profile() -> PermissionProfile {
        profile_with(
            true,
            true,
            true,
            true,
            vec!["example.com".to_string()],
            vec!["/tmp/aro-tests".to_string()],
        )
    }

    #[test]
    fn worker_denies_everything_without_a_profile() {
        for tool in [
            "core.memory.save",
            "core.connector.list",
            "core.search.web",
            "core.workspace.read",
        ] {
            let err = authorize_worker_tool(None, tool, &json!({})).unwrap_err();
            assert!(err.contains("permission profile"), "{tool}: {err}");
        }
    }

    #[test]
    fn worker_network_family_requires_allowlist() {
        let locked = profile_with(true, true, true, false, vec![], vec!["/tmp/aro-tests".to_string()]);
        for tool in ["core.search.web", "core.connector.call", "core.mcp.call"] {
            assert!(authorize_worker_tool(Some(&locked), tool, &json!({})).is_err());
        }
        let open = open_profile();
        for tool in ["core.search.web", "core.connector.call", "core.mcp.call"] {
            assert!(authorize_worker_tool(Some(&open), tool, &json!({})).is_ok());
        }
    }

    #[test]
    fn worker_shell_and_write_follow_profile_flags() {
        let read_only = profile_with(true, false, false, false, vec![], vec!["/tmp/aro-tests".to_string()]);
        assert!(authorize_worker_tool(
            Some(&read_only),
            "core.shell.execute",
            &json!({})
        )
        .is_err());
        assert!(authorize_worker_tool(
            Some(&read_only),
            "core.workspace.write",
            &json!({ "path": "/tmp/aro-tests/a.txt" })
        )
        .is_err());
        assert!(authorize_worker_tool(
            Some(&read_only),
            "core.workspace.read",
            &json!({ "path": "/tmp/aro-tests/a.txt" })
        )
        .is_ok());
    }

    #[test]
    fn worker_workspace_paths_must_stay_inside_trusted_roots() {
        let open = open_profile();
        assert!(authorize_worker_tool(
            Some(&open),
            "core.workspace.read",
            &json!({ "path": "/etc/passwd" })
        )
        .is_err());
        assert!(authorize_worker_tool(
            Some(&open),
            "core.workspace.read",
            &json!({ "path": "relative/file.txt" })
        )
        .is_err());
        assert!(authorize_worker_tool(
            Some(&open),
            "core.workspace.read",
            &json!({})
        )
        .is_err());
        assert!(authorize_worker_tool(
            Some(&open),
            "core.workspace.read",
            &json!({ "path": "/tmp/aro-tests/sub/file.txt" })
        )
        .is_ok());
        // Sibling prefix must not match: components, not raw strings.
        assert!(authorize_worker_tool(
            Some(&open),
            "core.workspace.read",
            &json!({ "path": "/tmp/aro-tests-evil/file.txt" })
        )
        .is_err());
        // Drive-letter absolutes resolve on any host.
        assert!(authorize_worker_tool(
            Some(&open_profile_with_root("C:\\work")),
            "core.workspace.read",
            &json!({ "path": "C:\\work\\file.txt" })
        )
        .is_ok());
    }

    fn open_profile_with_root(root: &str) -> PermissionProfile {
        profile_with(
            true,
            true,
            true,
            true,
            vec!["example.com".to_string()],
            vec![root.to_string()],
        )
    }

    #[test]
    fn worker_blocks_desktop_only_and_unknown_tools() {
        let open = open_profile();
        for tool in [
            "core.computer.use",
            "core.agent.delegate",
            "core.context.search",
            "core.plan.create",
            "nope.unknown.tool",
        ] {
            let err = authorize_worker_tool(Some(&open), tool, &json!({})).unwrap_err();
            assert!(err.contains("worker_unsupported_tool"), "{tool}: {err}");
        }
    }

    #[test]
    fn worker_skill_invoke_requires_shell_grant() {
        let read_only = profile_with(true, false, false, false, vec![], vec![]);
        assert!(authorize_worker_tool(Some(&read_only), "core.skill.invoke", &json!({})).is_err());
        assert!(authorize_worker_tool(Some(&open_profile()), "skill.invoke", &json!({})).is_ok());
    }

    #[test]
    fn worker_memory_and_read_families_pass_with_profile() {
        let readers = profile_with(true, false, false, false, vec![], vec![]);
        for tool in [
            "core.memory.save",
            "memory_search",
            "core.connector.list",
            "plugin.list",
            "core.skill.list",
            "core.notification.send",
        ] {
            assert!(authorize_worker_tool(Some(&readers), tool, &json!({})).is_ok(), "{tool}");
        }
    }

    /// End-to-end worker proof against real Postgres (skipped without
    /// `DATABASE_URL`, like the aro-store suite): gate → memory_save
    /// persists → search finds it → connector.list answers → unknown tools
    /// fail explicitly. Step-row persistence reuses the REST-tested
    /// `add_agent_step` path verbatim.
    #[tokio::test]
    async fn worker_memory_save_roundtrip() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            return;
        };
        let store = aro_store::AroStore::connect(&database_url, 5)
            .await
            .expect("connect test store");
        let unique = Uuid::new_v4();
        let principal = store
            .create_user_with_org(aro_store::NewUserWithOrg {
                email: format!("worker-tools-{unique}@aro.local"),
                name: "Worker Tools User".to_string(),
                role_title: None,
                avatar_color: None,
                password_hash: "argon2-test-hash".to_string(),
                organization_name: format!("Worker Tools Org {unique}"),
                organization_domain: None,
                organization_description: None,
            })
            .await
            .expect("create test principal");
        let auth = AuthContext::for_worker(
            principal.user.id,
            principal.active_organization.id,
        )
        .expect("worker auth context");
        let plugins_dir = std::env::temp_dir().join(format!("aro-worker-tools-{unique}"));
        std::fs::create_dir_all(&plugins_dir).expect("plugins temp dir");
        // Disabled vector mode: the e2e proves Postgres durability + FTS
        // recall without depending on Qdrant/Ollama in CI.
        let vector = aro_vector::MemoryVectorService::new(aro_vector::VectorMemoryConfig {
            mode: aro_vector::VectorMemoryMode::Disabled,
            qdrant_url: "http://127.0.0.1:6334".to_string(),
            qdrant_api_key: None,
            embedding_provider: aro_vector::EmbeddingProviderKind::Mock,
            embedding_model: "test".to_string(),
            ollama_endpoint: "http://127.0.0.1:11434".to_string(),
            request_timeout_ms: 1000,
        });
        let deps = WorkerToolDeps {
            store: store.clone(),
            tools: aro_tools::ToolExecutor::try_new(aro_tools::ToolExecutorConfig::default())
                .expect("tool executor"),
            plugins: Arc::new(aro_plugins::PluginManager::new(
                plugins_dir,
                Arc::new(aro_skills::SkillRegistry::new()),
            )),
            vector,
        };
        let run = AgentRun::new(
            "remember the test fact",
            AssistantMode::Chat,
            None,
            None,
            None,
            None,
        );
        let profile = open_profile();

        // No profile → explicit block, never silent.
        let denied = execute_worker_tool(
            &deps,
            &auth,
            &run,
            None,
            "core.memory.save",
            json!({ "content": "should not persist" }),
            1,
        )
        .await;
        assert!(matches!(
            denied.status,
            ToolExecutionStatus::Blocked
        ));

        // Save → completed with an id.
        let saved = execute_worker_tool(
            &deps,
            &auth,
            &run,
            Some(&profile),
            "core.memory.save",
            json!({ "content": "worker e2e fact", "category": "personal" }),
            2,
        )
        .await;
        assert!(
            matches!(saved.status, ToolExecutionStatus::Completed),
            "unexpected: {:?}",
            saved.error
        );
        let saved_id = saved
            .output
            .get("id")
            .and_then(|v| v.as_str())
            .expect("saved id")
            .to_string();

        // Recall through the worker sees the row (FTS, no vector needed).
        let recalled = execute_worker_tool(
            &deps,
            &auth,
            &run,
            Some(&profile),
            "memory_recall",
            json!({ "id": saved_id }),
            3,
        )
        .await;
        assert!(matches!(recalled.status, ToolExecutionStatus::Completed));
        assert_eq!(
            recalled.output.get("found"),
            Some(&json!(true)),
            "recall output: {}",
            recalled.output
        );

        // Connector inventory answers (empty temp manager → count 0).
        let listed = execute_worker_tool(
            &deps,
            &auth,
            &run,
            Some(&profile),
            "connector.list",
            json!({}),
            4,
        )
        .await;
        assert!(matches!(listed.status, ToolExecutionStatus::Completed));
        assert_eq!(listed.output.get("count"), Some(&json!(0)));

        // A scriptless skill runs the pure instruction path, sandboxed.
        deps.plugins
            .skill_registry()
            .register(aro_skills::Skill {
                id: "e2e-probe".to_string(),
                name: "E2E Probe".to_string(),
                description: "Worker e2e probe skill".to_string(),
                icon: None,
                tags: vec![],
                parameters: None,
                instructions: "Echo the ping.".to_string(),
                skill_dir: std::env::temp_dir(),
                scripts: vec![],
            })
            .await;
        let invoked = execute_worker_tool(
            &deps,
            &auth,
            &run,
            Some(&profile),
            "skill.invoke",
            json!({ "skill_id": "e2e-probe", "input": { "ping": 7 } }),
            5,
        )
        .await;
        assert!(
            matches!(invoked.status, ToolExecutionStatus::Completed),
            "unexpected: {:?}",
            invoked.error
        );
        assert!(
            invoked.output.to_string().contains("E2E Probe"),
            "output: {}",
            invoked.output
        );

        // Unknown tools are denied at the gate (Blocked, never executed),
        // with a code the model can use.
        let unknown = execute_worker_tool(
            &deps,
            &auth,
            &run,
            Some(&profile),
            "core.teleport.somewhere",
            json!({}),
            6,
        )
        .await;
        assert!(matches!(unknown.status, ToolExecutionStatus::Blocked));
        assert!(
            unknown
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("worker_unsupported_tool"),
            "unexpected: {:?}",
            unknown.error
        );
    }

    #[test]
    fn history_snippets_truncate_without_splitting_chars() {
        let request = ToolExecutionRequest::new(
            Uuid::new_v4(),
            None,
            "core.search.web",
            json!({}),
        );
        let big = "é".repeat(WORKER_HISTORY_SNIPPET_CHARS + 100);
        let result = ok_result(
            &request,
            "done".to_string(),
            json!({ "text": big }),
        );
        let snippet = tool_result_history_snippet(&result);
        assert!(snippet.ends_with("…[truncated]"));
        assert!(snippet.chars().count() <= WORKER_HISTORY_SNIPPET_CHARS + 13);
    }
}

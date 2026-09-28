# Technical Analysis: Feature 5 — Kernel-Grade Tool Authorization Guard & Sandboxing

**Date**: 2026-09-25  
**Author**: teamwork_preview_explorer_m2_1  
**Target Milestone**: Milestone 2 (Kernel-Grade Tool Authorization Guard & Sandboxing)  
**Corpus/Repo**: Hakille-ai/Aro (`c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro`)  

---

## 1. Executive Summary

Feature 5 establishes a kernel-grade authorization interceptor preventing unauthorized tool invocations before execution occurs. By enforcing active permission presets (`Standard`, `ReadOnly`, `Developer`, `Sandbox`, `Custom`), blacklists, and whitelists, the runtime guarantees that sub-agents and orchestrator runs operate strictly within their security boundaries.

This analysis provides the exact architectural plan, concrete file paths, line numbers, function signatures, data structures, and implementation diffs required to implement and verify Feature 5 without regression across both Rust crates and the existing TypeScript E2E test suites (`tests/e2e/helpers/security-guard-engine.ts`, `tier1-feature-coverage.test.ts`, `tier2-boundary-corner.test.ts`, `tier3-cross-feature.test.ts`, and `tier4-application-scenarios.test.ts`).

---

## 2. Architecture & File Placement

### 2.1 File Map & Responsibilities

| File Path | Component | Responsibility |
|---|---|---|
| `crates/aro-tools/src/security.rs` (NEW) | Core Security Guard Module | Defines `PermissionPreset`, `ToolAuthorizationGuard`, `ToolAuthorizationError`, `ToolCategory`, `classify_tool`, and `PermissionDecision`. |
| `crates/aro-tools/src/lib.rs` | Public Crate Re-exports | Re-exports all security types from `security.rs` so consumers (`aro-runtime`, `aro-agent`, desktop) import `aro_tools::{PermissionPreset, ToolAuthorizationGuard, ToolAuthorizationError}`. |
| `crates/aro-memory/src/lib.rs` | SQLite Profile Store | Adds `get_permission_profile(&self, id: Uuid)` and `get_permission_profile_by_name(&self, name: &str)` to complement existing `upsert_permission_profile`. |
| `crates/aro-runtime/src/lib.rs` | Kernel Pre-Execution Interception | Wires `ToolAuthorizationGuard` into `execute_tool_request` (lines 1505–1588) to intercept tool execution BEFORE invocation, store blocked steps, record error escalation envelopes, and abort runs on security violations. |
| `apps/desktop/src-tauri/src/commands.rs` | Directive Dispatch Bridge | Wires `agent_dispatch_directive` to look up the subagent's cognitive memory context, extract `permission_profile_id`, and set `autonomy_profile_id` on the `AgentRunStartRequest`. |
| `crates/aro-runtime/tests/tool_authorization_guard_tests.rs` (NEW) | Rust Integration Tests | Comprehensive integration tests verifying all 5 presets, priority overrides, and runtime interception invariants. |

---

## 3. Data Structures & Contract Design

### 3.1 `PermissionPreset` Enum (`crates/aro-tools/src/security.rs`)

Matches the TypeScript schema `PermissionPresetMode = "standard" | "read-only" | "developer" | "sandbox" | "custom"` in `packages/contracts/src/agent.ts:376`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionPreset {
    Standard,
    #[serde(rename = "read-only", alias = "readonly", alias = "read_only")]
    ReadOnly,
    Developer,
    Sandbox,
    Custom,
}

impl PermissionPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::ReadOnly => "read-only",
            Self::Developer => "developer",
            Self::Sandbox => "sandbox",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for PermissionPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PermissionPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "read-only" | "readonly" | "read_only" | "lecture seule" => Ok(Self::ReadOnly),
            "developer" | "dev" | "autonome" => Ok(Self::Developer),
            "sandbox" | "isolé" | "isole" => Ok(Self::Sandbox),
            "custom" | "personnalisé" => Ok(Self::Custom),
            other => Err(format!("unknown permission preset '{other}'")),
        }
    }
}
```

### 3.2 `ToolAuthorizationError` Struct (`crates/aro-tools/src/security.rs`)

Matches `ToolAuthorizationError` in `tests/e2e/helpers/security-guard-engine.ts:6`:

```rust
use aro_core::AroError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAuthorizationError {
    pub tool_name: String,
    pub reason: String,
    pub preset: PermissionPreset,
}

impl ToolAuthorizationError {
    pub fn new(tool_name: impl Into<String>, reason: impl Into<String>, preset: PermissionPreset) -> Self {
        Self {
            tool_name: tool_name.into(),
            reason: reason.into(),
            preset,
        }
    }
}

impl std::fmt::Display for ToolAuthorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tool Authorization Denied [{}] under preset '{}': {}",
            self.tool_name, self.preset, self.reason
        )
    }
}

impl std::error::Error for ToolAuthorizationError {}

impl From<ToolAuthorizationError> for AroError {
    fn from(err: ToolAuthorizationError) -> Self {
        AroError::Security(err.to_string())
    }
}
```

### 3.3 `ToolCategory` & `classify_tool` (`crates/aro-tools/src/security.rs`)

Classifies tools into logical categories with sensitivity ordering:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    Shell,
    Network,
    Write,
    Read,
    Other,
}

pub fn classify_tool(tool_name: &str) -> ToolCategory {
    let lower = tool_name.trim().to_lowercase();

    // 1. Shell & command execution tools (highest sensitivity check)
    if lower.contains("shell")
        || lower.contains("bash")
        || lower.contains("terminal")
        || lower.contains("cmd")
        || lower.contains("exec")
        || lower.contains("run_command")
        || lower.contains("core.shell.execute")
        || lower.contains("core.code.execute")
    {
        return ToolCategory::Shell;
    }

    // 2. Network & external communication tools
    if lower.contains("web")
        || lower.contains("http")
        || lower.contains("download")
        || lower.contains("curl")
        || lower.contains("browser")
        || lower.contains("external.fetch")
    {
        return ToolCategory::Network;
    }

    // 3. File write / mutation tools
    if lower.contains("write")
        || lower.contains("edit")
        || lower.contains("replace")
        || lower.contains("patch")
        || lower.contains("create")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("save")
        || lower.contains("workspace.write")
        || lower.contains("workspace.delete")
        || lower.contains("workspace.replace_in_files")
        || lower.contains("artifact.create")
        || lower.contains("artifact.update")
    {
        return ToolCategory::Write;
    }

    // 4. File read / inspection tools
    if lower.contains("read")
        || lower.contains("view")
        || lower.contains("search")
        || lower.contains("list")
        || lower.contains("inspect")
        || lower.contains("workspace.read")
        || lower.contains("workspace.list_dir")
        || lower.contains("workspace.git_diff")
        || lower.contains("artifact.list")
    {
        return ToolCategory::Read;
    }

    // 5. Other tools (e.g. core.agent.status, cognitive memory recall, planning)
    ToolCategory::Other
}
```

### 3.4 `ToolAuthorizationGuard` Struct & `check_permission` Implementation

```rust
use std::collections::HashSet;
use aro_core::PermissionProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    pub allowed: bool,
    pub reason: Option<String>,
}

impl PermissionDecision {
    pub fn allowed() -> Self {
        Self { allowed: true, reason: None }
    }

    pub fn allowed_with_reason(reason: impl Into<String>) -> Self {
        Self { allowed: true, reason: Some(reason.into()) }
    }
}

#[derive(Debug, Clone)]
pub struct ToolAuthorizationGuard {
    pub preset: PermissionPreset,
    pub profile: Option<PermissionProfile>,
    pub allowed_tools: Option<HashSet<String>>,
    pub denied_tools: Option<HashSet<String>>,
}

impl Default for ToolAuthorizationGuard {
    fn default() -> Self {
        Self::standard()
    }
}

impl ToolAuthorizationGuard {
    pub fn new(preset: PermissionPreset) -> Self {
        Self {
            preset,
            profile: None,
            allowed_tools: None,
            denied_tools: None,
        }
    }

    pub fn standard() -> Self {
        Self::new(PermissionPreset::Standard)
    }

    pub fn read_only() -> Self {
        Self::new(PermissionPreset::ReadOnly)
    }

    pub fn developer() -> Self {
        Self::new(PermissionPreset::Developer)
    }

    pub fn sandbox() -> Self {
        Self::new(PermissionPreset::Sandbox)
    }

    pub fn custom(profile: PermissionProfile) -> Self {
        Self {
            preset: PermissionPreset::Custom,
            profile: Some(profile),
            allowed_tools: None,
            denied_tools: None,
        }
    }

    pub fn with_allowed_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_tools = Some(tools.into_iter().map(|s| s.into().trim().to_lowercase()).collect());
        self
    }

    pub fn with_denied_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.denied_tools = Some(tools.into_iter().map(|s| s.into().trim().to_lowercase()).collect());
        self
    }

    /// Evaluates permission and returns Ok(()) if authorized, or Err(ToolAuthorizationError) if blocked.
    pub fn check_permission(&self, tool_name: &str) -> Result<(), ToolAuthorizationError> {
        let decision = self.evaluate(tool_name)?;
        if decision.allowed {
            Ok(())
        } else {
            Err(ToolAuthorizationError::new(
                tool_name,
                decision.reason.unwrap_or_else(|| "Operation forbidden".to_string()),
                self.preset,
            ))
        }
    }

    /// Evaluates permission returning a PermissionDecision or ToolAuthorizationError.
    pub fn evaluate(&self, tool_name: &str) -> Result<PermissionDecision, ToolAuthorizationError> {
        let trimmed = tool_name.trim();
        if trimmed.is_empty() {
            return Err(ToolAuthorizationError::new(
                if tool_name.is_empty() { "<empty>" } else { tool_name },
                "Tool name cannot be empty",
                self.preset,
            ));
        }

        let normalized = trimmed.to_lowercase();

        // 1. Explicit denied tools list takes highest priority (overrides all presets)
        if let Some(ref denied) = self.denied_tools {
            if denied.contains(&normalized) {
                return Err(ToolAuthorizationError::new(
                    tool_name,
                    "Tool explicitly blacklisted in denied_tools",
                    self.preset,
                ));
            }
        }

        // 2. Explicit allowed tools list (if set) acts as a strict whitelist
        if let Some(ref allowed) = self.allowed_tools {
            if !allowed.contains(&normalized) {
                return Err(ToolAuthorizationError::new(
                    tool_name,
                    "Tool not found in explicit allowed_tools whitelist",
                    self.preset,
                ));
            }
        }

        let category = classify_tool(&normalized);

        // 3. Preset checks
        match self.preset {
            PermissionPreset::Sandbox => {
                match category {
                    ToolCategory::Shell => Err(ToolAuthorizationError::new(
                        tool_name,
                        "Command execution is forbidden in Sandbox mode",
                        self.preset,
                    )),
                    ToolCategory::Write => Err(ToolAuthorizationError::new(
                        tool_name,
                        "File write and mutation operations are forbidden in Sandbox mode",
                        self.preset,
                    )),
                    ToolCategory::Network => Err(ToolAuthorizationError::new(
                        tool_name,
                        "External network access is forbidden in Sandbox mode",
                        self.preset,
                    )),
                    ToolCategory::Read if !normalized.contains("context") => Err(ToolAuthorizationError::new(
                        tool_name,
                        "Local filesystem access is forbidden in Sandbox mode",
                        self.preset,
                    )),
                    _ => Ok(PermissionDecision::allowed()),
                }
            }

            PermissionPreset::ReadOnly => {
                match category {
                    ToolCategory::Shell => Err(ToolAuthorizationError::new(
                        tool_name,
                        "Command execution is forbidden in Read-Only mode",
                        self.preset,
                    )),
                    ToolCategory::Write => Err(ToolAuthorizationError::new(
                        tool_name,
                        "File write and mutation operations are forbidden in Read-Only mode",
                        self.preset,
                    )),
                    ToolCategory::Network => Err(ToolAuthorizationError::new(
                        tool_name,
                        "External network access is forbidden in Read-Only mode",
                        self.preset,
                    )),
                    ToolCategory::Read | ToolCategory::Other => Ok(PermissionDecision::allowed()),
                }
            }

            PermissionPreset::Developer => {
                // Full access across read, write, shell, and network
                Ok(PermissionDecision::allowed())
            }

            PermissionPreset::Custom => {
                let profile = self.profile.as_ref().ok_or_else(|| {
                    ToolAuthorizationError::new(
                        tool_name,
                        "Custom preset requires an active AgentPermissionProfile",
                        self.preset,
                    )
                })?;

                match category {
                    ToolCategory::Read if !profile.allow_read => Err(ToolAuthorizationError::new(
                        tool_name,
                        format!("File reads prohibited by profile \"{}\"", profile.name),
                        self.preset,
                    )),
                    ToolCategory::Write if !profile.allow_write => Err(ToolAuthorizationError::new(
                        tool_name,
                        format!("File writes prohibited by profile \"{}\"", profile.name),
                        self.preset,
                    )),
                    ToolCategory::Shell if !profile.allow_shell => Err(ToolAuthorizationError::new(
                        tool_name,
                        format!("Shell execution prohibited by profile \"{}\"", profile.name),
                        self.preset,
                    )),
                    ToolCategory::Network if !profile.allow_network => Err(ToolAuthorizationError::new(
                        tool_name,
                        format!("Network access prohibited by profile \"{}\"", profile.name),
                        self.preset,
                    )),
                    _ => Ok(PermissionDecision::allowed()),
                }
            }

            PermissionPreset::Standard => {
                match category {
                    ToolCategory::Shell => Ok(PermissionDecision::allowed_with_reason(
                        "Requires user confirmation before execution",
                    )),
                    _ => Ok(PermissionDecision::allowed()),
                }
            }
        }
    }
}
```

---

## 4. Permission Matrix Analysis

| Preset | Read Files (`workspace.read`, `workspace.list_dir`) | Write / Mutate (`workspace.write`, `delete`) | Shell / Code Execution (`core.shell.execute`, `code.execute`) | Network (`web.fetch`, `browser.navigate`) | Other (`core.agent.status`, context) |
|---|---|---|---|---|---|
| **`ReadOnly`** | **ALLOWED** | **DENIED** (`File write and mutation operations are forbidden in Read-Only mode`) | **DENIED** (`Command execution is forbidden in Read-Only mode`) | **DENIED** (`External network access is forbidden in Read-Only mode`) | **ALLOWED** |
| **`Standard`** | **ALLOWED** | **ALLOWED** | **ALLOWED** (Flags: `Requires user confirmation before execution`) | **ALLOWED** | **ALLOWED** |
| **`Developer`** | **ALLOWED** | **ALLOWED** | **ALLOWED** (Autonomous: `commandApproval: never`) | **ALLOWED** | **ALLOWED** |
| **`Sandbox`** | **DENIED** (`Local filesystem access is forbidden in Sandbox mode`) | **DENIED** (`File write and mutation operations are forbidden in Sandbox mode`) | **DENIED** (`Command execution is forbidden in Sandbox mode`) | **DENIED** (`External network access is forbidden in Sandbox mode`) | **ALLOWED** (In-memory context / prompts only) |
| **`Custom`** | `profile.allow_read` | `profile.allow_write` | `profile.allow_shell` | `profile.allow_network` | **ALLOWED** |

### Priority & Override Resolution:
1. **Empty / Whitespace**: Rejects immediately with `ToolAuthorizationError("Tool name cannot be empty")`.
2. **`denied_tools` Blacklist**: Always takes highest precedence. Even under `Developer` mode, any tool listed in `denied_tools` is rejected immediately.
3. **`allowed_tools` Whitelist**: If present, any tool not present in `allowed_tools` is rejected immediately (`Tool not found in explicit allowed_tools whitelist`).
4. **Preset Policy**: Preset restrictions are evaluated according to `classify_tool`.

---

## 5. Runtime Pre-Execution Interception Design

### 5.1 The Single Kernel Choke-Point

In `crates/aro-runtime/src/lib.rs`, all tool execution funnels into `execute_tool_request` (lines 1505–1588):

```
Model Generation -> AgentAction (Tool) -> execute_agent_tool -> execute_tool_request
API / IPC Call   -> ToolExecutionRequest  -> execute_tool        -> execute_tool_request
Prefetch Engine  -> ToolExecutionRequest  -> prefetch_web_context -> execute_tool_request
```

### 5.2 Resolution of Active Guard from Run Metadata

Add `resolve_guard_for_run` in `crates/aro-runtime/src/lib.rs`:

```rust
impl AssistantEngine {
    pub fn resolve_guard_for_run(&self, run: &AgentRun) -> ToolAuthorizationGuard {
        if let Some(profile_id) = run.autonomy_profile_id {
            if let Ok(Some(profile)) = self.store.get_permission_profile(profile_id) {
                return match profile.name.trim().to_lowercase().as_str() {
                    "read-only" | "readonly" | "read_only" | "lecture seule" => {
                        ToolAuthorizationGuard::read_only()
                    }
                    "sandbox" | "isolé" | "isole" => ToolAuthorizationGuard::sandbox(),
                    "developer" | "autonome" => ToolAuthorizationGuard::developer(),
                    "standard" => ToolAuthorizationGuard::standard(),
                    _ => ToolAuthorizationGuard::custom(profile),
                };
            }
        }
        ToolAuthorizationGuard::standard()
    }
}
```

### 5.3 Interception Implementation inside `execute_tool_request`

In `crates/aro-runtime/src/lib.rs:1513`, insert the guard check before any branching to tool implementations:

```rust
    async fn execute_tool_request(
        &self,
        run: &AgentRun,
        request: ToolExecutionRequest,
        policy: &WebAccessPolicy,
        sequence: i32,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<ToolExecutionResult> {
        let guard = self.resolve_guard_for_run(run);

        // Kernel-Grade Pre-Execution Guard Interception
        if let Err(auth_err) = guard.check_permission(&request.tool_id) {
            let error_msg = auth_err.to_string();
            tracing::warn!(
                run_id = %run.id,
                tool_id = %request.tool_id,
                preset = %auth_err.preset,
                "ToolAuthorizationGuard blocked tool execution: {}",
                error_msg
            );

            // 1. Synthesize blocked result (never invokes underlying tool)
            let blocked_result = ToolExecutionResult {
                invocation_id: request.invocation_id,
                run_id: request.run_id,
                tool_id: request.tool_id.clone(),
                status: ToolExecutionStatus::Blocked,
                title: format!("Tool {} authorization denied", request.tool_id),
                output: json!({
                    "error": error_msg,
                    "tool": request.tool_id,
                    "preset": auth_err.preset.as_str(),
                    "reason": auth_err.reason,
                }),
                summary: error_msg.clone(),
                context_sources: vec![ContextSource {
                    id: format!("tool:{}:blocked", request.invocation_id),
                    kind: "security-interception".to_string(),
                    title: format!("Blocked tool: {}", request.tool_id),
                    excerpt: error_msg.clone(),
                    uri: None,
                    score: 0.0,
                    created_at: Some(Utc::now()),
                }],
                artifacts: Vec::new(),
                error: Some(error_msg.clone()),
                started_at: request.requested_at,
                finished_at: Utc::now(),
            };

            // 2. Persist blocked tool step to SQLite and notify UI listeners
            self.store_tool_execution(run, &request, &blocked_result, sequence, on_step)?;

            // 3. Dispatch an error_escalation envelope to cognitive memory ledger (Combo 3 invariant)
            if let Some(conv_id) = run.conversation_id {
                let conv_str = conv_id.to_string();
                let agent_str = run.id.to_string();
                let envelope = AgentMessageEnvelope {
                    id: Uuid::new_v4().to_string(),
                    conversation_id: conv_str,
                    parent_message_id: None,
                    correlation_id: None,
                    sender: AgentParticipant::subagent(
                        &agent_str,
                        "Security Guard",
                        "kernel",
                        None,
                    ),
                    recipient: AgentParticipant::orchestrator("orchestrator", "Aro Orchestrator"),
                    message_type: AgentMessageType::ErrorEscalation,
                    payload: AgentMessagePayload::text(&format!(
                        "Security Guard Interception on step {}: {}",
                        sequence, error_msg
                    )),
                    permission_profile_id: run.autonomy_profile_id.map(|id| id.to_string()),
                    priority: Some(AgentRunPriority::High),
                    timestamp: Utc::now(),
                };
                let _ = self.store.record_agent_envelope(&envelope);
            }

            // 4. Return Security error to abort invocation immediately
            return Err(AroError::Security(error_msg));
        }

        // Proceed to execute tool only if authorized...
        let tool_id = request.tool_id.as_str();
        let mut result = if tool_id.starts_with("core.plan.") || tool_id.starts_with("plan.") {
            ...
```

### 5.4 Sub-Agent Execution Loop Error Handling

In `spawn_subagent_task` (`crates/aro-runtime/src/lib.rs:344–348`), when `execute_agent_tool` returns `Err(AroError::Security(err))`:
- The sub-agent loop catches the error.
- Updates `run.status = AgentRunStatus::Failed`.
- Updates `run.last_error = Some(err.to_string())`.
- Saves to SQLite via `self.store.upsert_agent_run(&run)`.
- Halts execution and exits the loop immediately.

### 5.5 IPC Directive Dispatch Integration

In `apps/desktop/src-tauri/src/commands.rs:89–122` (`agent_dispatch_directive`):
```rust
    let existing_memory = state
        .engine
        .memory_store()
        .get_agent_memory(&conversation_id, &agent_id)
        .map_err(|e| e.to_string())?;

    let autonomy_profile_id = existing_memory
        .as_ref()
        .and_then(|m| m.permission_profile_id.as_deref())
        .and_then(|id| Uuid::parse_str(id).ok());

    let request = AgentRunStartRequest {
        run_id: None,
        lane_id: None,
        conversation_id: parsed_conv_id,
        goal: directive.clone(),
        mode: AssistantMode::Code,
        system_prompt: None,
        model_id: None,
        provider: None,
        autonomy_profile_id,
        priority: Some(AgentRunPriority::Normal),
        max_steps: None,
    };
```
This guarantees that whenever a subagent is assigned a permission profile during delegation or memory creation, every future directive dispatched to that subagent inherits and enforces that exact permission profile.

---

## 6. SQLite Memory Store Extension (`crates/aro-memory/src/lib.rs`)

`crates/aro-memory/src/lib.rs` currently contains table `agent_permission_profiles` (line 197) and `upsert_permission_profile` (line 2170), but lacks getter methods. Add:

```rust
    pub fn get_permission_profile(&self, id: Uuid) -> AroResult<Option<PermissionProfile>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT profile_json FROM agent_permission_profiles WHERE id = ?1")
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let result = stmt
            .query_row(params![id.to_string()], |row| {
                let json_str: String = row.get(0)?;
                serde_json::from_str(&json_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                })
            })
            .optional()
            .map_err(|err| AroError::Memory(err.to_string()))?;

        Ok(result)
    }

    pub fn get_permission_profile_by_name(&self, name: &str) -> AroResult<Option<PermissionProfile>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT profile_json FROM agent_permission_profiles WHERE lower(name) = lower(?1)")
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let result = stmt
            .query_row(params![name.trim()], |row| {
                let json_str: String = row.get(0)?;
                serde_json::from_str(&json_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                })
            })
            .optional()
            .map_err(|err| AroError::Memory(err.to_string()))?;

        Ok(result)
    }
```

---

## 7. Verification Method

Once implemented by the coder agent, verification can be independently performed through:

1. **Rust Crate Compilation & Clippy**:
   ```powershell
   cargo check -p aro-tools -p aro-runtime -p aro-memory
   cargo clippy -p aro-tools -p aro-runtime -p aro-memory -- -D warnings
   ```
2. **Dedicated Tool Authorization Guard Integration Tests**:
   Create and run `crates/aro-runtime/tests/tool_authorization_guard_tests.rs`:
   ```powershell
   cargo test -p aro-runtime --test tool_authorization_guard_tests
   ```
3. **End-to-End TypeScript Suite**:
   ```powershell
   npm run test:e2e
   # Or targeted test runs:
   npx vitest run tests/e2e/tier1-feature-coverage.test.ts
   npx vitest run tests/e2e/tier2-boundary-corner.test.ts
   npx vitest run tests/e2e/tier3-cross-feature.test.ts
   npx vitest run tests/e2e/tier4-application-scenarios.test.ts
   ```

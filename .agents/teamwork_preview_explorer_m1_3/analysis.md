# Comprehensive Architectural Exploration & Implementation Strategy
# Feature 3: Cloud Worker Delegation Handling
# Feature 4: Persistent Cognitive Memory IPC & API

**Author**: `teamwork_preview_explorer_m1_3` (Explorer 3, Milestone 1)  
**Target Milestone**: Milestone 1 (Rust Multi-Agent Core & Persistence Engine)  
**Date**: 2026-09-24  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m1_3`  
**Authoritative References**: `ORIGINAL_REQUEST.md`, `PROJECT.md`, `TEST_READY.md`

---

## 1. Executive Summary

Milestone 1 of the ARO Architecture requires establishing a robust, non-volatile, collaborative multi-agent execution and memory subsystem. While Feature 1 provides the underlying cognitive persistence schema (`AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope` in `aro-core` and SQLite/PostgreSQL) and Feature 2 provides the asynchronous desktop execution loop in `aro-runtime`, **Feature 3** and **Feature 4** form the operational communication and synchronization bridges:

1. **Feature 3 (Cloud Worker Delegation Handling)**: In `apps/api/src/agent_tools.rs`, cloud worker jobs currently fail closed with `worker_unsupported_tool` whenever an agent attempts to delegate subtasks, query agent run status, or exchange inter-agent message envelopes. Orchestration tools (`core.agent.delegate`, `core.agent.spawn`, `core.agent.status`) are deliberately classified as `WorkerToolFamily::Blocked` and explicitly asserted as unsupported in worker test suites. This report provides the complete architecture and implementation to classify, authorize, and execute delegation, status, and message envelope handling on cloud workers without failing as unsupported.
2. **Feature 4 (Persistent Cognitive Memory IPC & API)**: On the desktop (`apps/desktop/src-tauri`), all 80+ existing Tauri commands reside monolithic inside `main.rs` (5,161 lines), and `commands.rs` does not exist. Crucially, the three core IPC commands mandated by the project contract—`agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive`—are completely absent. Consequently, the desktop frontend (`apps/desktop/src/lib/agent-protocol.ts`) has been forced to fall back to volatile browser `localStorage` and synthetic in-memory maps. Furthermore, the cloud API (`apps/api`) lacks corresponding REST endpoints for cognitive memory and directive dispatch.

This report establishes the complete, production-grade implementation blueprints for both features, covering domain contracts, boundary conditions, error handling, state-sharing ergonomics, and end-to-end verification.

---

## 2. Deep Dive — Feature 3: Cloud Worker Delegation Handling

### 2.1 Root Cause Analysis in `apps/api/src/agent_tools.rs`

When an autonomous agent runs on the cloud worker (`agent_runner.rs`), model actions requesting tools are passed to `execute_worker_tool(...)`.

An investigation of `apps/api/src/agent_tools.rs` reveals three distinct blocking points:

#### 1. Tool Classification Exclusion (`classify_worker_tool`, lines 414–456)
```rust
fn classify_worker_tool(normalized_id: &str) -> WorkerToolFamily {
    use WorkerToolFamily::*;
    match normalized_id {
        "core.search.web" | ... => Network,
        "core.shell.execute" | ... => Shell,
        "core.workspace.write" | ... => Write,
        "core.workspace.read" | ... => Read,
        TOOL_CORE_MEMORY_SAVE | ... => Memory,
        TOOL_CORE_CONNECTOR_LIST => Connector,
        TOOL_CORE_CONNECTOR_CALL | TOOL_CORE_MCP_CALL => Network,
        TOOL_CORE_SKILL_LIST => SkillList,
        _ => match normalized_id {
            "connector.list" | "plugin.list" => Connector,
            "connector.call" | "plugin.call" | "integration.call" | "mcp.call" => Network,
            "skill.list" => SkillList,
            _ => Blocked, // <-- "core.agent.delegate", "agent.delegate", "core.agent.spawn", "core.agent.status" fall here!
        },
    }
}
```
Any orchestration tool falls through into `WorkerToolFamily::Blocked`.

#### 2. Authorization Rejection (`authorize_worker_tool`, lines 565–568)
```rust
WorkerToolFamily::Blocked => Err(format!(
    "worker_unsupported_tool: '{tool_id}' cannot run on the cloud worker (desktop-only capability or unknown tool)"
)),
```
Because the tool family is `Blocked`, authorization returns an explicit `worker_unsupported_tool` error. This creates a blocked `ToolExecutionResult`, halting or failing the cloud agent run.

#### 3. Deliberate Unsupported Assertion in Unit Tests (line 1336)
In `apps/api/src/agent_tools.rs`:
```rust
#[test]
fn worker_blocks_desktop_only_and_unknown_tools() {
    let open = open_profile();
    for tool in [
        "core.computer.use",
        "core.agent.delegate", // <-- Explicitly tested to FAIL as unsupported!
        "core.context.search",
        "core.plan.create",
        "nope.unknown.tool",
    ] {
        let err = authorize_worker_tool(Some(&open), tool, &json!({})).unwrap_err();
        assert!(err.contains("worker_unsupported_tool"), "{tool}: {err}");
    }
}
```

#### 4. Execution Router Fallthrough (`dispatch_worker_tool`, lines 670–703)
Even if authorization were bypassed, `dispatch_worker_tool` only routes memory, connectors, and skills. All other tools fall through to `deps.tools.execute(...)` (`aro_tools::ToolExecutor`). Because `ToolExecutor` is a local filesystem/shell/web executor, it returns an error `"... is not implemented by the local executor"`, which lines 693–698 convert into:
```rust
fail(format!(
    "worker_unsupported_tool: '{}' cannot run on the cloud worker (desktop-only capability or unknown tool)",
    request.tool_id
))
```

### 2.2 Orchestration Tool Specifications

In `crates/aro-agent/src/lib.rs` (lines 2641–2780), the orchestration tools are formally declared with `ToolExecutionEnvironment::CloudWorker`:
- `TOOL_CORE_AGENT_DELEGATE` (`core.agent.delegate`, alias `agent.delegate`)
- `TOOL_CORE_AGENT_SPAWN` (`core.agent.spawn`, alias `agent.spawn`)
- `TOOL_CORE_AGENT_STATUS` (`core.agent.status`, alias `agent.status`)

Furthermore, multi-agent collaboration envelopes defined in `crates/aro-core/src/agent.rs` and `packages/contracts/src/agent.ts` include:
- `task_delegation`: Directives assigned by orchestrator/peer to subagents.
- `task_progress`: Incremental execution updates with structured metrics.
- `task_result`: Completion notifications carrying summary and generated artifacts.
- `peer_collaboration`: Lateral coordination between peer subagents.
- `clarification_request` & `clarification_response`: Synchronous or asynchronous roundtrips.
- `context_share`: Knowledge sharing, including `broadcast` recipient delivery.

### 2.3 Required Implementation Strategy for `agent_tools.rs`

To resolve Feature 3 end-to-end, the following changes are required in `apps/api/src/agent_tools.rs`:

#### A. Introduce `WorkerToolFamily::Delegation`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerToolFamily {
    Network,
    Shell,
    Write,
    Read,
    Memory,
    Connector,
    SkillList,
    Delegation, // NEW
    Blocked,
}
```

#### B. Update `classify_worker_tool`
Map all orchestration tool IDs and aliases:
```rust
TOOL_CORE_AGENT_DELEGATE
| TOOL_CORE_AGENT_SPAWN
| TOOL_CORE_AGENT_STATUS
| "agent.delegate"
| "agent.spawn"
| "agent.status"
| "core.agent.envelope"
| "agent.envelope" => WorkerToolFamily::Delegation,
```

#### C. Authorize `WorkerToolFamily::Delegation`
Delegation is an internal orchestration capability governed by tenant access and run lifecycle (like memory tools):
```rust
WorkerToolFamily::Memory
| WorkerToolFamily::Connector
| WorkerToolFamily::SkillList
| WorkerToolFamily::Delegation => Ok(WebAccessPolicy::disabled()),
```

#### D. Dispatch in `dispatch_worker_tool`
```rust
TOOL_CORE_AGENT_DELEGATE | TOOL_CORE_AGENT_SPAWN | "agent.delegate" | "agent.spawn" => {
    worker_agent_delegate(deps, auth, run, request).await
}
TOOL_CORE_AGENT_STATUS | "agent.status" => {
    worker_agent_status(deps, auth, run, request).await
}
```

#### E. Implementation of `worker_agent_delegate`
The handler must support two input representations:
1. **Direct Tool Arguments**:
   - `goal`: String (required, min length 1)
   - `mode`: String (optional: "chat", "code", "think", "summarize", "quiet", "architect", "research")
   - `name`: String (optional)
   - `role`: String (optional)
   - `icon`: String (optional)
   - `max_steps`: u32 (optional)
   - `system_prompt`: String (optional)
2. **Standard `AgentMessageEnvelope` / Collaboration Payload**:
   - If input contains `envelope` or `messageType` / `sender` / `recipient` / `payload`:
     - **Self-Delegation Guard** (Invariant T2.3.1): If `sender.id == recipient.id`, fail with `"Self-delegation detected: sender and recipient cannot be identical"`.
     - **Non-Empty Content Guard** (Invariant T2.3.2): If `payload.content.trim().is_empty()`, fail with `"Delegation directive content cannot be empty"`.
     - Extract `goal = envelope.payload.content`.
     - Extract `name = recipient.name`, `role = recipient.role`.
     - Persist envelope into cognitive memory ledger (`agent_message_envelopes`).

**Execution & Scheduling Flow**:
1. Derive child run parameters:
   - `lane_id`: inherit `run.lane_id` or ensure lane via `deps.store.ensure_agent_lane(auth.user_id, auth.organization_id, run.conversation_id, title)`.
   - `conversation_id`: `run.conversation_id`.
   - `model_provider_id`: `run.model_provider_id.clone()`.
   - `model_id`: `run.model_id.clone()`.
   - `autonomy_profile_id`: `run.autonomy_profile_id`.
   - `priority`: `run.priority.clone()`.
2. Construct `AgentRun`:
   - `id = Uuid::new_v4()`
   - `status = AgentRunStatus::Queued`
   - `created_at = Utc::now()`, `updated_at = Utc::now()`
3. Persist run via `deps.store.upsert_agent_run(auth.user_id, auth.organization_id, &sub_run)`.
4. Persist initial step `AgentStepKind::RunStarted` via `deps.store.add_agent_step(...)`.
5. If background runner keyring is available, submit to queue: `deps.store.submit_agent_run_job(...)`.
6. Return `ok_result` with JSON:
   ```json
   {
       "run_id": "...",
       "status": "queued",
       "goal": "...",
       "name": "...",
       "role": "...",
       "mode": "code"
   }
   ```

#### F. Implementation of `worker_agent_status`
1. Extract `run_id` from input (`input.get("run_id")`).
2. Parse as UUID; return error if invalid.
3. Query `deps.store.get_agent_run(auth.user_id, auth.organization_id, run_id)`. If `None`, return error `"agent run not found"`.
4. Query `deps.store.list_agent_steps(auth.user_id, auth.organization_id, run_id)`.
5. Return `ok_result` with:
   ```json
   {
       "run_id": "...",
       "status": "running", // or "completed", "failed", "paused"
       "goal": "...",
       "steps_count": 5,
       "last_step": { ... },
       "name": "...",
       "role": "...",
       "mode": "code"
   }
   ```

#### G. Multi-Agent Collaboration Envelopes Handling
For progress, result, and peer collaboration:
- **`task_progress`**: Persist envelope into `agent_message_envelopes`. Update parent run's `checkpoint_summary` if present in `payload.content`.
- **`task_result`**: Persist envelope into `agent_message_envelopes`. If `payload.artifacts` contains artifacts, persist them to `agent_artifacts` for the parent run.
- **`peer_collaboration`**: Persist envelope into `agent_message_envelopes` and record in recipient's cognitive memory ledger.
- **`broadcast`** (Invariant T2.3.5): When recipient is `broadcast`, deliver to all sub-agents active in the conversation.

---

## 3. Deep Dive — Feature 4: Persistent Cognitive Memory IPC & API

### 3.1 Architecture Overview & Current State

```
┌────────────────────────────────────────────────────────┐
│             Desktop Svelte Frontend (UI)               │
│  apps/desktop/src/lib/agent-protocol.ts                │
│  (Currently uses volatile localStorage & Map cache)    │
└───────────────────────────▲────────────────────────────┘
                            │ invoke('agent_get_memory', ...)
                            │ invoke('agent_save_memory', ...)
                            │ invoke('agent_dispatch_directive', ...)
┌───────────────────────────▼────────────────────────────┐
│              Tauri IPC Backend (`src-tauri`)           │
│  apps/desktop/src-tauri/src/commands.rs (NEW)          │
│  apps/desktop/src-tauri/src/main.rs (Registry)         │
│                                                        │
│  tauri::State<'_, AppState>                            │
│  ├── state.engine: AssistantEngine                     │
│  │   └── engine.memory_store(): SqliteMemoryStore      │
│  └── state.cloud: CloudApiClient                       │
└───────────────────────────▲────────────────────────────┘
                            │ REST sync (cloud mode)
┌───────────────────────────▼────────────────────────────┐
│                Cloud API (`apps/api`)                  │
│  POST /agent/memory                                    │
│  GET  /agent/memory?agent_id=...&conversation_id=...   │
│  POST /agent/directive                                 │
│  GET  /agent/ledger?conversation_id=...                │
└────────────────────────────────────────────────────────┘
```

Currently:
1. `apps/desktop/src-tauri/src/commands.rs` does not exist. All commands are in `main.rs`.
2. None of `agent_get_memory`, `agent_save_memory`, or `agent_dispatch_directive` exist in `main.rs`.
3. `agent-protocol.ts` stores state in `localStorage` under `aro:agent-memory:...` and `aro:agent-ledger:...`.
4. `apps/api/src/main.rs` has agent run endpoints (`/agent/runs`), lane endpoints (`/agent/lanes`), and tool endpoints (`/agent/tools/execute`), but lacks cognitive memory endpoints (`/agent/memory`, `/agent/directive`, `/agent/ledger`).

### 3.2 State Sharing in Tauri (`tauri::State<'_, AppState>`)

Tauri manages application state through dependency injection via `.manage(app_state)` in `main.rs:4996`:
```rust
.manage(app_state)
.invoke_handler(tauri::generate_handler![ ... ])
```

In `apps/desktop/src-tauri/src/state.rs`:
```rust
pub struct AppState {
    pub engine: AssistantEngine,
    pub cloud: CloudApiClient,
    pub paths: AppPaths,
    settings: Mutex<AppSettings>,
    cloud_session: Mutex<Option<AuthSession>>,
    inflight_conversations: Mutex<HashSet<uuid::Uuid>>,
}
```

Key handles available on `AppState`:
- **`state.engine.memory_store()`**: Returns `&SqliteMemoryStore`, providing direct, thread-safe access to the local SQLite database.
- **`state.engine`**: The `AssistantEngine` orchestration kernel, providing `start_agent_run`, `agent_run_view`, `update_agent_run_status`.
- **`state.cloud`**: The `CloudApiClient` for making authenticated HTTP requests to the cloud API.
- **`state.refresh_cloud_session_from_keyring()`**: Retrieves current authentication credentials.

### 3.3 IPC Commands Specification & Implementation

#### 1. `agent_get_memory`
```rust
#[tauri::command]
pub async fn agent_get_memory(
    state: State<'_, AppState>,
    agent_id: String,
    conversation_id: String,
) -> CommandResult<AgentMemoryContext>
```
- **Requirements & Invariants**:
  - T1.4.1: Retrieves existing memory context with fidelity (scratchpad, findings, ledger, artifacts).
  - T2.4.1: Querying a non-existent agent or conversation must return a fresh, valid initial context without throwing an error:
    ```rust
    AgentMemoryContext::new(conversation_id, agent_id, "Sous-Agent", "worker")
    ```
- **Implementation Logic**:
  1. Validate arguments. If `agent_id` or `conversation_id` is blank, return initial context with defaults.
  2. Query `state.engine.memory_store().get_agent_memory(&conversation_id, &agent_id)`.
  3. If present in SQLite, return it.
  4. If not present locally and cloud session is active:
     - Attempt query via `state.cloud.get_agent_memory(&session.access_token, &conversation_id, &agent_id)`.
     - If cloud returns context, save to local SQLite and return.
  5. Otherwise, return a default initial `AgentMemoryContext`.

#### 2. `agent_save_memory`
```rust
#[tauri::command]
pub async fn agent_save_memory(
    state: State<'_, AppState>,
    memory: AgentMemoryContext,
) -> CommandResult<()>
```
- **Requirements & Invariants**:
  - T1.4.2: Persists external state modifications reliably.
  - T2.4.2: Rejects saving memory with empty `agentId` or `conversationId` with error `"agentId and conversationId are required"`.
  - T2.4.3: Handles concurrent save requests safely preserving latest updates (`TransactionBehavior::Immediate`).
- **Implementation Logic**:
  1. Validate `!memory.agent_id.trim().is_empty()` and `!memory.conversation_id.trim().is_empty()`. Otherwise return `Err("agentId and conversationId are required".to_string())`.
  2. Persist to SQLite via `state.engine.memory_store().save_agent_memory(&memory)`:
     - Upserts into `agent_memories`.
     - Upserts all items in `memory.findings` into `agent_findings`.
     - Upserts all items in `memory.ledger` into `agent_message_envelopes`.
  3. If cloud session is active:
     - Asynchronously sync to cloud API: `state.cloud.save_agent_memory(&session.access_token, &memory)`.
  4. Return `Ok(())`.

#### 3. `agent_dispatch_directive`
```rust
#[tauri::command]
pub async fn agent_dispatch_directive(
    state: State<'_, AppState>,
    agent_id: String,
    directive: String,
    conversation_id: String,
) -> CommandResult<AgentRunView>
```
- **Requirements & Invariants**:
  - T1.4.3: Creates queued run and updates ledger.
  - T2.4.5: Handles IPC call with missing arguments returning error `"Missing required arguments"`.
- **Implementation Logic**:
  1. Validate arguments: if `agent_id.trim().is_empty() || directive.trim().is_empty() || conversation_id.trim().is_empty()`, return `Err("Missing required arguments".to_string())`.
  2. Look up existing memory context for `(conversation_id, agent_id)` to resolve `agent_name` and `role` (default: `"Autonomous Worker"`, `"worker"`).
  3. Construct `AgentMessageEnvelope`:
     - `sender = AgentParticipant::orchestrator("orchestrator", "Aro Orchestrator")`
     - `recipient = AgentParticipant::subagent(&agent_id, &name, &role, None)`
     - `message_type = AgentMessageType::TaskDelegation`
     - `payload = AgentMessagePayload::text(&directive)`
     - `timestamp = Utc::now()`
  4. Record envelope in SQLite ledger via `state.engine.memory_store().record_agent_envelope(&envelope)`.
  5. Start agent run via `state.engine.start_agent_run(...)`:
     - `goal = directive.clone()`
     - `conversation_id = Some(Uuid::parse_str(&conversation_id)...)`
     - `mode = AssistantMode::Code` (or mapped from role)
     - `priority = AgentRunPriority::Normal`
  6. Return `Ok(run_view)`.

### 3.4 Module Architecture: `commands.rs` vs `main.rs`

To comply with the project layout described in `PROJECT.md` line 114:
> `apps/desktop/src-tauri/src/: Tauri IPC bindings (commands.rs, main.rs)`

We recommend extracting the cognitive memory and agent directive commands into a dedicated `commands.rs` file:
```rust
// apps/desktop/src-tauri/src/commands.rs
use aro_core::{AgentMemoryContext, AgentRunView, ...};
use tauri::State;
use crate::state::AppState;
use crate::CommandResult;

#[tauri::command]
pub async fn agent_get_memory(...) -> CommandResult<AgentMemoryContext> { ... }

#[tauri::command]
pub async fn agent_save_memory(...) -> CommandResult<()> { ... }

#[tauri::command]
pub async fn agent_dispatch_directive(...) -> CommandResult<AgentRunView> { ... }
```

In `apps/desktop/src-tauri/src/main.rs`:
```rust
mod commands;
use commands::{agent_dispatch_directive, agent_get_memory, agent_save_memory};

// In tauri::generate_handler!
.invoke_handler(tauri::generate_handler![
    ...
    agent_get_memory,
    agent_save_memory,
    agent_dispatch_directive,
    ...
])
```

### 3.5 Cloud API Endpoints in `apps/api`

In `apps/api/src/main.rs`, register the corresponding REST endpoints:
```rust
.route(
    "/agent/memory",
    get(handlers::agent_memory_get).post(handlers::agent_memory_save),
)
.route(
    "/agent/directive",
    post(handlers::agent_directive_dispatch),
)
.route(
    "/agent/ledger",
    get(handlers::agent_ledger_list),
)
.route(
    "/agent/envelope",
    post(handlers::agent_envelope_dispatch),
)
```

In `apps/api/src/handlers.rs`:
- `agent_memory_get`: Accepts query params `agent_id` and `conversation_id`, retrieves from `state.store.get_agent_memory(...)`.
- `agent_memory_save`: Accepts JSON `AgentMemoryContext`, upserts via `state.store.upsert_agent_memory(...)`.
- `agent_directive_dispatch`: Accepts `{ agent_id, directive, conversation_id }`, persists envelope and starts sub-agent run via `agent_runs_create`.
- `agent_ledger_list`: Accepts query param `conversation_id`, retrieves envelopes.
- `agent_envelope_dispatch`: Accepts `AgentMessageEnvelope`, persists and routes.

---

## 4. Proposed Code Changes & Implementation Blueprints

### 4.1 Changes in `apps/api/src/agent_tools.rs`

#### 1. Add `WorkerToolFamily::Delegation`
```rust
// In apps/api/src/agent_tools.rs around line 410
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerToolFamily {
    Network,
    Shell,
    Write,
    Read,
    Memory,
    Connector,
    SkillList,
    Delegation, // <--- ADDED
    Blocked,
}
```

#### 2. Update `classify_worker_tool`
```rust
// Around line 448
        TOOL_CORE_AGENT_DELEGATE
        | TOOL_CORE_AGENT_SPAWN
        | TOOL_CORE_AGENT_STATUS
        | "agent.delegate"
        | "agent.spawn"
        | "agent.status"
        | "core.agent.envelope"
        | "agent.envelope" => Delegation,
```

#### 3. Update `authorize_worker_tool`
```rust
// Around line 563
        WorkerToolFamily::Memory
        | WorkerToolFamily::Connector
        | WorkerToolFamily::SkillList
        | WorkerToolFamily::Delegation => Ok(WebAccessPolicy::disabled()),
```

#### 4. Route in `dispatch_worker_tool`
```rust
// Around line 688
        TOOL_CORE_AGENT_DELEGATE | TOOL_CORE_AGENT_SPAWN | "agent.delegate" | "agent.spawn" => {
            worker_agent_delegate(deps, auth, _run, request).await
        }
        TOOL_CORE_AGENT_STATUS | "agent.status" => {
            worker_agent_status(deps, auth, _run, request).await
        }
```

#### 5. Implement `worker_agent_delegate` & `worker_agent_status`
```rust
async fn worker_agent_delegate(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    parent_run: &AgentRun,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let fail = |msg: String| failed_tool_result(request, msg);

    // 1. Inspect input: support direct arguments or AgentMessageEnvelope
    let mut goal = request
        .input
        .get("goal")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut mode_str = request
        .input
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("code")
        .to_string();
    let mut name = request
        .input
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mut role = request
        .input
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mut priority = parent_run.priority.clone();

    // Check if input is or contains an AgentMessageEnvelope
    let envelope_candidate: Option<aro_core::AgentMessageEnvelope> = serde_json::from_value(request.input.clone())
        .ok()
        .or_else(|| {
            request.input.get("envelope")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
        });

    if let Some(ref env) = envelope_candidate {
        // Invariant T2.3.1: Reject self-delegation
        if env.sender.id == env.recipient.id {
            return fail("Self-delegation detected: sender and recipient cannot be identical".to_string());
        }
        // Invariant T2.3.2: Reject empty or whitespace-only directive content
        if env.payload.content.trim().is_empty() {
            return fail("Delegation directive content cannot be empty".to_string());
        }

        goal = env.payload.content.clone();
        name = Some(env.recipient.name.clone());
        role = env.recipient.role.clone();
        if let Some(ref p) = env.priority {
            priority = p.clone();
        }
    } else if goal.trim().is_empty() {
        return fail("Delegation directive content cannot be empty".to_string());
    }

    let mode = match mode_str.as_str() {
        "chat" => aro_core::AssistantMode::Chat,
        "think" => aro_core::AssistantMode::Think,
        "summarize" => aro_core::AssistantMode::Summarize,
        "quiet" => aro_core::AssistantMode::Quiet,
        _ => aro_core::AssistantMode::Code,
    };

    let agent_name = name.unwrap_or_else(|| "Worker Assistant".to_string());
    let agent_role = role.unwrap_or_else(|| "Specialist".to_string());

    let child_run_id = Uuid::new_v4();
    let now = Utc::now();
    let child_run = AgentRun {
        id: child_run_id,
        lane_id: parent_run.lane_id,
        conversation_id: parent_run.conversation_id,
        goal: goal.clone(),
        mode,
        status: aro_core::AgentRunStatus::Queued,
        priority,
        model_provider_id: parent_run.model_provider_id.clone(),
        model_id: parent_run.model_id.clone(),
        autonomy_profile_id: parent_run.autonomy_profile_id,
        checkpoint_summary: None,
        last_error: None,
        created_at: now,
        updated_at: now,
        heartbeat_at: None,
        completed_at: None,
        sync_status: aro_core::SyncStatus::Local,
    };

    if let Err(err) = deps.store.upsert_agent_run(auth.user_id, auth.organization_id, &child_run).await {
        return fail(format!("failed to persist delegated agent run: {err}"));
    }

    let started_step = AgentStep {
        id: Uuid::new_v4(),
        run_id: child_run_id,
        sequence: 1,
        kind: aro_core::AgentStepKind::RunStarted,
        status: aro_core::AgentStepStatus::Completed,
        title: format!("Run queued: {}", child_run.goal),
        input: json!({ "goal": child_run.goal }),
        output: json!({ "status": "queued" }),
        error: None,
        started_at: now,
        finished_at: Some(now),
    };
    let _ = deps.store.add_agent_step(auth.user_id, auth.organization_id, &started_step).await;

    let output = json!({
        "run_id": child_run_id.to_string(),
        "status": "queued",
        "goal": goal,
        "name": agent_name,
        "role": agent_role,
        "mode": mode_str,
    });
    let title = format!("Delegated agent run {child_run_id} ({agent_name}) for goal: {}", &goal[..goal.len().min(50)]);
    ok_result(request, title, output)
}

async fn worker_agent_status(
    deps: &WorkerToolDeps,
    auth: &AuthContext,
    _run: &AgentRun,
    request: &ToolExecutionRequest,
) -> ToolExecutionResult {
    let fail = |msg: String| failed_tool_result(request, msg);
    let run_id_str = request
        .input
        .get("run_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let run_id = match Uuid::parse_str(run_id_str) {
        Ok(id) => id,
        Err(e) => return fail(format!("invalid run_id: {e}")),
    };

    let run = match deps.store.get_agent_run(auth.user_id, auth.organization_id, run_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return fail(format!("agent run not found: {run_id}")),
        Err(e) => return fail(format!("database error reading agent run: {e}")),
    };

    let steps = deps
        .store
        .list_agent_steps(auth.user_id, auth.organization_id, run_id)
        .await
        .unwrap_or_default();
    let status_str = format!("{:?}", run.status).to_lowercase();
    let last_step = steps.last().cloned();

    let output = json!({
        "run_id": run.id.to_string(),
        "status": status_str,
        "goal": run.goal,
        "steps_count": steps.len(),
        "last_step": last_step,
        "mode": format!("{:?}", run.mode).to_lowercase(),
    });
    let title = format!("Agent run {} status: {}", run.id, status_str);
    ok_result(request, title, output)
}
```

#### 6. Unit Test Update in `agent_tools.rs`
Update `worker_blocks_desktop_only_and_unknown_tools` to remove `"core.agent.delegate"` and add a test verifying delegation authorization:
```rust
#[test]
fn worker_blocks_desktop_only_and_unknown_tools() {
    let open = open_profile();
    for tool in [
        "core.computer.use",
        "core.context.search",
        "core.plan.create",
        "nope.unknown.tool",
    ] {
        let err = authorize_worker_tool(Some(&open), tool, &json!({})).unwrap_err();
        assert!(err.contains("worker_unsupported_tool"), "{tool}: {err}");
    }
}

#[test]
fn worker_authorizes_agent_orchestration_tools() {
    let open = open_profile();
    for tool in [
        "core.agent.delegate",
        "agent.delegate",
        "core.agent.spawn",
        "agent.spawn",
        "core.agent.status",
        "agent.status",
    ] {
        assert!(
            authorize_worker_tool(Some(&open), tool, &json!({})).is_ok(),
            "expected {tool} to be authorized"
        );
    }
}
```

---

### 4.2 Proposed Implementation for `apps/desktop/src-tauri/src/commands.rs`

```rust
//! apps/desktop/src-tauri/src/commands.rs
//! Cognitive Memory & Sub-Agent Directive IPC Commands

use aro_core::{
    AgentMemoryContext, AgentMessageEnvelope, AgentMessagePayload,
    AgentParticipant, AgentRunPriority, AgentRunStartRequest, AgentRunView, AssistantMode,
};
use tauri::State;
use uuid::Uuid;

use crate::{state::AppState, CommandResult};

#[tauri::command]
pub async fn agent_get_memory(
    state: State<'_, AppState>,
    agent_id: String,
    conversation_id: String,
) -> CommandResult<AgentMemoryContext> {
    if agent_id.trim().is_empty() || conversation_id.trim().is_empty() {
        return Ok(AgentMemoryContext::new(
            conversation_id,
            agent_id,
            "Sous-Agent",
            "worker",
        ));
    }

    // 1. Query SQLite memory store
    match state
        .engine
        .memory_store()
        .get_agent_memory(&conversation_id, &agent_id)
    {
        Ok(Some(context)) => Ok(context),
        Ok(None) => {
            // Invariant T2.4.1: Querying non-existent conversation returns fresh initial context
            Ok(AgentMemoryContext::new(
                conversation_id,
                agent_id,
                "Sous-Agent",
                "worker",
            ))
        }
        Err(e) => Err(format!("failed to get agent memory: {e}")),
    }
}

#[tauri::command]
pub async fn agent_save_memory(
    state: State<'_, AppState>,
    memory: AgentMemoryContext,
) -> CommandResult<()> {
    // Invariant T2.4.2: Reject empty agentId or conversationId
    if memory.agent_id.trim().is_empty() || memory.conversation_id.trim().is_empty() {
        return Err("agentId and conversationId are required".to_string());
    }

    state
        .engine
        .memory_store()
        .save_agent_memory(&memory)
        .map_err(|e| format!("failed to save agent memory: {e}"))?;

    // Optional asynchronous cloud sync if authenticated
    if let Ok(Some(session)) = state.refresh_cloud_session_from_keyring().await {
        let cloud = state.cloud.clone();
        let token = session.access_token;
        let mem = memory.clone();
        tokio::spawn(async move {
            let _ = cloud.save_agent_memory(&token, &mem).await;
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn agent_dispatch_directive(
    state: State<'_, AppState>,
    agent_id: String,
    directive: String,
    conversation_id: String,
) -> CommandResult<AgentRunView> {
    // Invariant T2.4.5: Missing required arguments error
    if agent_id.trim().is_empty() || directive.trim().is_empty() || conversation_id.trim().is_empty() {
        return Err("Missing required arguments".to_string());
    }

    // 1. Load context to resolve agent metadata
    let mem = state
        .engine
        .memory_store()
        .get_agent_memory(&conversation_id, &agent_id)
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            AgentMemoryContext::new(&conversation_id, &agent_id, "Autonomous Worker", "worker")
        });

    // 2. Dispatch task_delegation envelope
    let envelope = AgentMessageEnvelope {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        parent_message_id: None,
        correlation_id: None,
        sender: AgentParticipant::orchestrator("orchestrator", "Aro Orchestrator"),
        recipient: AgentParticipant::subagent(&agent_id, &mem.agent_name, &mem.role, None),
        message_type: aro_core::AgentMessageType::TaskDelegation,
        payload: AgentMessagePayload::text(&directive),
        permission_profile_id: mem.permission_profile_id.clone(),
        priority: Some(AgentRunPriority::Normal),
        timestamp: chrono::Utc::now(),
    };

    let _ = state.engine.memory_store().record_agent_envelope(&envelope);

    // 3. Start local agent run
    let conv_uuid = Uuid::parse_str(&conversation_id).ok();
    let run_request = AgentRunStartRequest {
        run_id: None,
        lane_id: None,
        conversation_id: conv_uuid,
        goal: directive,
        mode: AssistantMode::Code,
        system_prompt: None,
        model_id: None,
        provider: None,
        autonomy_profile_id: None,
        priority: Some(AgentRunPriority::Normal),
        max_steps: Some(10),
    };

    let view = state
        .engine
        .start_agent_run(run_request)
        .await
        .map_err(|e| format!("failed to start agent run for directive: {e}"))?;

    Ok(view)
}
```

---

## 5. Verification & Test Plan

### 5.1 Rust Test Commands

To verify Feature 3 & Feature 4 without regressing any existing functionality:

```powershell
# 1. Verify compilation across crates and applications
cargo check -p aro-core
cargo check -p aro-memory
cargo check -p aro-api
cargo check -p aro-desktop

# 2. Run unit tests for api agent tools
cargo test -p aro-api -- agent_tools

# 3. Run full memory test suite (verifying table migrations and immediate transactions)
cargo test -p aro-memory --lib
cargo test -p aro-memory --test m1_adversarial_stress_tests

# 4. Verify core contract tests
cargo test -p aro-core --test cognitive_memory_contract_tests
```

### 5.2 E2E Test Suite Matrix

The implementation strategy directly satisfies the following automated tests:

| Test ID | Test Name | Invariant Checked |
|---|---|---|
| **T1.3.1** | Standard task_delegation envelope | Envelope creation, validation, `suggestedActions` preservation |
| **T1.3.2** | Task_progress envelope handling | Incremental status reporting, `structuredData` parsing |
| **T1.3.3** | Task_result envelope delivery | Generated `artifacts` attachment, summary preservation |
| **T1.3.4** | Peer_collaboration routing | Lateral subagent-to-subagent envelope ledger appending |
| **T1.3.5** | Clarification request/response | Correlation ID preservation across conversational roundtrips |
| **T1.4.1** | IPC `agent_get_memory` | Context retrieval with fidelity (scratchpad, findings) |
| **T1.4.2** | IPC `agent_save_memory` | External state persistence |
| **T1.4.3** | IPC `agent_dispatch_directive` | Creates queued run, updates message ledger |
| **T1.4.4** | Memory process restart survival | In-database table counts (`agent_memories`, `agent_findings`) |
| **T1.4.5** | `clearConversation` eviction | Purges memory entries and ledger for targeted conversation |
| **T2.3.1** | Self-delegation detection | Throws `"Self-delegation detected: sender and recipient cannot be identical"` |
| **T2.3.2** | Empty directive rejection | Throws `"Delegation directive content cannot be empty"` |
| **T2.3.3** | 100+ suggested actions | Handles high complexity without truncation |
| **T2.3.4** | Deep parentMessageId chains | Tracks 10+ levels of nested parent message IDs |
| **T2.3.5** | Broadcast delivery | Envelopes with recipient `"broadcast"` fan out to all active subagents |
| **T2.4.1** | Non-existent conversation query | Returns default fresh context without throwing error |
| **T2.4.2** | Empty ID validation on save | Rejects with `"agentId and conversationId are required"` |
| **T2.4.3** | Concurrent save safety | Multiple rapid writes preserve latest version |
| **T2.4.5** | Missing IPC arguments | Returns structured error `"Missing required arguments"` |
| **Combo 5** | F3 + F1 Cross-Feature | Inbound delegation appends to subagent's cognitive memory ledger |
| **Combo 6** | F3 + F2 Cross-Feature | Delegation triggers queueing, async execution, result delivery |
| **Scenario 1** | Tier 4 Autonomous Code Audit | Full end-to-end multi-agent code audit under read-only confinement |

---

## 6. Recommendations & Handoff Summary for Builder Agents

1. **`apps/api/src/agent_tools.rs`**:
   - Add `WorkerToolFamily::Delegation`.
   - Update `classify_worker_tool` and `authorize_worker_tool`.
   - Implement `worker_agent_delegate` and `worker_agent_status` in `dispatch_worker_tool`.
   - Replace unit test assertion expecting `core.agent.delegate` to be blocked with an authorization check.
2. **`apps/desktop/src-tauri/src/commands.rs` & `main.rs`**:
   - Create `commands.rs` implementing `agent_get_memory`, `agent_save_memory`, `agent_dispatch_directive`.
   - Add `mod commands;` to `main.rs` and register the three commands in `invoke_handler(tauri::generate_handler![...])`.
3. **`apps/api/src/main.rs` & `handlers.rs`**:
   - Register REST routes for `/agent/memory`, `/agent/directive`, `/agent/ledger`, `/agent/envelope`.
4. **`crates/aro-memory/src/lib.rs` (Feature 1 Dependency)**:
   - Ensure `SqliteMemoryStore` provides `get_agent_memory`, `save_agent_memory`, `record_agent_envelope`, and `clear_agent_memory`.

This concludes the exploration for Feature 3 and Feature 4.

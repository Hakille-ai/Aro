# ARO Architecture Survey — Rust Backend & Autonomous Multi-Agent Orchestration (Requirement R1)

**Document Reference**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_1\survey_rust_orchestration.md`  
**Author**: Explorer Survey 1 (`teamwork_preview_explorer`)  
**Date**: 2026-09-24  
**Integrity Mode**: Development  
**Authoritative Goal**: Requirement R1 — Multi-Agent Collaboration & Autonomous Orchestration  

---

## 1. Executive Summary

ARO's codebase contains an extraordinarily rich set of primitives:
- A modular Rust workspace with 16 specialized crates (`aro-agent`, `aro-agent-domain`, `aro-core`, `aro-files`, `aro-memory`, `aro-policy`, `aro-integrations`, `aro-secrets`, `aro-store`, `aro-tools`, `aro-mcp`, `aro-skills`, `aro-plugins`, `aro-runtime`, `aro-vector`, `aro-voice`);
- A durable PostgreSQL job queue (`agent_run_jobs`, leases, snapshot encryption, exponential backoff, worker daemon) in `aro-store` and `apps/api`;
- A local SQLite database (`agent_runs`, `agent_lanes`, `agent_steps`, `agent_artifacts`, `agent_context_items`, `agent_permission_profiles`) in `aro-memory`;
- An event-sourced domain aggregate machine with pure reducers in `aro-agent-domain`;
- An Apple/Google-grade desktop UI in `apps/desktop` with micro-pills, breadcrumbs, unified message streaming, and agent inspection modals;
- A TypeScript contract package (`@aro/contracts`) defining envelopes, scratchpads, findings (constats), and permission directives.

### The Core Architectural Disconnect in R1
Despite these powerful pieces, **multi-agent collaboration and autonomous orchestration are currently split by a fundamental seam**:
1. **Cognitive Working Data is Trapped in Browser `localStorage`**:
   The TypeScript contracts (`@aro/contracts/src/agent.ts:353-372`) define `AgentMemoryFinding`, `AgentMemoryContext`, and `AgentMessageEnvelope`. However, in the application (`apps/desktop/src/lib/agent-protocol.ts:16-160`), all scratchpad updates, findings/constats, and inter-agent messages are stored in `localStorage` under keys `aro:agent-memory:...` and `aro:agent-ledger:...`. There are **no corresponding Rust structs, no SQLite tables, and no PostgreSQL tables** for these entities.
2. **Sub-Agent Execution on Desktop is Unstarted**:
   When an agent invokes `core.agent.delegate` or `core.agent.spawn` on desktop (`crates/aro-runtime/src/lib.rs:2435-2540`), the runtime calls `start_agent_run()`, which persists the run, step 1 (`RunStarted`), and step 2 (`ContextBuilt`) in SQLite (`crates/aro-runtime/src/lib.rs:585-598`). But **no background task (`tokio::spawn`) is ever launched to execute subsequent model steps or tools**. The sub-agent remains permanently frozen at step 2.
3. **Sub-Agent Directives in UI are Mocked**:
   When a user clicks a sub-agent pill and types a directive in the composer (`apps/desktop/src/App.svelte:8303-8365`), the UI intercepts the submission, creates a fake assistant message (*"Directive bien reçue par..."* / *"Directive received by..."*), updates the scratchpad in `localStorage`, and halts. No LLM or tool execution ever takes place for the sub-agent.
4. **Cloud Worker Fails Closed on Delegation**:
   In the cloud API worker (`apps/api/src/agent_tools.rs:681-703`), `dispatch_worker_tool` does not intercept `core.agent.delegate` or `core.agent.spawn`. Delegation falls through to `local_executor` and returns `worker_unsupported_tool`.
5. **Session Loss on Conversation Switching & Restart**:
   The sub-agent chat thread (`subAgentMessagesMap` in `App.svelte:921`) is stored in volatile component memory. On conversation switch or restart, the thread is lost and only reconstructed through ad-hoc fallback strings.

Closing this gap to fulfill Requirement R1 requires establishing first-class Rust types, durable SQLite/PostgreSQL schemas, true background execution for sub-agents, inter-agent messaging IPC/API channels, and bidirectional frontend persistence.

---

## 2. Rust Workspace Architecture & Crate Topology

The Cargo workspace (`Cargo.toml:1-22`) manages 18 members (16 crates and 2 applications). Below is the precise map of crates involved in agent orchestration, memory, and persistence:

```
                                  ┌────────────────────────┐
                                  │  apps/desktop/src-tauri│ (Tauri IPC, State, Local Bridge)
                                  └───────────┬────────────┘
                                              │
                      ┌───────────────────────┼───────────────────────┐
                      ▼                       ▼                       ▼
            ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
            │   aro-runtime    │    │    aro-memory    │    │    aro-tools     │
            │ (AssistantEngine)│    │(SqliteMemoryStore│    │ (ToolExecutor)   │
            └─────────┬────────┘    └─────────┬────────┘    └─────────┬────────┘
                      │                       │                       │
                      └───────────────────────┼───────────────────────┘
                                              ▼
                                    ┌──────────────────┐
                                    │    aro-agent     │ (Context, Orchestrator, Descriptors)
                                    └─────────┬────────┘
                                              ▼
                                    ┌──────────────────┐
                                    │     aro-core     │ (Shared Structs, Enums, Constants)
                                    └─────────┬────────┘
                                              ▼
                                    ┌──────────────────┐
                                    │ aro-agent-domain │ (Pure Reducer State Machine)
                                    └──────────────────┘
```

### Detailed Crate Survey

#### 1. `crates/aro-core`
- **Location**: `crates/aro-core/src/`
- **Role**: Foundational types shared across all crates without heavy dependencies.
- **Key Files & Types**:
  - `agent.rs`:
    - `AgentRun` (lines 220-238): Stores `id`, `lane_id`, `conversation_id`, `goal`, `mode`, `status`, `priority`, `model_provider_id`, `model_id`, `autonomy_profile_id`, `checkpoint_summary`, `last_error`, `created_at`, `updated_at`, `heartbeat_at`, `completed_at`.
    - `AgentLane` (lines 272-281): Orchestration lane with concurrency limit `max_concurrent_runs`.
    - `AgentStep` (lines 320-333): Step sequence, kind (`AgentStepKind`), status (`AgentStepStatus`), `input`, `output`, `error`.
    - `AgentArtifact` (lines 410-420): Generated artifact (`id`, `run_id`, `kind`, `title`, `uri`, `content`, `metadata`).
    - `AgentContextItem` (lines 423-434): Retrieved context item for prompt injection.
    - `PermissionProfile` (lines 153-167): Autonomy boundaries (`trusted_roots`, `allowed_domains`, `allow_read`, `allow_write`, `allow_shell`, `allow_network`, `command_approval`, `redact_secrets`).
  - `memory.rs`:
    - `WorkingMemoryBuffer` (lines 389-446): Tier 1 working memory containing `conversation_id`, `max_turns`, `max_tokens`, `messages`, `session_variables: HashMap<String, String>`, and `scratchpad: Option<String>`.
    - `LongTermMemory` (lines 23-46): Long-term memory entries with salience scoring, decay, and recall tracking.
  - `tool.rs`:
    - Dotted tool constants (lines 10-54, 171-205):
      - `TOOL_CORE_AGENT_DELEGATE = "core.agent.delegate"` (line 196)
      - `TOOL_CORE_AGENT_SPAWN = "core.agent.spawn"` (line 197)
      - `TOOL_CORE_AGENT_STATUS = "core.agent.status"` (line 198)

#### 2. `crates/aro-agent`
- **Location**: `crates/aro-agent/src/`
- **Role**: Agent context building, tool descriptor registry, checkpointing, and execution scheduling.
- **Key Components**:
  - `AgentRuntime` (`lib.rs:40-61`): Aggregates `ContextBuilder`, `ToolRegistry`, `CheckpointService`, `RunOrchestrator`.
  - `ToolDescriptor` for delegation (`lib.rs:2641-2692`): `agent_delegate_descriptor()` registers `core.agent.delegate` with input schema `{ goal, mode, max_steps, system_prompt }`.
  - `ToolDescriptor` for spawning (`lib.rs:2694-2743`): `agent_spawn_descriptor()` registers `core.agent.spawn`.
  - `ContextBuilder` (`src/context.rs:1-120`): Builds structured `ContextPack` combining goals, instructions, past steps, memory sources, and active tools.

#### 3. `crates/aro-agent-domain`
- **Location**: `crates/aro-agent-domain/src/`
- **Role**: Provider-neutral, pure state machines and event-sourced aggregates for durable execution.
- **Status**: Pure state machine crate (lines 1-53 of `README.md`). Currently not yet integrated into the live user execution paths.
- **Key Models**:
  - `RunAggregate` (`aggregate.rs:35-58`): Tracks run lifecycle with `AggregateMetadata` and `AggregateVersion`.
  - `TaskAggregate` (`aggregate.rs:62-91`): Sub-tasks with parent task linkage (`parent_task_id`), compensation result tracking.
  - `StepAggregate` & `StepAttemptAggregate` (`aggregate.rs:95-150`): Fine-grained step attempts, effect preparation (`prepare_effect`), effect committing (`commit_effect`), and reconciliation.
  - `ApprovalAggregate` (`aggregate.rs:152-180`): Single-use cryptographic approval gates (`action_digest`).
  - `EnvironmentAggregate` (`aggregate.rs:182-212`): Sandbox lifecycle and cleanup receipts.
  - `RunCommand`, `TaskCommand`, `StepCommand` (`command.rs:46-178`): Strongly typed domain commands.
  - Pure Reducer (`reducer.rs:1-800+`): Deterministic state transitions producing events atomically.

#### 4. `crates/aro-memory` (Local SQLite Persistence)
- **Location**: `crates/aro-memory/src/lib.rs`
- **Role**: Embedded SQLite database (`SqliteMemoryStore`) powering local single-user desktop mode.
- **Existing Schema Tables** (`lib.rs:100-220`):
  1. `conversations` (id, title, mode, project_id, folder_id, root_path, created_at, updated_at, deleted_at)
  2. `messages` (id, conversation_id, role, content, created_at, token_estimate, model_id, agent_run_id)
  3. `memories` & `memories_fts` (semantic and episodic long-term memory)
  4. `agent_runs` (id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id, autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at, heartbeat_at, completed_at, sync_status)
  5. `agent_lanes` (id, conversation_id, title, status, priority, max_concurrent_runs, created_at, updated_at, sync_status)
  6. `agent_steps` (id, run_id, sequence, kind, status, title, input_json, output_json, error, started_at, finished_at)
  7. `agent_artifacts` (id, run_id, kind, title, uri, content, metadata_json, created_at)
  8. `agent_context_items` & `agent_context_fts` (searchable contextual snippets)
  9. `agent_permission_profiles` (id, name, profile_json, created_at, updated_at)
  10. `plans` (id, conversation_id, title, description, tasks, status, created_at, updated_at, deleted_at)
- **Observations on Gaps**:
  - No `agent_memories` table for agent-specific cognitive scratchpads.
  - No `agent_findings` table for constats/insights discovered by agents.
  - No `agent_message_envelopes` table for the inter-agent collaboration ledger.

#### 5. `crates/aro-store` (Server PostgreSQL Persistence)
- **Location**: `crates/aro-store/src/` & `crates/aro-store/migrations/`
- **Role**: Multi-tenant PostgreSQL store with row-level security (`AroStore`).
- **Key Migrations**:
  - `202607010008_agent_runtime.sql`: Defines `agent_runs`, `agent_lanes`, `agent_steps`, `agent_artifacts`, `agent_context_items`, `agent_permission_profiles`.
  - `202607020012_agent_execution_queue.sql`: Implements durable `agent_run_jobs` queue, lease tokens, generational leases, encrypted snapshot payloads (`request_snapshot_encrypted`), budget enforcement (`max_steps`, `max_tool_calls`, `max_wall_time_seconds`), and append-only `agent_run_events`.
  - `202607260001_agent_unlimited_steps.sql`: Upgrades step budget constraints.
- **Observations on Gaps**:
  - Like SQLite, PostgreSQL lacks tables for `agent_memories`, `agent_findings`, and `agent_message_envelopes`.

#### 6. `crates/aro-runtime`
- **Location**: `crates/aro-runtime/src/lib.rs`
- **Role**: High-level orchestration engine (`AssistantEngine`), tying model providers, local memory, vector search, and tool execution together.
- **Core Execution Functions**:
  - `send_message()` (lines 118-338): Primary conversational entry point. Assembles context, retrieves memories via `ContinuousCompactor`, builds `ContextPack`, and executes `run_tool_loop()`.
  - `start_agent_run()` (lines 525-608): Creates conversation/lane if needed, initializes `AgentRun`, records step 1 (`RunStarted`) and step 2 (`ContextBuilt`), and returns `AgentRunView`.
  - `execute_orchestration_tool()` (lines 2428-2580):
    - When `TOOL_CORE_AGENT_DELEGATE` or `TOOL_CORE_AGENT_SPAWN` is called:
      - Reads `goal`, `mode`, `max_steps`, `system_prompt`, `name`, `role`, `icon` from input.
      - Dispatches `self.start_agent_run(sub_request).await?`.
      - Returns JSON `{ run_id, status: "running", goal, name, role, icon, mode }`.
    - When `TOOL_CORE_AGENT_STATUS` is called:
      - Looks up `agent_run_view(run_id)`.
      - Returns status, goal, steps count, and last step.

#### 7. `apps/desktop/src-tauri`
- **Location**: `apps/desktop/src-tauri/src/`
- **Role**: Tauri v2 backend running on the user's desktop, binding Rust to the frontend webview via IPC commands.
- **State**: `AppState` (`src/state.rs:21-30`) wraps `AssistantEngine`, `CloudApiClient`, `AppPaths`, and cached settings.
- **Registered Tauri Commands** (`src/main.rs:5050-5157`):
  - Agent run commands: `agent_run_start`, `agent_run_list`, `agent_orchestrator_snapshot`, `agent_lane_list`, `permission_profiles_list`, `permission_profile_upsert`, `agent_lane_pause`, `agent_lane_resume`, `agent_lane_set_priority`, `agent_run_get`, `agent_run_pause`, `agent_run_resume`, `agent_run_cancel`, `agent_context_search`.
  - Missing commands: No IPC commands for agent memory, scratchpads, findings, inter-agent messages, or sub-agent directive execution.

#### 8. `apps/api`
- **Location**: `apps/api/src/`
- **Role**: Headless server hosting Axum HTTP routes and background worker daemons.
- **Durable Worker**: `agent_runner::process_agent_jobs()` (`src/agent_runner.rs:21-45`) claims jobs from `agent_run_jobs` via `claim_agent_run_job()`, runs an iterative step loop against `ModelRouter`, validates actions, executes authorized tools via `execute_worker_tool()`, and updates leases every 15 seconds.

---

## 3. Multi-Agent Coordination & Collaborative Chain Analysis

The table below contrasts what is defined in contracts vs. what is currently implemented in Rust vs. what is stored in the frontend:

| Concept | TypeScript Contract (`@aro/contracts`) | Frontend State (`apps/desktop/src`) | Local SQLite (`aro-memory`) | Server Postgres (`aro-store`) | Runtime Engine (`aro-runtime`) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Scratchpad** | `AgentMemoryContext.scratchpad: string` | Stored in `localStorage` (`aro:agent-memory:...`) | In `WorkingMemoryBuffer` (in-memory struct only, not in DB) | Not stored | Not injected into sub-agent prompt |
| **Constats (Findings)** | `AgentMemoryFinding { id, summary, category, sourceTool, timestamp }` | Stored in `localStorage` (`recordAgentFinding`) | Not stored | Not stored | Not recorded or queryable by tools |
| **Artefacts Sharing** | `AgentArtifactRef { id, title, kind, uri }` | Stored in `localStorage` (`context.artifacts`) | Stored in `agent_artifacts` table per run | Stored in `agent_artifacts` table per run | No cross-agent artifact query tool |
| **Inter-Agent Ledger** | `AgentMessageEnvelope { id, sender, recipient, messageType, payload, ... }` | Stored in `localStorage` (`aro:agent-ledger:...`) | Not stored | Not stored | No inter-agent communication tool |
| **Sub-Agent Directives** | Directives sent via composer | Canned mock reply generated in `App.svelte` | Not executed | Not executed | No background execution spawned |
| **Session Persistence** | Expected across restarts | `subAgentMessagesMap` in-memory Svelte map | Only `agent_runs` & initial 2 steps | Only `agent_runs` & queue jobs | Cannot restore live sub-agent chat |

### Detailed Execution Trace: The "Freezing Sub-Agent" Defect
1. **Primary Agent Loop**:
   User sends a message in `ConversationView`. `AssistantEngine::send_message()` executes `run_tool_loop()`.
2. **Delegation Invocation**:
   The primary model chooses tool `core.agent.delegate` with payload:
   ```json
   { "goal": "Analyze AST security flaws", "mode": "code", "name": "Security Agent" }
   ```
3. **Execution in `aro-runtime`**:
   `execute_orchestration_tool()` receives the request and calls `start_agent_run(sub_request)`.
4. **SQLite Insertion**:
   `start_agent_run()` creates an `AgentRun` with status `Running`, writes step 1 (`Run started`) and step 2 (`Context pack built`), and returns an `AgentRunView`.
5. **No Thread / Task Launch**:
   `start_agent_run()` does **not** call `tokio::spawn()`. There is no loop waiting to invoke the LLM for step 3. The parent agent receives the tool output:
   ```json
   { "run_id": "9b1deb4d-...", "status": "running", "name": "Security Agent" }
   ```
6. **Parent Finishes, Sub-Agent Never Runs**:
   The parent agent believes the sub-agent is running in the background. In reality, the sub-agent is completely dead in the water.
7. **User Inspection in UI**:
   The user sees the micro-pill for "Security Agent" in `AgentMicroPills.svelte`. Clicking it sets `activeSubAgent` and switches `ConversationTopbar` to the breadcrumb view.
   Because `subAgentMessagesMap` has no entries, `getInitialSubAgentMessages()` generates:
   - Message 1 (user): *"Mission assignée à Security Agent : exécution autonome."*
   - Message 2 (assistant): *"Security Agent est en cours d'exécution de sa mission..."*
8. **User Sends Directive**:
   User types in the composer: *"Focus on buffer overflows first"*.
   `App.svelte:8303` intercepts the event:
   ```typescript
   if (activeSubAgent) {
     // ...
     const asstMsg = {
       content: "Directive bien reçue par **Security Agent**. Intégration en cours...",
       isGenerating: false
     };
     subAgentMessagesMap = { ...subAgentMessagesMap, [activeSubAgent.id]: [...currentThread, userMsg, asstMsg] };
     updateAgentScratchpad(activeConversation.id, activeSubAgent.id, content);
     return;
   }
   ```
   No backend call is ever made. The sub-agent never processes the directive.
9. **User Switches Conversation or Restarts App**:
   `subAgentMessagesMap` is cleared from JavaScript heap memory. The conversation with the sub-agent vanishes.

---

## 4. Contract Discrepancy & Type Mapping: Rust vs. TypeScript

Below is the definitive cross-language contract mapping between `@aro/contracts` and the Rust crates:

| `@aro/contracts` (TypeScript) | Rust Equivalent (`aro-core`) | SQLite (`aro-memory`) | Postgres (`aro-store`) | Status & Action Needed |
| :--- | :--- | :--- | :--- | :--- |
| `AgentRun` (`agent.ts:12-29`) | `AgentRun` (`agent.rs:220-238`) | Table `agent_runs` | Table `agent_runs` | **Aligned**. Fields map cleanly. |
| `AgentLane` (`agent.ts:31-38`) | `AgentLane` (`agent.rs:272-281`) | Table `agent_lanes` | Table `agent_lanes` | **Aligned**. |
| `AgentStep` (`agent.ts:179`) | `AgentStep` (`agent.rs:320-333`) | Table `agent_steps` | Table `agent_steps` | **Aligned**. |
| `AgentArtifact` (`artifacts.ts:1-25`) | `AgentArtifact` (`agent.rs:410-420`) | Table `agent_artifacts` | Table `agent_artifacts` | **Aligned**. |
| `AgentMemoryFinding` (`agent.ts:353-359`) | **Missing in Rust** | **Missing in SQLite** | **Missing in Postgres** | **Requires Rust struct + DB tables**. |
| `AgentMemoryContext` (`agent.ts:361-372`) | **Missing in Rust** (only partial `WorkingMemoryBuffer`) | **Missing in SQLite** | **Missing in Postgres** | **Requires Rust struct + DB tables**. |
| `AgentMessageEnvelope` (`agent.ts:334-351`) | **Missing in Rust** | **Missing in SQLite** | **Missing in Postgres** | **Requires Rust struct + DB tables**. |
| `AgentMessageType` (`agent.ts:306-316`) | **Missing in Rust** | **Missing in SQLite** | **Missing in Postgres** | **Requires Rust enum** (`kebab-case`/`snake_case`). |
| `AgentParticipant` (`agent.ts:319-325`) | **Missing in Rust** | **Missing in SQLite** | **Missing in Postgres** | **Requires Rust struct**. |
| `PermissionProfile` (`agent.ts:378-391`) | `PermissionProfile` (`agent.rs:153-167`) | Table `agent_permission_profiles` | Table `agent_permission_profiles` | **Aligned**. |

### Detailed Rust Struct Specifications Needed in `aro-core`

```rust
// Proposed additions to crates/aro-core/src/agent.rs

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentParticipant {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    pub icon: Option<String>,
    #[serde(rename = "type")]
    pub kind: AgentParticipantKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryFinding {
    pub id: String,
    pub summary: String,
    pub category: Option<String>,
    pub source_tool: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessagePayload {
    pub content: String,
    #[serde(default)]
    pub structured_data: Option<serde_json::Value>,
    #[serde(default)]
    pub artifacts: Vec<AgentArtifactRef>,
    #[serde(default)]
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentArtifactRef {
    pub id: String,
    pub title: String,
    pub kind: Option<String>,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageEnvelope {
    pub id: String,
    pub conversation_id: String,
    pub parent_message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub sender: AgentParticipant,
    pub recipient: AgentParticipant,
    pub message_type: AgentMessageType,
    pub payload: AgentMessagePayload,
    pub permission_profile_id: Option<String>,
    pub priority: Option<AgentRunPriority>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryContext {
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub conversation_id: String,
    pub scratchpad: String,
    pub findings: Vec<AgentMemoryFinding>,
    pub ledger: Vec<AgentMessageEnvelope>,
    pub artifacts: Vec<AgentArtifactRef>,
    pub permission_profile_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}
```

---

## 5. Storage Schema & Persistence Architecture

To guarantee session durability across app restarts and conversation switches without data loss, SQLite in `aro-memory` and PostgreSQL in `aro-store` must be extended with three dedicated tables:

### 1. `agent_memories`
Stores the isolated cognitive working state (scratchpad and metadata) per agent per conversation:
```sql
CREATE TABLE IF NOT EXISTS agent_memories (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  agent_name TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'worker',
  scratchpad TEXT NOT NULL DEFAULT '',
  permission_profile_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  sync_status TEXT NOT NULL DEFAULT 'local',
  UNIQUE (conversation_id, agent_id)
);
CREATE INDEX IF NOT EXISTS idx_agent_memories_conv ON agent_memories(conversation_id);
```

### 2. `agent_findings`
Stores verified findings/constats discovered by agents during exploration and tool execution:
```sql
CREATE TABLE IF NOT EXISTS agent_findings (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  summary TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'insight',
  source_tool TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_findings_conv_agent ON agent_findings(conversation_id, agent_id);
```

### 3. `agent_message_envelopes`
Stores the inter-agent communication ledger:
```sql
CREATE TABLE IF NOT EXISTS agent_message_envelopes (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  parent_message_id TEXT,
  correlation_id TEXT,
  sender_json TEXT NOT NULL,
  recipient_json TEXT NOT NULL,
  message_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  permission_profile_id TEXT,
  priority TEXT NOT NULL DEFAULT 'normal',
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_envelopes_conv ON agent_message_envelopes(conversation_id, created_at);
```

### Corresponding SQLite Store API in `SqliteMemoryStore`:
```rust
impl SqliteMemoryStore {
    pub fn get_agent_memory(&self, conversation_id: &str, agent_id: &str) -> AroResult<Option<AgentMemoryContext>>;
    pub fn upsert_agent_scratchpad(&self, conversation_id: &str, agent_id: &str, agent_name: &str, role: &str, scratchpad: &str) -> AroResult<()>;
    pub fn record_agent_finding(&self, conversation_id: &str, agent_id: &str, finding: &AgentMemoryFinding) -> AroResult<()>;
    pub fn append_agent_message(&self, envelope: &AgentMessageEnvelope) -> AroResult<()>;
    pub fn list_agent_messages(&self, conversation_id: &str, agent_id: Option<&str>) -> AroResult<Vec<AgentMessageEnvelope>>;
    pub fn list_subagent_messages(&self, conversation_id: &str, agent_id: &str) -> AroResult<Vec<ChatMessage>>;
    pub fn add_subagent_message(&self, message: &ChatMessage, agent_id: &str) -> AroResult<()>;
}
```

---

## 6. Autonomous Sub-Agent Execution Engine Design

### Background Sub-Agent Task Lifecycle
To solve the "freezing sub-agent" issue, when `start_agent_run` is invoked or when `core.agent.delegate` / `core.agent.spawn` is called on desktop, the runtime must:
1. Initialize the `AgentRun` record with status `Queued` or `Running`.
2. Persist the `task_delegation` envelope into `agent_message_envelopes`.
3. In `aro-runtime` (or `AppState`), trigger a background execution task via `tokio::spawn`:
   ```rust
   let engine = self.clone();
   let run_id = run.id;
   let conv_id = run.conversation_id.unwrap_or(run.id);
   tokio::spawn(async move {
       if let Err(err) = engine.execute_agent_run_loop(run_id, conv_id).await {
           tracing::error!(?err, %run_id, "background agent run failed");
       }
   });
   ```
4. In `execute_agent_run_loop`:
   - Load `AgentRun`, `AgentLane`, and the sub-agent's `AgentMemoryContext`.
   - Resolve model provider connection from settings.
   - Assemble context including:
     - The goal and instructions.
     - The compiled permission directive (`compilePermissionDirective`).
     - The cognitive scratchpad (`[WORKING SCRATCHPAD]`).
     - Prior findings (`[DISCOVERED FINDINGS]`).
     - The inter-agent ledger (`[INTER-AGENT MESSAGES]`).
   - Iterate step-by-step up to `max_steps`:
     - Generate next action from LLM.
     - Record `Model` step in `agent_steps`.
     - If `AgentActionType::Tool`:
       - Check permissions against `PermissionProfile`.
       - Execute tool.
       - Record `Tool` step and any artifacts in `agent_artifacts`.
       - If tool is a collaboration tool (`core.agent.scratchpad`, `core.agent.finding`, `core.agent.send_message`), mutate the sub-agent's cognitive context in SQLite!
       - Feed tool output back to the model history.
     - If `AgentActionType::Final`:
       - Record `Final` step.
       - Append `task_result` envelope to ledger.
       - Mark `AgentRun` as `Completed`.
       - Post desktop notification (`NotificationKind::AgentCompletion`).
       - Emit Tauri event `aro:agent-run-update` so the frontend UI immediately reflects the completion.

### Inter-Agent Collaboration Tools
We must equip the tool registry with four dedicated multi-agent coordination tools:

1. **`core.agent.scratchpad` (`agent.scratchpad`)**:
   - Allows an agent to update its cognitive scratchpad notes.
   - Input schema: `{ "scratchpad": { "type": "string" } }`
   - Persists immediately to SQLite `agent_memories`.
2. **`core.agent.finding` (`agent.finding`)**:
   - Allows an agent to log a verified discovery or constat.
   - Input schema: `{ "summary": { "type": "string" }, "category": { "type": "string" } }`
   - Persists immediately to SQLite `agent_findings`.
3. **`core.agent.send_message` (`agent.send_message`)**:
   - Allows an agent to message the user, parent orchestrator, or another sub-agent.
   - Input schema: `{ "recipient_id": { "type": "string" }, "message_type": { "type": "string" }, "content": { "type": "string" }, "artifacts": { "type": "array" } }`
   - Persists to `agent_message_envelopes`.
4. **`core.agent.artifact_share` (`agent.artifact_share`)**:
   - Allows an agent to attach an artifact to its parent or peer.
   - Input schema: `{ "artifact_id": { "type": "string" }, "target_agent_id": { "type": "string" } }`

---

## 7. Tauri IPC & Frontend Integration Plan

### New Tauri Commands in `apps/desktop/src-tauri/src/main.rs`:
```rust
#[tauri::command]
async fn agent_memory_get(state: State<'_, AppState>, conversation_id: String, agent_id: String) -> CommandResult<Option<AgentMemoryContext>>

#[tauri::command]
async fn agent_scratchpad_update(state: State<'_, AppState>, conversation_id: String, agent_id: String, scratchpad: String) -> CommandResult<()>

#[tauri::command]
async fn agent_finding_record(state: State<'_, AppState>, conversation_id: String, agent_id: String, summary: String, category: Option<String>, source_tool: Option<String>) -> CommandResult<AgentMemoryFinding>

#[tauri::command]
async fn agent_message_send(state: State<'_, AppState>, envelope: AgentMessageEnvelope) -> CommandResult<()>

#[tauri::command]
async fn agent_message_list(state: State<'_, AppState>, conversation_id: String, agent_id: Option<String>) -> CommandResult<Vec<AgentMessageEnvelope>>

#[tauri::command]
async fn agent_subagent_messages_get(state: State<'_, AppState>, conversation_id: String, agent_id: String) -> CommandResult<Vec<ChatMessage>>

#[tauri::command]
async fn agent_subagent_directive_send(state: State<'_, AppState>, conversation_id: String, agent_id: String, directive: String) -> CommandResult<AgentRunView>
```

### Desktop UI Integration in `apps/desktop/src/lib/agent-protocol.ts` & `App.svelte`:
1. In `agent-protocol.ts`:
   - Replace direct `localStorage.getItem` / `setItem` with Tauri IPC calls (`invoke("agent_memory_get", ...)`, etc.), falling back to `localStorage` only in browser web mode without Tauri.
2. In `App.svelte`:
   - When switching conversations (`selectConversation`), fetch sub-agent messages from SQLite via `agent_subagent_messages_get`.
   - When a directive is sent to a sub-agent (`if (activeSubAgent)` in `handleSend`), call `invoke("agent_subagent_directive_send", ...)` to dispatch real execution rather than generating a mock response.

---

## 8. Requirements Traceability Matrix for Requirement R1

| Requirement Identifier | Requirement Statement | Architectural Component | Implementation Status | Action Required |
| :--- | :--- | :--- | :--- | :--- |
| **R1.1** | Fluid multi-agent coordination & communication | `aro-core`, `aro-agent`, `agent_message_envelopes` | Partial (Types in TS, missing in Rust) | Implement Rust structs, DB tables, IPC commands, and `core.agent.send_message` tool. |
| **R1.2** | Cognitive Scratchpad sharing & updates | `aro-core`, `agent_memories` table | Partial (In `localStorage` & `WorkingMemoryBuffer`) | Implement `agent_memories` table, SQLite persistence, and `core.agent.scratchpad` tool. |
| **R1.3** | Constats (Findings) tracking & sharing | `aro-core`, `agent_findings` table | Partial (TS contracts only, in `localStorage`) | Implement `agent_findings` table, SQLite persistence, and `core.agent.finding` tool. |
| **R1.4** | Artifacts sharing across agents | `agent_artifacts` table, `AgentArtifactRef` | Existing per-run, missing cross-agent query | Expose cross-agent artifact references in `AgentMemoryContext`. |
| **R1.5** | Autonomous background sub-agent execution | `aro-runtime` (`execute_agent_run_loop`) | Incomplete (Runs freeze at step 2) | Implement `tokio::spawn` loop in desktop runtime and cloud worker delegation handler. |
| **R1.6** | Full session persistence across restarts | SQLite `agent_memories`, `messages`, `agent_steps` | Incomplete (Subagent chat lost on restart) | Persist subagent message threads and cognitive contexts to SQLite. |
| **R1.7** | Zero-Regression Quality Invariants | All CI test suites | Checked and Passing baseline | Maintain 100% pass rate on `contracts:check`, `api-client:test`, `test:unit`, `test:components`, `lint:rust`. |

---

## 9. Conclusion & Recommendations for Phase 1 Plan

The survey reveals that ARO already possesses an exceptional design foundation. The lack of active sub-agent collaboration is not an insurmountable architectural conflict, but rather an unfinished bridge between the frontend TypeScript contracts and the backend Rust persistence/runtime layers.

**Key Recommendations for Phase 1 Implementation**:
1. **Extend `aro-core`**: Add `AgentMemoryFinding`, `AgentMemoryContext`, `AgentMessageEnvelope`, `AgentMessageType`, and `AgentParticipant` with clean camelCase serde serialization.
2. **Add SQLite Schemas & Store Methods in `aro-memory`**: Create `agent_memories`, `agent_findings`, and `agent_message_envelopes` tables with indices and cascading foreign keys.
3. **Wire Desktop Autonomous Execution in `aro-runtime`**: Enable `execute_agent_run_loop` via `tokio::spawn` when a run is started or delegated.
4. **Expose Tauri IPC Commands**: Expose memory, finding, envelope, and subagent directive execution commands in `apps/desktop/src-tauri`.
5. **Bridge Frontend `agent-protocol.ts` & `App.svelte`**: Migrate off `localStorage` to Rust IPC and trigger genuine background sub-agent execution on directive submission.

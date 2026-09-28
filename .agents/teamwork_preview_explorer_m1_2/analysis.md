# Feature 2 Exploration & Implementation Strategy: Sub-Agent Asynchronous Execution Loop

**Target Crate**: `crates/aro-runtime`  
**Milestone**: Milestone 1 (Feature 2)  
**Author**: `teamwork_preview_explorer_m1_2`  
**Date**: 2026-09-24  

---

## 1. Executive Summary

Feature 2 addresses a fundamental operational defect in the ARO multi-agent desktop runtime: **sub-agents freeze immediately at Step 2 (`ContextBuilt`) and never execute model turns or tools**.

When a user or parent agent invokes `core.agent.delegate` / `core.agent.spawn` or triggers desktop run start via `agent_run_start`, the runtime calls `AssistantEngine::start_agent_run`. This method creates the `AgentRun` entity, assigns it a lane, records Step 1 (`RunStarted`, sequence 1), builds the initial `ContextPack`, and records Step 2 (`ContextBuilt`, sequence 2). It then immediately returns `AgentRunView` to the caller. Crucially, **no background task (`tokio::spawn`) is ever scheduled**, no model provider loop is invoked, and no executor advances the run. The sub-agent stays permanently stuck with `stepCount = 2`, `status = "running"`, and zero progress.

This document establishes the root cause, provides an exhaustive analysis of the execution mechanics across `aro-runtime`, `aro-tools`, `aro-agent`, and `aro-memory`, and defines a battle-tested architecture for an asynchronous sub-agent loop powered by `tokio::spawn`, complete with cooperative cancellation, state machine transitions, thought/step tracking, and lane queue concurrency control.

---

## 2. Root Cause Analysis: Why Sub-Agents Freeze at Step 2

### 2.1 The Execution Flow in `crates/aro-runtime/src/lib.rs`

In `crates/aro-runtime/src/lib.rs`, `AssistantEngine::start_agent_run` is implemented at lines 525–608:

```rust
// crates/aro-runtime/src/lib.rs:568-608
let mut run = self.agent.start_run(
    &AgentRunStartRequest {
        lane_id: Some(lane.id),
        priority: request
            .priority
            .clone()
            .or_else(|| Some(lane.priority.clone())),
        ..request
    },
    autonomy_profile_id,
);
self.agent.schedule_run(
    &mut run,
    &lane,
    self.store.count_running_agent_runs()?,
    self.store.count_running_agent_runs_for_lane(lane.id)?,
);
self.store.upsert_agent_run(&run)?;
self.store
    .add_agent_step(&self.run_started_step(&run, max_steps))?; // Step 1 (sequence 1)

let history = conversation_id
    .map(|id| self.store.list_messages(id))
    .transpose()?
    .unwrap_or_default();
let memory_sources = self.memory_context_sources(trimmed, 8).await?;
let context_pack =
    self.agent
        .build_context_pack(&run, &history, &memory_sources, &[], environment);
self.touch_recalled_memories(&memory_sources)?;
self.store
    .add_agent_step(&self.agent.context_step(&run, &context_pack))?; // Step 2 (sequence 2)

// TERMINATION: start_agent_run exits right here!
Ok(AgentRunView {
    run: self
        .store
        .get_agent_run(run.id)?
        .ok_or_else(|| AroError::Memory("agent run not found".to_string()))?,
    steps: self.store.list_agent_steps(run.id)?,
    artifacts: self.store.list_agent_artifacts(run.id)?,
    context_pack: Some(context_pack),
})
```

### 2.2 Direct Consequences

1. **Step Count Capped at 2**:
   - `run_started_step(&run, max_steps)` records Step 1 (`sequence = 1`, `kind = AgentStepKind::RunStarted`).
   - `context_step(&run, &context_pack)` records Step 2 (`sequence = 2`, `kind = AgentStepKind::ContextBuilt`).
   - Nothing executes after line 608. The database contains only steps 1 and 2.
2. **Parent Agent Status Polling Deadlock**:
   - In `crates/aro-runtime/src/lib.rs:2435-2540`, when an orchestrator or parent agent delegates a task via `TOOL_CORE_AGENT_DELEGATE` (`agent.delegate`), it receives `{ run_id: "...", status: "running" }`.
   - When the parent later queries `TOOL_CORE_AGENT_STATUS` (`agent.status`, lines 2541–2584), `agent_run_view` returns `steps_count = 2`, `status = "running"`, and `last_step = ContextBuilt`.
   - The sub-agent never reaches `completed` or `failed`.
3. **Desktop UI Starvation**:
   - In `apps/desktop/src/App.svelte` (lines 915–970), the UI reads sub-agent status and steps. The sub-agent micro-pill shows a perpetual spinner ("running"), thoughts remain empty, and no tools are reported as executed.
4. **Contrast with Main Chat Loop**:
   - In contrast, `AssistantEngine::send_message` (lines 140–338) and `send_message_stream` (lines 1501–1650) explicitly invoke `self.run_tool_loop(...)` within the request future. But running `run_tool_loop` synchronously inside `start_agent_run` is unacceptable because it would block the IPC caller (or parent agent tool execution) for minutes or hours, preventing concurrency.

---

## 3. Architecture for Asynchronous Subagent Execution

### 3.1 Background Task Loop with `tokio::spawn`

To decouple subagent execution from the initiating request and allow unbounded multi-turn execution, `AssistantEngine` must launch a background `tokio::spawn` task when `start_agent_run` schedules a run with status `AgentRunStatus::Running`.

```
                    start_agent_run(request)
                              │
               ┌──────────────┴──────────────┐
               ▼                             ▼
       run.status == Queued          run.status == Running
               │                             │
    Saved in SQLite queue            Persist Steps 1 & 2
    (waits for lane slot)                    │
                                     tokio::spawn
                                             │
                       ┌─────────────────────┴─────────────────────┐
                       ▼                                           ▼
             Immediate Return to Caller               Background Sub-Agent Loop
             (AgentRunView with steps 1 & 2)           (execute_subagent_run_loop)
                                                                   │
                                                       ┌───────────┴───────────┐
                                                       ▼                       ▼
                                                Turn Iteration          Tool Execution
                                                (Model Thought)         (ToolExecutor)
                                                       │                       │
                                                       └───────────┬───────────┘
                                                                   ▼
                                                            Terminal State
                                                        (Completed / Failed)
                                                                   │
                                                        Drain Queued Runs in Lane
```

### 3.2 Concurrency and Lane Scheduler Integration

The ARO runtime already features `AgentLane` with concurrency controls:
- `AgentLane.max_concurrent_runs` (defaults to 1, configurable per lane).
- `agent.max_global_running()` (global throttle).
- In `SqliteMemoryStore`, `count_running_agent_runs()` and `count_running_agent_runs_for_lane(lane_id)` compute active concurrency.

When a sub-agent background run completes (or fails or is cancelled):
1. It updates its own run status in SQLite.
2. It invokes a scheduler drain hook: `drain_queued_runs_for_lane(lane_id)`.
3. If a `Queued` run exists in that lane and the lane has capacity:
   - Transition `Queued` -> `Running`.
   - Persist status update.
   - Spawn the sub-agent loop for the dequeued run via `tokio::spawn`.

### 3.3 Model Provider Management in `AssistantEngine`

Currently, `send_message` receives `provider: &dyn ModelProvider` as an argument from the caller. But a spawned background task cannot borrow a reference from the stack.
Therefore, `AssistantEngine` needs a thread-safe, cloneable reference to a provider:

```rust
pub struct AssistantEngine {
    store: SqliteMemoryStore,
    agent: AgentRuntime,
    vector: MemoryVectorService,
    tools: ToolExecutor,
    plugins: std::sync::Arc<aro_plugins::PluginManager>,
    // Provider resolution for autonomous background runs
    provider: std::sync::Arc<tokio::sync::RwLock<Option<std::sync::Arc<dyn ModelProvider>>>>,
    active_runs: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<uuid::Uuid, tokio::sync::watch::Sender<bool>>>>,
}
```

- Desktop initialization (`apps/desktop/src-tauri/src/state.rs`) sets the provider on `engine` using `ModelRouter::from_active(&settings.model, api_key)`.
- When settings change, `engine.set_model_provider(Arc::new(router))` atomically updates the provider for future and running turns.
- Unit and integration tests can supply a `MockProvider` or `FinalAfterNProvider` via `engine.set_model_provider(...)`.
- If no provider is explicitly set, fallback to `LocalModelProvider::from_settings(&ModelSettings::default())` prevents hard crashes.

---

## 4. Subagent Execution State Machine & Lifecycle Tracking

### 4.1 State Machine Transitions

```
                    ┌──────────────┐
                    │    Queued    │
                    └──────┬───────┘
                           │ (lane capacity available)
                           ▼
                    ┌──────────────┐
       ┌───────────►│   Running    │◄──────────┐
       │            └──────┬───────┘           │
       │                   │                   │
(resume/unpause)           │                   │
       │        ┌──────────┴──────────┐        │
       │        ▼                     ▼        │
 ┌──────────┐ ┌───────────┐     ┌───────────┐  │
 │  Paused  │ │  Waiting  │     │ Cancelled │  │
 └──────────┘ └─────┬─────┘     └───────────┘  │
                    │                          │
                    └──────────────────────────┘
                               │
                ┌──────────────┴──────────────┐
                ▼                             ▼
         ┌─────────────┐               ┌─────────────┐
         │  Completed  │               │   Failed    │
         └─────────────┘               └─────────────┘
```

1. **`Queued` -> `Running`**:
   Triggered on start if capacity is available, or when a preceding run completes in the same lane.
2. **`Running` -> `Completed`**:
   The model produces an action with `AgentActionType::Final` (or max step limit is reached without error).
   `completed_at` is timestamped, `heartbeat_at` updated.
3. **`Running` -> `Failed`**:
   Occurs if tool execution returns a terminal error, permission check fails closed, model validation fails, or step/wall-clock safety ceiling is breached.
   `last_error` is recorded, `completed_at` timestamped.
4. **`Running` -> `Waiting`**:
   Occurs when `AgentActionType::Pause` is emitted (awaiting clarification or external event).
5. **`Running` / `Queued` -> `Cancelled`**:
   Triggered by user cancellation (`agent_run_cancel`) or parent abort. Cooperative cancellation token signals the loop, which halts before the next model or tool turn.

### 4.2 Step Sequence and Observability Tracking

Every step recorded in SQLite via `add_agent_step` must have a strictly incrementing `sequence: i32`:
- **Sequence 1 (`AgentStepKind::RunStarted`)**: Goal, mode, max steps.
- **Sequence 2 (`AgentStepKind::ContextBuilt`)**: Initial context pack, recalled memories, registered tools.
- **Sequence 3 (`AgentStepKind::Model`)**:
  - `input`: excerpt of prompt.
  - `output`: parsed `AgentAction` (tool call, final, thinking).
  - `run.checkpoint_summary`: updated with `action.thinking` or `action.reason`.
- **Sequence 4 (`AgentStepKind::Tool`)**:
  - `input`: `{ toolId, input, requestedAt }`.
  - `output`: serialized `ToolExecutionResult`.
  - `status`: `Completed` or `Failed`.
- **Sequence 5 (`AgentStepKind::ContextBuilt`)**:
  - Context pack refreshed with tool output in history.
- ... continues until Sequence N (`AgentStepKind::Model` / `Final`).

### 4.3 Capturing Thoughts & Cognitive Scratchpads

1. **Thoughts**:
   - Models output thoughts either via `<think>...</think>` tags in content or via structured JSON field `"thinking"`.
   - `parse_model_action` extracts `action.thinking`.
   - The loop writes `run.checkpoint_summary = action.thinking.clone()` and persists `run` to SQLite.
   - Desktop UI (`App.svelte:937`) immediately renders `agent.currentThought` via the breadcrumb or inspection drawer.
2. **Cognitive Memory Findings**:
   - For every successful tool execution (e.g. `workspace.read`, `web_search`), the loop can record an `AgentMemoryFinding` (`"discovery"` or `"fact"`) linked to `run.id` and `conversation_id`.
   - Scratchpad text in `AgentMemoryContext` is updated with `Step {n}: {thought}`.

---

## 5. Cooperative Cancellation and Task Management

### 5.1 The Need for Cooperative Cancellation

Because model generation calls (`provider.generate`) and network/tool calls are asynchronous operations, killing a thread forcefully is unsafe and can corrupt SQLite transactions.

Instead, we employ a dual-layer cooperative cancellation mechanism:

1. **Layer 1: In-Memory Watch Channel (`tokio::sync::watch`)**:
   - When `AssistantEngine::spawn_subagent_loop` launches a run, it creates a `watch::channel(false)`.
   - The `Sender<bool>` is registered in `engine.active_runs.lock().await.insert(run_id, tx)`.
   - When `update_agent_run_status(run_id, AgentRunStatus::Cancelled)` is invoked:
     - SQLite is updated to `Cancelled`.
     - `active_runs.get(&run_id).send(true)` notifies the loop.
2. **Layer 2: Database Status Check at Loop Boundaries**:
   - At the beginning of every turn in `execute_subagent_run_loop`:
     ```rust
     let current = self.store.get_agent_run(run_id)?;
     if let Some(r) = current {
         if matches!(r.status, AgentRunStatus::Cancelled | AgentRunStatus::Paused) {
             tracing::info!(%run_id, status = ?r.status, "sub-agent loop halted by status transition");
             return Ok(());
         }
     }
     ```
   - This ensures that even if cancellation originated from a different process or thread, the loop halts before issuing another expensive LLM turn.

---

## 6. Interaction Contracts with `aro-tools` and `aro-agent`

### 6.1 Interaction with `aro-agent`

`aro-agent` (`AgentRuntime`) acts as the cognitive brain:
1. `agent.model_request(&run, &context_pack, system_prompt, history, user_input, ...)`: Builds the standardized `ModelGenerationRequest`.
2. `agent.parse_model_action(&generation.content)`: Parses LLM output into `AgentAction`.
3. `agent.validate_action(&action, &context_pack)`: Checks if the requested tool exists in `context_pack.tools`. If unknown, returns an error step rather than crashing.
4. `agent.model_step(...)` and `agent.context_step_at(...)`: Constructs fully-formed `AgentStep` domain entities.

### 6.2 Interaction with `aro-tools`

`aro-tools` (`ToolExecutor`) acts as the execution arms:
1. When `action.action_type == AgentActionType::Tool`:
   - Tool request is formed: `ToolExecutionRequest::new(run.id, run.conversation_id, tool_id, input)`.
   - Workspace root resolution: If `root_path` is not provided, resolved from `self.store.resolve_effective_root_path(conv_id)`.
2. Routing:
   - Built-in runtime tools (`core.plan.*`, `core.memory.*`, `core.context.search`, `core.agent.*`, `core.skill.*`, `core.connector.*`, `core.mcp.*`) are executed directly by `AssistantEngine`.
   - System, code, workspace, browser, shell tools are delegated to `self.tools.execute(request, policy).await`.
3. Results & Artifacts:
   - Tool outputs and artifacts (`AgentArtifact`) are stored in SQLite via `store_tool_execution`.
   - Tool outcome is formatted as a message in `history`:
     ```rust
     history.push(ChatMessage::new(
         conv_id,
         MessageRole::User,
         format!("Tool `{tool_name}` returned: {}\n{}", result.title, result.summary),
     ));
     ```
   - Context pack is rebuilt, and the next turn is triggered.

---

## 7. Concrete Implementation Blueprint

### 7.1 New Module: `crates/aro-runtime/src/scheduler.rs`

To keep `lib.rs` clean and maintainable, encapsulate the subagent execution loop in `crates/aro-runtime/src/scheduler.rs`:

```rust
// crates/aro-runtime/src/scheduler.rs
use std::sync::Arc;
use tokio::sync::{watch, Mutex, RwLock};
use std::collections::HashMap;
use uuid::Uuid;
use aro_core::{AgentRun, AgentRunStatus, AroResult, AroError, AgentStep, AgentStepKind};
use crate::{AssistantEngine, ModelProvider};

pub struct RunController {
    cancel_tx: watch::Sender<bool>,
}

#[derive(Clone, Default)]
pub struct SubAgentScheduler {
    active_runs: Arc<Mutex<HashMap<Uuid, RunController>>>,
}

impl SubAgentScheduler {
    pub fn new() -> Self {
        Self {
            active_runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, run_id: Uuid) -> watch::Receiver<bool> {
        let (tx, rx) = watch::channel(false);
        let mut map = self.active_runs.lock().await;
        map.insert(run_id, RunController { cancel_tx: tx });
        rx
    }

    pub async fn unregister(&self, run_id: &Uuid) {
        let mut map = self.active_runs.lock().await;
        map.remove(run_id);
    }

    pub async fn cancel(&self, run_id: &Uuid) -> bool {
        let map = self.active_runs.lock().await;
        if let Some(ctrl) = map.get(run_id) {
            let _ = ctrl.cancel_tx.send(true);
            true
        } else {
            false
        }
    }
}
```

### 7.2 Core Execution Loop: `execute_subagent_run_loop`

In `crates/aro-runtime/src/lib.rs` (or `scheduler.rs`):

```rust
impl AssistantEngine {
    pub fn spawn_subagent_loop(&self, run_id: Uuid, max_steps: Option<u32>) {
        let engine = self.clone();
        tokio::spawn(async move {
            if let Err(err) = engine.execute_subagent_run_loop(run_id, max_steps).await {
                tracing::error!(%run_id, ?err, "sub-agent background execution failed");
            }
        });
    }

    pub async fn execute_subagent_run_loop(
        &self,
        run_id: Uuid,
        max_steps: Option<u32>,
    ) -> AroResult<()> {
        let mut cancel_rx = self.scheduler.register(run_id).await;
        
        let result = async {
            let mut run = self.store.get_agent_run(run_id)?
                .ok_or_else(|| AroError::Memory("agent run not found".to_string()))?;

            if !matches!(run.status, AgentRunStatus::Running) {
                return Ok(());
            }

            let provider = self.resolve_model_provider(&run.model_provider_id, &run.model_id).await?;
            let budget = AgentLoopBudget::from_env().capped_by(max_steps);
            
            let existing_steps = self.store.list_agent_steps(run.id)?;
            let mut next_sequence = existing_steps.iter().map(|s| s.sequence).max().unwrap_or(2) + 1;
            
            let mut history = run.conversation_id
                .map(|id| self.store.list_messages(id))
                .transpose()?
                .unwrap_or_default();

            let memory_sources = self.memory_context_sources(&run.goal, 8).await?;
            let mut context_sources = memory_sources.clone();
            let mut context_pack = self.agent.build_context_pack(
                &run,
                &history,
                &context_sources,
                &[],
                EnvironmentSnapshot::desktop_local(run.model_provider_id.clone(), run.model_id.clone()),
            );

            let window_mgr = ContextWindowManager::default();
            let loop_started = std::time::Instant::now();
            let mut iterations: u32 = 0;
            let policy = web_policy_for(WebAccessMode::Off, None);

            loop {
                iterations += 1;

                // 1. Cooperative cancellation check
                if *cancel_rx.borrow() {
                    tracing::info!(%run_id, "sub-agent received cancellation signal");
                    run.status = AgentRunStatus::Cancelled;
                    run.completed_at = Some(Utc::now());
                    self.store.upsert_agent_run(&run)?;
                    return Ok(());
                }

                // 2. Database state check (external status update)
                if let Ok(Some(current)) = self.store.get_agent_run(run.id) {
                    if matches!(current.status, AgentRunStatus::Cancelled | AgentRunStatus::Paused) {
                        return Ok(());
                    }
                }

                // 3. Step budget check
                if let Some(breach) = budget.breach(iterations, loop_started.elapsed()) {
                    run.status = AgentRunStatus::Failed;
                    run.last_error = Some(breach.clone());
                    run.completed_at = Some(Utc::now());
                    self.store.upsert_agent_run(&run)?;
                    self.store.add_agent_step(&AgentStep::failed(
                        run.id,
                        next_sequence,
                        AgentStepKind::Error,
                        "Safety budget exceeded",
                        json!({ "iterations": iterations }),
                        breach,
                    ))?;
                    return Ok(());
                }

                // 4. Update heartbeat
                run.heartbeat_at = Some(Utc::now());
                self.store.upsert_agent_run(&run)?;

                // 5. Generate model action
                let reserve_cap = window_mgr.budget().reserve_budget as u32;
                let max_gen_tokens = provider.max_tokens().min(reserve_cap);
                let generation_request = self.agent.model_request(
                    &run,
                    &context_pack,
                    run.mode.system_instruction(),
                    history.clone(),
                    run.goal.clone(),
                    provider.temperature(),
                    max_gen_tokens,
                    ModelResponseFormat::AgentActionJson,
                );

                let generation = provider.generate(generation_request).await?;
                let action = self.agent.parse_model_action(&generation.content);
                let validation_error = self.agent.validate_action(&action, &context_pack).err();

                // 6. Record thought
                if let Some(thinking) = &action.thinking {
                    run.checkpoint_summary = Some(thinking.clone());
                    let _ = self.store.upsert_agent_run(&run);
                }

                // 7. Record Model Step
                let model_step = self.agent.model_step(
                    &run,
                    next_sequence,
                    &generation.content,
                    &action,
                    validation_error.as_deref(),
                );
                self.store.add_agent_step(&model_step)?;
                next_sequence += 1;

                if let Some(error) = validation_error {
                    run.status = AgentRunStatus::Failed;
                    run.last_error = Some(error);
                    run.completed_at = Some(Utc::now());
                    self.store.upsert_agent_run(&run)?;
                    return Ok(());
                }

                // 8. Process Action
                match action.action_type {
                    AgentActionType::Final => {
                        run.status = AgentRunStatus::Completed;
                        run.completed_at = Some(Utc::now());
                        self.store.upsert_agent_run(&run)?;
                        break;
                    }
                    AgentActionType::Pause => {
                        run.status = AgentRunStatus::Waiting;
                        self.store.upsert_agent_run(&run)?;
                        break;
                    }
                    AgentActionType::Tool => {
                        let tool_result = self.execute_agent_tool(
                            &run,
                            &action,
                            &policy,
                            next_sequence,
                            &mut None,
                        ).await?;
                        next_sequence += 1;
                        context_sources.extend(tool_result.context_sources.clone());

                        let tool_name = action.tool_id.as_deref().unwrap_or("unknown");
                        history.push(ChatMessage::new(
                            run.conversation_id.unwrap_or(run.id),
                            MessageRole::Assistant,
                            format!("Calling tool `{tool_name}` with arguments: {}", action.input),
                        ));
                        history.push(ChatMessage::new(
                            run.conversation_id.unwrap_or(run.id),
                            MessageRole::User,
                            format!("Tool `{tool_name}` returned: {}\n{}", tool_result.title, tool_result.summary),
                        ));

                        context_pack = self.agent.build_context_pack(
                            &run,
                            &history,
                            &context_sources,
                            &[],
                            EnvironmentSnapshot::desktop_local(run.model_provider_id.clone(), run.model_id.clone()),
                        );
                        self.store.add_agent_step(&self.agent.context_step_at(&run, next_sequence, &context_pack))?;
                        next_sequence += 1;
                    }
                }
            }

            Ok(())
        }.await;

        self.scheduler.unregister(&run_id).await;

        // Drain next queued run in the lane if this run is terminal
        if let Ok(Some(finished_run)) = self.store.get_agent_run(run_id) {
            if let Some(lane_id) = finished_run.lane_id {
                self.drain_lane_queue(lane_id).await;
            }
        }

        result
    }
}
```

### 7.3 Modification in `start_agent_run`

In `crates/aro-runtime/src/lib.rs:599-608`:
After writing Step 2 (`context_step`):
```rust
        self.store
            .add_agent_step(&self.agent.context_step(&run, &context_pack))?;

        // SPAWN ASYNC LOOP: If the run is scheduled as Running, launch background worker!
        if matches!(run.status, AgentRunStatus::Running) {
            self.spawn_subagent_loop(run.id, max_steps);
        }

        Ok(AgentRunView {
            run: self
                .store
                .get_agent_run(run.id)?
                .ok_or_else(|| AroError::Memory("agent run not found".to_string()))?,
            steps: self.store.list_agent_steps(run.id)?,
            artifacts: self.store.list_agent_artifacts(run.id)?,
            context_pack: Some(context_pack),
        })
```

---

## 8. Verification Strategy & Test Matrix

| Test ID | Test Target | Verification Command | Expected Outcome |
|---------|-------------|----------------------|------------------|
| **V1** | Unit compilation | `cargo check -p aro-runtime` | Clean 0 warnings/errors. |
| **V2** | Multi-step loop execution | `cargo test -p aro-runtime --lib` | Sub-agent executes past step 2 to step 5+ until Final. |
| **V3** | Cooperative cancellation | `cargo test -p aro-runtime test_subagent_cancellation` | Status transitions to `cancelled`, loop terminates cleanly within 50ms. |
| **V4** | Max steps boundary capping | `cargo test -p aro-runtime test_subagent_max_steps_cap` | Loop halts precisely at `max_steps`, status set to `completed` or `failed`. |
| **V5** | Tool execution & step recording | `cargo test -p aro-runtime test_subagent_tool_execution` | Steps 3 (Model), 4 (Tool), 5 (Context) recorded sequentially in SQLite. |
| **V6** | E2E Tier 1 Feature Coverage | `npm run test:e2e:tier1` | All tests in T1.2 (`T1.2.1` to `T1.2.5`) pass 100%. |
| **V7** | E2E Tier 2 Boundaries | `npm run test:e2e:tier2` | All tests in T2.2 (`T2.2.1` to `T2.2.5`) pass 100%. |

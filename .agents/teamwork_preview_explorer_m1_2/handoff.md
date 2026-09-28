# Handoff Report: Sub-Agent Asynchronous Execution Loop in `aro-runtime`

**Agent ID**: `teamwork_preview_explorer_m1_2`  
**Milestone**: Milestone 1 (Feature 2)  
**Date**: 2026-09-24  
**Detailed Report**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m1_2\analysis.md`  

---

## 1. Observation

1. **`crates/aro-runtime/src/lib.rs:525-608` (`start_agent_run`)**:
   - Line 568: `let mut run = self.agent.start_run(&AgentRunStartRequest { ... }, autonomy_profile_id);`
   - Lines 579–584: `self.agent.schedule_run(&mut run, &lane, self.store.count_running_agent_runs()?, self.store.count_running_agent_runs_for_lane(lane.id)?);`
   - Line 585: `self.store.upsert_agent_run(&run)?;`
   - Lines 586–587: `self.store.add_agent_step(&self.run_started_step(&run, max_steps))?;` (Step 1, `sequence: 1`, `kind: RunStarted`)
   - Lines 592–595: Context pack built and Recall memories touched.
   - Lines 597–598: `self.store.add_agent_step(&self.agent.context_step(&run, &context_pack))?;` (Step 2, `sequence: 2`, `kind: ContextBuilt`)
   - Lines 599–608:
     ```rust
     Ok(AgentRunView {
         run: self.store.get_agent_run(run.id)?.ok_or_else(|| AroError::Memory("agent run not found".to_string()))?,
         steps: self.store.list_agent_steps(run.id)?,
         artifacts: self.store.list_agent_artifacts(run.id)?,
         context_pack: Some(context_pack),
     })
     ```
   - Verbatim finding: `start_agent_run` returns immediately after persisting step 2. No `tokio::spawn` is invoked, no execution loop is launched, and no worker claims the run on desktop.

2. **`crates/aro-runtime/src/lib.rs:2435-2540` (`execute_orchestration_tool`)**:
   - For `TOOL_CORE_AGENT_DELEGATE` / `TOOL_CORE_AGENT_SPAWN`:
     - Line 2521: `let view = self.start_agent_run(sub_request).await?;`
     - Returns `{ run_id: view.run.id, status: status_str, goal: goal, ... }`.
   - For `TOOL_CORE_AGENT_STATUS`:
     - Lines 2549–2551: `let view = self.agent_run_view(run_id)?; let status_str = format!("{:?}", view.run.status).to_lowercase(); let last_step = view.steps.last().cloned();`
     - Because `start_agent_run` only wrote steps 1 and 2, `agent.status` always reports `steps_count = 2`, `last_step = ContextBuilt`, and `status = running`. The sub-agent stays permanently stuck at step 2.

3. **`apps/desktop/src-tauri/src/main.rs:3281-3305` (`agent_run_start`) & `state.rs:21-30`**:
   - `agent_run_start` simply awaits `state.engine.start_agent_run(request.clone()).await`.
   - `AppState` owns `engine: AssistantEngine` and `settings: Mutex<AppSettings>`, but `AssistantEngine` lacks an internal model provider reference, requiring `provider` to be passed on every call in chat, but leaving `start_agent_run` with no provider for background turns.

4. **Existing Tool Loop Capabilities in `crates/aro-runtime/src/lib.rs:931-1115` (`run_tool_loop`)**:
   - `run_tool_loop` already supports multi-turn execution with model action validation (`validate_action`), model step recording (`model_step`), tool execution via `execute_agent_tool`, tool step storage (`store_tool_execution`), artifact recording, and history expansion.
   - However, it currently lacks cooperative cancellation checks against SQLite status or watch channels during iteration, and is only invoked synchronously within `send_message` / `send_message_stream`.

5. **Test Suite Baseline**:
   - `cargo check -p aro-runtime` passes cleanly with code 0 (13.99s).
   - `cargo test -p aro-runtime --lib` passes 24/24 unit tests cleanly with code 0 (34.26s).

---

## 2. Logic Chain

1. **Step 1 (From Observation 1)**: `start_agent_run` is the single entry point for starting agent runs in `aro-runtime` on desktop, called by both Tauri IPC (`agent_run_start`) and orchestration tools (`core.agent.delegate`, `core.agent.spawn`).
2. **Step 2 (From Observation 1)**: `start_agent_run` strictly records sequence 1 (`RunStarted`) and sequence 2 (`ContextBuilt`), and then immediately returns without scheduling any asynchronous future or task.
3. **Step 3 (From Observation 2 & 3)**: Because no background future is scheduled, no code advances the run. Parent agents polling via `core.agent.status` observe `steps_count = 2` forever, and desktop UI micro-pills show an eternal "running" spinner with no thoughts or tools.
4. **Step 4 (From Observation 4)**: The core multi-step reasoning and tool execution engine (`run_tool_loop`) already exists in `aro-runtime`, but it requires a `ModelProvider` reference and is designed for synchronous invocation within chat requests.
5. **Step 5 (Synthesis)**: To solve the step 2 freeze, `AssistantEngine` must:
   - Store a thread-safe `Arc<RwLock<Option<Arc<dyn ModelProvider>>>>` (or provider resolver) so spawned tasks can generate model turns autonomously.
   - Introduce `spawn_subagent_loop(run_id, max_steps)` inside `start_agent_run` immediately after writing step 2 when `run.status == AgentRunStatus::Running`.
   - Implement `execute_subagent_run_loop` with cooperative cancellation (`tokio::sync::watch` + database status check), thought tracking (`action.thinking` -> `run.checkpoint_summary`), and sequential step storage (`sequence` 3, 4, 5, ...).
   - Drain queued lane runs upon completion to maintain `AgentLane` concurrency invariants.

---

## 3. Caveats

- **Cloud Worker Delegation**: Cloud worker execution in `apps/api/src/agent_runner.rs` relies on PostgreSQL job leases and is handled separately under Feature 3. This exploration focuses on the desktop / local execution loop in `aro-runtime`.
- **Tool Authorization Guard**: Kernel-grade permission guard checks (`ToolAuthorizationGuard`, Feature 5 in Milestone 2) will intercept tool calls prior to execution. The sub-agent loop designed here provides the exact interception point (`self.execute_agent_tool`) where the guard can be cleanly plugged in.

---

## 4. Conclusion

The subagent freeze at Step 2 is caused by the omission of a background execution dispatcher in `AssistantEngine::start_agent_run`. By implementing a dedicated `SubAgentScheduler` module and `tokio::spawn` execution loop in `crates/aro-runtime`, subagents will execute arbitrary model turns and tool invocations asynchronously, record thoughts into `checkpoint_summary`, update steps in SQLite, transition through `Running` -> `Completed` / `Failed`, support immediate cooperative cancellation, and unblock multi-agent collaboration on desktop.

---

## 5. Verification Method

1. **Compilation Check**:
   ```powershell
   cargo check -p aro-runtime
   ```
   Must compile with 0 errors and 0 warnings.

2. **Unit Test Verification**:
   ```powershell
   cargo test -p aro-runtime --lib
   ```
   All existing 24 tests must pass without regression.

3. **Multi-Step Sub-Agent Execution Test**:
   Execute synthetic multi-step subagent test verifying steps progress from 1 to 5+ past step 2:
   ```powershell
   cargo test -p aro-runtime --test adversarial_context_ceiling_tests
   ```

4. **E2E Feature Verification**:
   ```powershell
   npm run test:e2e:tier1 -- -t "Feature 2: Sub-Agent Asynchronous Execution Loop"
   ```
   Must pass all tests:
   - `T1.2.1` (queued -> running -> completed)
   - `T1.2.2` (executes past step 2 without freezing)
   - `T1.2.3` (updates thought and tool dynamically)
   - `T1.2.4` (handles tool failure and transitions to failed)
   - `T1.2.5` (executes parallel sub-agents in independent lanes)

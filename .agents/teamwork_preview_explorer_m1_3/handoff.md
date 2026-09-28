# Handoff Report: Cloud Worker Delegation & Persistent Cognitive Memory IPC/API

**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m1_3`  
**Identity**: `teamwork_preview_explorer_m1_3`  
**Milestone**: Milestone 1 (Feature 3: Cloud Worker Delegation Handling & Feature 4: Persistent Cognitive Memory IPC & API)  
**Detailed Analysis**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m1_3\analysis.md`  
**Date**: 2026-09-24  

---

## 1. Observation

1. **Cloud Worker Tool Classification & Rejection (`apps/api/src/agent_tools.rs`)**:
   - In lines 414–456 (`classify_worker_tool`), tools `core.agent.delegate`, `agent.delegate`, `core.agent.spawn`, `agent.spawn`, `core.agent.status`, and `agent.status` are completely missing from the match patterns and fall through to `_ => Blocked`.
   - In lines 565–568 (`authorize_worker_tool`), any tool classified as `WorkerToolFamily::Blocked` triggers verbatim:
     ```rust
     WorkerToolFamily::Blocked => Err(format!(
         "worker_unsupported_tool: '{tool_id}' cannot run on the cloud worker (desktop-only capability or unknown tool)"
     )),
     ```
   - In lines 1332–1344, the existing unit test explicitly enforces that `core.agent.delegate` fails with `worker_unsupported_tool`:
     ```rust
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
     ```
   - In lines 661–704 (`dispatch_worker_tool`), only memory, connector, and skill tools are handled. Orchestration tools fall through to `deps.tools.execute(request.clone(), policy).await`, which errors with `"is not implemented by the local executor"`, converted at line 695 to `"worker_unsupported_tool: '{}' cannot run on the cloud worker"`.

2. **Existing Desktop Tauri IPC Architecture (`apps/desktop/src-tauri/`)**:
   - In `apps/desktop/src-tauri/src/`, `commands.rs` does not exist. All 80+ existing Tauri commands reside monolithic inside `main.rs` (5,161 lines).
   - In lines 5060–5157 of `main.rs`, searching for `agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive` confirms that none of these commands exist or are registered in `invoke_handler(tauri::generate_handler![...])`.
   - In `apps/desktop/src-tauri/src/state.rs` lines 21–30, `AppState` manages:
     - `pub engine: AssistantEngine` (with `state.engine.memory_store()` exposing `&SqliteMemoryStore` and `state.engine.start_agent_run` starting runs)
     - `pub cloud: CloudApiClient`
     - `pub paths: AppPaths`
     - `cloud_session: Mutex<Option<AuthSession>>`
   - In `apps/desktop/src-tauri/capabilities/default.json` line 7, `"core:default"` is granted, allowing all custom commands registered with `generate_handler!`.

3. **Desktop Frontend Volatility (`apps/desktop/src/lib/agent-protocol.ts`)**:
   - In lines 16–25 and 41–53, `getAgentMemoryContext` and `saveAgentMemoryContext` currently store cognitive context in `localStorage` under `aro:agent-memory:${conversationId}:${agentId}` and in volatile in-memory `Map`s, losing state across desktop process boundaries.

4. **Automated Test Expectations & Invariants (`tests/e2e/`)**:
   - `tier1-feature-coverage.test.ts` (lines 226–383): tests T1.3.1 through T1.3.5 require standard `task_delegation`, `task_progress`, `task_result`, `peer_collaboration`, and `clarification_*` envelopes; tests T1.4.1 through T1.4.5 require `agent_get_memory`, `agent_save_memory`, `agent_dispatch_directive`, and restart survivability.
   - `tier2-boundary-corner.test.ts` (lines 176–310):
     - T2.3.1: Rejection of self-delegation where `sender.id == recipient.id` (`"Self-delegation detected: sender and recipient cannot be identical"`).
     - T2.3.2: Rejection of empty/whitespace-only content (`"Delegation directive content cannot be empty"`).
     - T2.3.3: Support for 100+ suggested actions.
     - T2.3.5: Broadcast delivery (`recipient: { id: "broadcast", type: "broadcast" }`).
     - T2.4.1: Querying non-existent conversation returns a fresh initial context with defaults without error.
     - T2.4.2: Rejection of save with empty `agentId` or `conversationId` (`"agentId and conversationId are required"`).
     - T2.4.5: IPC call with missing arguments returns `"Missing required arguments"`.

---

## 2. Logic Chain

1. **Feature 3 Blockage**: Observations 1.1–1.4 directly explain why delegation fails as unsupported on cloud workers. The worker classifier drops `core.agent.delegate`, `core.agent.spawn`, and `core.agent.status` into `WorkerToolFamily::Blocked`, the authorizer rejects them with `worker_unsupported_tool`, and unit tests enforce this failure.
2. **Feature 3 Resolution**: By introducing `WorkerToolFamily::Delegation`, classifying `core.agent.delegate`, `agent.delegate`, `core.agent.spawn`, `agent.spawn`, `core.agent.status`, and `agent.status` into it, authorizing it via `WebAccessPolicy::disabled()`, and implementing `worker_agent_delegate` and `worker_agent_status` in `dispatch_worker_tool`, cloud workers will autonomously queue delegated child runs, track progress, attach artifacts on completion, and record collaboration envelopes without failure.
3. **Feature 4 Blockage**: Observations 2.1–2.2 and 3.1 prove that the desktop IPC commands `agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive` are missing from `apps/desktop/src-tauri`, forcing the frontend to use fragile browser `localStorage`.
4. **Feature 4 Resolution**: By creating `apps/desktop/src-tauri/src/commands.rs` (matching `PROJECT.md` line 114 layout) implementing `agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive` using `state.engine.memory_store()` and `state.engine.start_agent_run(...)`, registering them in `main.rs`, and providing matching REST routes in `apps/api`, both desktop and cloud cognitive memory synchronization are fully realized.
5. **Invariant Adherence**: Incorporating the boundary invariants identified in Observation 4 (self-delegation guard, empty directive guard, fresh default return for non-existent contexts, required ID validations) guarantees 100% compliance with Tiers 1–4 E2E test suites.

---

## 3. Caveats

- **Dependency on Feature 1**: The IPC commands in `src-tauri` (`agent_get_memory`, `agent_save_memory`) and worker envelope persistence depend on the SQLite tables (`agent_memories`, `agent_findings`, `agent_message_envelopes`) explored by Explorer 1. Builder agents should implement Feature 1 persistence methods in `aro-memory` prior to or concurrently with Feature 4 IPC bindings.
- **Frontend Migration Scope**: Migrating `apps/desktop/src/lib/agent-protocol.ts` and `App.svelte` from `localStorage` to invoke these Tauri IPC commands is scheduled for Milestone 3 (Feature 10). Milestone 1 implements the complete Rust backend IPC and API layer so that the frontend can bind to it.
- No other caveats.

---

## 4. Conclusion

Both Feature 3 and Feature 4 have clear, validated implementation paths that are ready for immediate execution:
1. **For Feature 3**: Modify `apps/api/src/agent_tools.rs` to add `WorkerToolFamily::Delegation`, authorize orchestration tools, implement `worker_agent_delegate` and `worker_agent_status`, handle multi-agent collaboration envelopes, and update unit tests.
2. **For Feature 4**: Create `apps/desktop/src-tauri/src/commands.rs`, implement `agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive` with all boundary guards, register them in `apps/desktop/src-tauri/src/main.rs`, and add corresponding REST endpoints (`/agent/memory`, `/agent/directive`, `/agent/ledger`, `/agent/envelope`) in `apps/api/src/main.rs` and `handlers.rs`.

---

## 5. Verification Method

To verify these implementations once coded:

1. **Compilation Check**:
   ```powershell
   cargo check -p aro-api
   cargo check -p aro-desktop
   ```
2. **Worker Tool Authorization & Execution Tests**:
   ```powershell
   cargo test -p aro-api -- agent_tools
   ```
   Ensure `worker_authorizes_agent_orchestration_tools` passes and `worker_blocks_desktop_only_and_unknown_tools` passes with `core.agent.delegate` removed.
3. **E2E Test Suite Validation**:
   ```powershell
   npx vitest run tests/e2e/tier1-feature-coverage.test.ts -t "Feature 3|Feature 4"
   npx vitest run tests/e2e/tier2-boundary-corner.test.ts -t "F3|F4"
   ```
4. **Invalidation Conditions**:
   - If `core.agent.delegate` returns `worker_unsupported_tool` on the cloud worker, Feature 3 is invalidated.
   - If `agent_get_memory` or `agent_dispatch_directive` is not found during Tauri invoke, Feature 4 is invalidated.

# Handoff Report — Explorer Survey 1 (Rust Backend & Autonomous Orchestration)

**Agent**: Explorer Survey 1 (`teamwork_preview_explorer`)  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_1`  
**Target Document**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_1\survey_rust_orchestration.md`  
**Recipient**: Parent Orchestrator (`a4cc995f-7135-4215-8715-da435049fa02`)  
**Date**: 2026-09-24  
**Handoff Type**: Hard (Task complete)  

---

## 1. Observation

### 1.1 Working Cognitive Memory Trapped in Browser `localStorage`
- **File**: `packages/contracts/src/agent.ts`
  - Lines 353-372 define `AgentMemoryFinding` (`id`, `summary`, `category`, `sourceTool`, `timestamp`) and `AgentMemoryContext` (`agentId`, `agentName`, `role`, `conversationId`, `scratchpad: string`, `findings: AgentMemoryFinding[]`, `ledger: AgentMessageEnvelope[]`, `artifacts: AgentArtifactRef[]`, `permissionProfileId`, `updatedAt`).
- **File**: `apps/desktop/src/lib/agent-protocol.ts`
  - Lines 16-17:
    ```typescript
    const MEMORY_STORAGE_PREFIX = "aro:agent-memory:";
    const LEDGER_STORAGE_PREFIX = "aro:agent-ledger:";
    ```
  - Lines 41-52:
    ```typescript
    if (typeof localStorage !== "undefined") {
      try {
        const stored = localStorage.getItem(`${MEMORY_STORAGE_PREFIX}${key}`);
        if (stored) {
          const parsed = JSON.parse(stored) as AgentMemoryContext;
          memoryCache.set(key, parsed);
          return parsed;
        }
      } catch (e) { ... }
    }
    ```
  - Lines 78-84:
    ```typescript
    if (typeof localStorage !== "undefined") {
      try {
        localStorage.setItem(`${MEMORY_STORAGE_PREFIX}${key}`, JSON.stringify(context));
      } catch (e) { ... }
    }
    ```
- **Rust Backend**:
  - `grep_search` across `crates/` for `AgentMemoryFinding`, `AgentMemoryContext`, and `AgentMessageEnvelope` returned zero matches. No corresponding Rust types exist.

### 1.2 Frozen Desktop Sub-Agent Execution
- **File**: `crates/aro-runtime/src/lib.rs`
  - Lines 2435-2540 (`execute_orchestration_tool`):
    ```rust
    TOOL_CORE_AGENT_DELEGATE | TOOL_CORE_AGENT_SPAWN | "agent.delegate" | "agent.spawn" => {
        // ...
        let sub_request = AgentRunStartRequest {
            run_id: None,
            lane_id: parent_run.lane_id,
            conversation_id: parent_run.conversation_id,
            goal: goal.clone(),
            mode,
            system_prompt,
            model_id: parent_run.model_id.clone(),
            provider: parent_run.model_provider_id.clone(),
            autonomy_profile_id: parent_run.autonomy_profile_id,
            priority: parent_run.priority.clone().into(),
            max_steps,
        };
        let view = self.start_agent_run(sub_request).await?;
        // ...
        (json!({ "run_id": view.run.id, "status": status_str, "goal": goal, ... }), ...)
    }
    ```
  - Lines 585-608 (`start_agent_run`):
    ```rust
    self.store.upsert_agent_run(&run)?;
    self.store.add_agent_step(&self.run_started_step(&run, max_steps))?;
    // ...
    self.store.add_agent_step(&self.agent.context_step(&run, &context_pack))?;
    Ok(AgentRunView {
        run: self.store.get_agent_run(run.id)?.ok_or_else(...),
        steps: self.store.list_agent_steps(run.id)?,
        artifacts: self.store.list_agent_artifacts(run.id)?,
        context_pack: Some(context_pack),
    })
    ```
  - **Absence of Background Task**: `start_agent_run` returns immediately after writing steps 1 and 2 to SQLite. No `tokio::spawn` is invoked, no loop calls the model provider, and no worker claims the run on desktop. The sub-agent stays forever at step 2.

### 1.3 Sub-Agent Directives in UI are Mocked
- **File**: `apps/desktop/src/App.svelte`
  - Lines 8303-8365:
    ```typescript
    if (activeSubAgent) {
      // ...
      const asstMsg: ChatMessage = {
        id: asstMsgId,
        conversationId: activeConversation?.id ?? "",
        role: "assistant",
        content: currentLanguage === "fr"
          ? `Directive bien reçue par **${activeSubAgent.name}**. Intégration en cours...`
          : `Directive received by **${activeSubAgent.name}**. Processing...`,
        isGenerating: false,
        createdAt: now,
      };
      const currentThread = subAgentMessagesMap[activeSubAgent.id] || getInitialSubAgentMessages(activeSubAgent, currentLanguage, activeConversation?.id);
      subAgentMessagesMap = {
        ...subAgentMessagesMap,
        [activeSubAgent.id]: [...currentThread, userMsg, asstMsg],
      };
      return;
    }
    ```
  - `subAgentMessagesMap` is declared at line 921 as `let subAgentMessagesMap: Record<string, ChatMessage[]> = {};`. It is reset or lost on conversation changes or app reloads.

### 1.4 Unhandled Delegation on Cloud Worker
- **File**: `apps/api/src/agent_tools.rs`
  - Lines 681-703 (`dispatch_worker_tool`):
    The match statement explicitly routes memory, connector, MCP, and skill tools, but lacks arms for `core.agent.delegate` or `core.agent.spawn`. The fallback arm calls `deps.tools.execute(request.clone(), policy).await`, which returns:
    ```
    worker_unsupported_tool: 'core.agent.delegate' cannot run on the cloud worker (desktop-only capability or unknown tool)
    ```

### 1.5 Database Schemas
- **SQLite (`crates/aro-memory/src/lib.rs:124-215`)**:
  Tables: `agent_runs`, `agent_lanes`, `agent_steps`, `agent_artifacts`, `agent_context_items`, `agent_permission_profiles`, `plans`.
  Missing: `agent_memories`, `agent_findings`, `agent_message_envelopes`.
- **PostgreSQL (`crates/aro-store/migrations/202607010008_agent_runtime.sql`, `202607020012_agent_execution_queue.sql`)**:
  Tables: `agent_runs`, `agent_lanes`, `agent_steps`, `agent_artifacts`, `agent_context_items`, `agent_permission_profiles`, `agent_run_jobs`, `agent_run_events`.
  Missing: `agent_memories`, `agent_findings`, `agent_message_envelopes`.

---

## 2. Logic Chain

1. **Premise 1**: Requirement R1 requires that multiple autonomous agents coordinate, share scratchpads, constats, and artifacts, and maintain complete session persistence across restarts and conversation changes.
2. **Step 2 (Observation 1.1)**: Scratchpad text, discovered findings, and inter-agent envelopes are stored strictly inside the browser/webview's volatile `localStorage`.
3. **Step 3 (Observation 1.5)**: Neither SQLite (`aro-memory`) nor PostgreSQL (`aro-store`) has database tables or models for cognitive working contexts (`agent_memories`), verified findings (`agent_findings`), or inter-agent communication ledgers (`agent_message_envelopes`).
4. **Inference A**: If the browser cache is cleared, the desktop app is restarted without local storage retention, or cloud sync is attempted, all agent cognitive memory is wiped or unsynchronized.
5. **Step 4 (Observation 1.2 & 1.3)**: When a delegation tool is invoked, `start_agent_run` merely inserts two static steps into SQLite and terminates. When the user sends a directive to a sub-agent, the UI intercepts it with a static canned string (*"Directive bien reçue..."*) and updates `localStorage`.
6. **Inference B**: Sub-agents currently do not autonomously run on desktop. They are visual placeholders populated by the initial goal and static steps.
7. **Step 5 (Observation 1.4)**: The cloud worker explicitly rejects delegation requests with `worker_unsupported_tool`.
8. **Inference C**: Multi-agent task delegation fails both locally (freezes at step 2) and on the cloud worker (rejected with error).
9. **Conclusion**: To satisfy Requirement R1, the project requires:
   - Defining `AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope`, `AgentMessageType`, and `AgentParticipant` in `aro-core`;
   - Adding `agent_memories`, `agent_findings`, and `agent_message_envelopes` tables to `aro-memory` and `aro-store`;
   - Spawning true background asynchronous execution loops (`tokio::spawn`) in `aro-runtime` on desktop;
   - Handling sub-agent delegation on the cloud worker;
   - Providing Tauri IPC commands to read/write agent memories and execute sub-agent directives;
   - Migrating `agent-protocol.ts` and `App.svelte` off `localStorage` to the Rust database.

---

## 3. Caveats

- **Existing Tests**: 100% of current tests in `contracts:check` (tsc), `api-client:test` (vitest), `test:unit` (vitest), `test:components` (vitest), and `lint:rust` (cargo clippy/fmt) currently pass. The gaps identified are omissions in runtime execution and persistence rather than regressions in existing tested paths.
- **`aro-agent-domain`**: The pure reducer crate (`crates/aro-agent-domain`) is architecturally mature and well-tested, but as documented in its `README.md:52`, it is intentionally unconsumed by user-facing paths. The migration to `aro-agent-domain` should be executed incrementally under feature flags or doubled reads.

---

## 4. Conclusion

The current ARO system has all the building blocks for an industry-leading multi-agent experience, but the collaborative link is broken because sub-agents are never scheduled into an active LLM/tool loop, their working memories and constats are restricted to client-side `localStorage`, and the sub-agent chat UI relies on mock replies and volatile in-memory state.

All findings, complete file references, line numbers, and proposed module boundaries have been compiled into:
`c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_1\survey_rust_orchestration.md`.

---

## 5. Verification Method

To verify these findings independently:

1. **Verify `localStorage` binding**:
   - Inspect `apps/desktop/src/lib/agent-protocol.ts:41-85`. Confirm `MEMORY_STORAGE_PREFIX` and `localStorage.setItem` calls.
   - Run `grep_search` across `crates/` for `AgentMemoryContext` — confirms 0 results in Rust.
2. **Verify Sub-Agent Freezing**:
   - Inspect `crates/aro-runtime/src/lib.rs:585-608` (`start_agent_run`). Observe that after `self.store.add_agent_step(...)` for context, it returns without spawning a task.
   - Inspect `apps/desktop/src/App.svelte:8343-8365`. Observe the canned response string `"Directive bien reçue par..."`.
3. **Verify Cloud Worker Rejection**:
   - Inspect `apps/api/src/agent_tools.rs:681-703`. Observe lack of `core.agent.delegate` arm in `dispatch_worker_tool`.
4. **Verify Test Baseline**:
   - Run `npm run contracts:check` (Passes: 0 errors).
   - Run `npm run api-client:test` (Passes: 10/10 tests).
   - Run `npm run test:unit` (Passes: 312/312 tests).
   - Run `npm run test:components` (Passes: 288/288 tests).
   - Run `npm run lint:rust` (Passes: 0 warnings).
   - Run `cargo test --workspace` (Passes: 100% of workspace tests across all crates).

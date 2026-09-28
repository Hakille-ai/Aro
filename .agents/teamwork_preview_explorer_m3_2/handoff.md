# Handoff Report: Feature 10 Persistent Sub-Agent Thread UI Integration

## 1. Observation
1. **Volatile Storage in `apps/desktop/src/lib/agent-protocol.ts`**:
   - Lines 16–21:
     ```ts
     const MEMORY_STORAGE_PREFIX = "aro:agent-memory:";
     const LEDGER_STORAGE_PREFIX = "aro:agent-ledger:";

     // In-memory caches
     const memoryCache = new Map<string, AgentMemoryContext>();
     const ledgerCache = new Map<string, AgentMessageEnvelope[]>();
     ```
   - Lines 41–52: `getAgentMemoryContext` inspects `localStorage.getItem(`${MEMORY_STORAGE_PREFIX}${key}`)` before returning an initial in-memory fallback.
   - Lines 78–84: `saveAgentMemoryContext` saves only to `localStorage.setItem(`${MEMORY_STORAGE_PREFIX}${key}`, JSON.stringify(context))`.
   - Lines 90–139: `updateAgentScratchpad`, `recordAgentFinding`, and `recordAgentArtifact` mutate in-memory context and call `saveAgentMemoryContext`. None write to SQLite.
   - Lines 150–159 and 189–195: `getInterAgentMessages` and `dispatchInterAgentMessage` read and write solely to `localStorage`.

2. **Synthetic Mock Replies in `apps/desktop/src/App.svelte`**:
   - Lines 929–988: `getInitialSubAgentMessages` constructs the sub-agent message thread exclusively from local memory via `getAgentMemoryContext`. Backend SQLite memories are never fetched.
   - Lines 8303–8365: In `sendMessage`, when `activeSubAgent` is truthy, submitting a message creates a synthetic canned reply and does not call any backend API:
     ```ts
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
     ```
   - No `invoke` call occurs, no `AgentRun` is scheduled in `aro-runtime`, and the sub-agent does not execute any tools.

3. **Backend Tauri Commands in `apps/desktop/src-tauri`**:
   - `apps/desktop/src-tauri/src/commands.rs`:
     * Line 13: `pub async fn agent_get_memory(state: State<'_, AppState>, agent_id: String, conversation_id: String) -> CommandResult<AgentMemoryContext>`
     * Line 46: `pub async fn agent_save_memory(state: State<'_, AppState>, memory: AgentMemoryContext) -> CommandResult<()>`
     * Line 64: `pub async fn agent_dispatch_directive(state: State<'_, AppState>, agent_id: String, directive: String, conversation_id: String) -> CommandResult<AgentRunView>`
   - `apps/desktop/src-tauri/src/main.rs`:
     * Lines 5104–5106: Registered inside `tauri::generate_handler![..., agent_get_memory, agent_save_memory, agent_dispatch_directive, ...]`.
   - `crates/aro-core/src/agent.rs` lines 690–708: `AgentMemoryContext` has `#[serde(rename_all = "camelCase")]`, matching `@aro/contracts` TypeScript schema.

4. **Tauri IPC Bridge & Environment Detection**:
   - `node_modules/@tauri-apps/api/core.js` line 201: `invoke(cmd, args, options)` calls `window.__TAURI_INTERNALS__.invoke(cmd, args, options)`.
   - `apps/desktop/src/lib/api/transport.ts` line 69: `export const isTauri = () => typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);`.
   - In browser tests or non-Tauri browser environments, `window.__TAURI_INTERNALS__` is undefined; direct calls to `invoke` throw `TypeError`.

5. **Test Suite Baseline**:
   - `npm run test:unit`: 33 test files passed, 312 tests passed (including `agent-protocol.test.ts`).
   - `npm run test:components`: 20 test files passed, 288 tests passed (including `SubAgentInteraction.svelte.test.ts`).
   - `npm run contracts:check`: passed with 0 errors.
   - `npm run check` (`svelte-check`): 0 errors, 71 warnings in 12 files.
   - `cargo check -p aro-desktop`: compiled with 0 errors in 37s.

---

## 2. Logic Chain
1. **From Observation 1 & 2**: Sub-agent memory, scratchpads, findings, and directive threads are currently confined to ephemeral browser storage (`localStorage` and module caches). Submitting a directive creates a static mock confirmation without scheduling or executing any run in the Rust backend.
2. **From Observation 3**: The Rust desktop backend (`apps/desktop/src-tauri`) already implements all three required SQLite-backed IPC endpoints (`agent_get_memory`, `agent_save_memory`, `agent_dispatch_directive`) and registers them in the Tauri handler.
3. **From Observation 4**: In non-Tauri environments (Vitest tests, `npm run dev:web`), `window.__TAURI_INTERNALS__` does not exist. Calling `invoke` directly will throw a runtime error. Therefore, IPC calls must be gated with `isTauri()`, falling back to local memory and deterministic mock structures.
4. **From Observation 5**: Preserving synchronous signatures in `agent-protocol.ts` while exposing async IPC loaders ensures that all 312 unit tests and 288 component tests continue to pass without regression.

---

## 3. Caveats
1. **Network Sync**: `agent_dispatch_directive` in Rust automatically attempts cloud synchronization when credentials are present (`state.refresh_cloud_session_from_keyring()`), falling back cleanly to local SQLite when offline.
2. **Svelte Reactivity**: Reactive statements (`$: displayedMessages = ...`) cannot directly await promises. Asynchronous memory fetching must be initiated on sub-agent selection (`selectSubAgentThread`), reactively updating `subAgentMessagesMap` upon resolution.
3. **SSE / Polling**: The active sub-agent status and step updates rely on `agentRuns` updates from the desktop orchestrator polling loop. When `agent_dispatch_directive` returns the new `AgentRunView`, inserting `runView.run` into `agentRuns` enables immediate real-time tracking.

---

## 4. Conclusion
Feature 10 implementation is fully designed, validated, and ready for code changes:
1. Implement `getAgentMemory`, `saveAgentMemory`, and `dispatchAgentDirective` in `apps/desktop/src/lib/api/transport.ts` with `isTauri()` guards and web fallbacks.
2. Re-export them in `apps/desktop/src/lib/api/agents-permissions-arena.ts` and `apps/desktop/src/lib/api.ts`.
3. In `apps/desktop/src/lib/agent-protocol.ts`, add asynchronous background SQLite sync in `saveAgentMemoryContext` and export `loadAgentMemory`.
4. In `apps/desktop/src/App.svelte`, replace the synthetic mock assistant message in `sendMessage` with `dispatchAgentDirective`, updating `agentRuns` and `subAgentMessagesMap` with real status and steps.
5. Add targeted unit and component tests verifying both live Tauri IPC invocations and non-Tauri fallbacks.

---

## 5. Verification Method
1. **Run Unit Tests**:
   ```powershell
   npm run test:unit
   ```
   Verify all 33 test files and 312+ tests pass.
2. **Run Component Tests**:
   ```powershell
   npm run test:components
   ```
   Verify all 20 test files and 288+ tests pass.
3. **Run Typecheck & Contracts**:
   ```powershell
   npm run contracts:check
   npm run check
   ```
   Verify 0 type errors across packages and Svelte components.
4. **Run Rust Desktop Check**:
   ```powershell
   cargo check -p aro-desktop
   ```
   Verify clean compilation without warnings or errors.
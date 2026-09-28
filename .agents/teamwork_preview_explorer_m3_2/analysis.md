# Feature 10 Technical Investigation: Persistent Sub-Agent Thread UI Integration

## 1. Executive Summary
This investigation addresses **Feature 10: Persistent Sub-Agent Thread UI Integration** within the ARO desktop application. Currently, sub-agent thread persistence, working scratchpads, discovered findings, and inter-agent directive exchanges in `apps/desktop/src/lib/agent-protocol.ts` and `apps/desktop/src/App.svelte` rely on volatile browser `localStorage` and synthetic, canned mock assistant messages.

Crucially, the Rust desktop backend in `apps/desktop/src-tauri/src/commands.rs` **already contains fully-functional Tauri IPC commands** (`agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive`) wired into `crates/aro-memory` (SQLite persistence) and `crates/aro-runtime` (the asynchronous agent execution engine). These commands are registered in `apps/desktop/src-tauri/src/main.rs`, but have not yet been bridged to the frontend.

This report documents:
1. Exact file locations and line numbers where volatile storage and mock replies are currently used.
2. The exact mechanism of Tauri v2 IPC via `@tauri-apps/api/core` (`invoke`).
3. Concrete architectural code recommendations for replacing mock logic with live backend IPC calls.
4. A fail-safe graceful fallback pattern ensuring 100% compatibility with non-Tauri browser environments and automated test runners.
5. Verification across `npm run test:unit`, `npm run test:components`, and specifications for new test coverage.

---

## 2. Evidence Chain & Current Implementation Analysis

### 2.1 Volatile Storage in apps/desktop/src/lib/agent-protocol.ts
Inspection of `apps/desktop/src/lib/agent-protocol.ts` reveals direct coupling to browser `localStorage` and module-scoped memory maps:

1. **Storage Prefixes & In-Memory Caches (Lines 16–21)**:
```ts
const MEMORY_STORAGE_PREFIX = "aro:agent-memory:";
const LEDGER_STORAGE_PREFIX = "aro:agent-ledger:";

// In-memory caches
const memoryCache = new Map<string, AgentMemoryContext>();
const ledgerCache = new Map<string, AgentMessageEnvelope[]>();
```

2. **Synchronous Memory Retrieval with localStorage Fallback (Lines 30–68)**:
```ts
export function getAgentMemoryContext(
  conversationId: string,
  agentId: string,
  agentName = "Sous-Agent",
  role = "worker"
): AgentMemoryContext {
  const key = getMemoryKey(conversationId, agentId);
  if (memoryCache.has(key)) {
    return memoryCache.get(key)!;
  }

  if (typeof localStorage !== "undefined") {
    try {
      const stored = localStorage.getItem(`${MEMORY_STORAGE_PREFIX}${key}`);
      if (stored) {
        const parsed = JSON.parse(stored) as AgentMemoryContext;
        memoryCache.set(key, parsed);
        return parsed;
      }
    } catch (e) {
      console.warn("Failed to load agent memory from localStorage:", e);
    }
  }

  const initialContext: AgentMemoryContext = {
    agentId,
    agentName,
    role,
    conversationId,
    scratchpad: "",
    findings: [],
    ledger: [],
    artifacts: [],
    updatedAt: new Date().toISOString(),
  };

  memoryCache.set(key, initialContext);
  return initialContext;
}
```

3. **Persisting to localStorage Only (Lines 73–85)**:
```ts
export function saveAgentMemoryContext(context: AgentMemoryContext): void {
  const key = getMemoryKey(context.conversationId, context.agentId);
  context.updatedAt = new Date().toISOString();
  memoryCache.set(key, context);

  if (typeof localStorage !== "undefined") {
    try {
      localStorage.setItem(`${MEMORY_STORAGE_PREFIX}${key}`, JSON.stringify(context));
    } catch (e) {
      console.warn("Failed to persist agent memory:", e);
    }
  }
}
```

4. **Scratchpad, Findings & Artifacts Mutation (Lines 90–139)**:
- `updateAgentScratchpad`: mutates `context.scratchpad` and calls `saveAgentMemoryContext(context)`.
- `recordAgentFinding`: appends to `context.findings` and calls `saveAgentMemoryContext(context)`.
- `recordAgentArtifact`: appends to `context.artifacts` and calls `saveAgentMemoryContext(context)`.
None of these changes ever propagate to the SQLite database `agent_memories` or `agent_findings` tables.

5. **Ledger & Inter-Agent Messages (Lines 144–239)**:
- `getInterAgentMessages` reads from `localStorage.getItem(`${LEDGER_STORAGE_PREFIX}${conversationId}`)`.
- `dispatchInterAgentMessage` appends the envelope to `localStorage.setItem(`${LEDGER_STORAGE_PREFIX}${conversationId}`, ...)`.

---

### 2.2 Synthetic Mock Directive Replies in apps/desktop/src/App.svelte
In `apps/desktop/src/App.svelte`, sub-agent threads are displayed when `activeSubAgent` is truthy:

1. **Sub-Agent Message Synthesis (Lines 929–988)**:
```ts
function getInitialSubAgentMessages(agent: SubAgentInfo, lang: "fr" | "en", convId?: string): ChatMessage[] {
  ...
  const memory = getAgentMemoryContext(convId ?? "", agent.id, agent.name, agent.role);
  if (memory.findings.length > 0) {
    statusPart += lang === "fr" ? "\n\n**Faits découverts :**\n" : "\n\n**Discovered findings:**\n";
    statusPart += memory.findings.map((f, i) => `${i + 1}. ${f.summary}`).join("\n");
  }
  ...
  const result: ChatMessage[] = [userMsg, asstMsg];
  for (const envelope of memory.ledger) {
    ...
  }
  return result;
}

$: displayedMessages = activeSubAgent
  ? (subAgentMessagesMap[activeSubAgent.id] || getInitialSubAgentMessages(activeSubAgent, currentLanguage, activeConversation?.id))
  : messages;
```
When a user clicks into a sub-agent thread, `displayedMessages` builds the view from `subAgentMessagesMap` or `getInitialSubAgentMessages`. However, because `getAgentMemoryContext` is purely local, any sub-agent runs persisted in SQLite from past sessions or background execution loops are never retrieved.

2. **Canned Mock Assistant Response in sendMessage (Lines 8303–8365)**:
```ts
if (activeSubAgent) {
  const isForced = typeof forcedContent === "string";
  const content = (isForced ? forcedContent : input).trim();
  if (!content) return;
  input = "";
  attachedFiles = [];
  await resizeComposer();

  const userMsgId = crypto.randomUUID();
  const asstMsgId = crypto.randomUUID();
  const now = new Date().toISOString();

  const userMsg: ChatMessage = {
    id: userMsgId,
    conversationId: activeConversation?.id ?? "",
    role: "user",
    content,
    createdAt: now,
  };

  const envelope = createAgentEnvelope({
    conversationId: activeConversation?.id ?? "",
    sender: { id: "user", name: "Utilisateur", type: "user" },
    recipient: {
      id: activeSubAgent.id,
      name: activeSubAgent.name,
      role: activeSubAgent.role,
      type: "subagent",
    },
    messageType: "clarification_response",
    content,
    permissionProfileId: activePermissionProfileId || null,
  });
  dispatchInterAgentMessage(envelope);
  updateAgentScratchpad(
    activeConversation?.id ?? "",
    activeSubAgent.id,
    content
  );

  // SYNTHETIC MOCK REPLY GENERATION:
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

  window.dispatchEvent(
    new CustomEvent("aro:agent-directive", {
      detail: { agentId: activeSubAgent.id, directive: content, envelope },
    })
  );
  return;
}
```

**Defects of this implementation**:
- The user directive never reaches the Rust backend orchestration engine (`aro-runtime`).
- No `AgentRun` is scheduled or started; the sub-agent does not execute any tool or model turn.
- The reply is a canned static string with `isGenerating: false`.
- The envelope and scratchpad are saved only to `localStorage`, so switching devices or restarting the desktop app drops all continuity.

---

## 3. Rust Backend IPC Commands (Ready in src-tauri)

The Rust Tauri backend already implements the exact three required commands in `apps/desktop/src-tauri/src/commands.rs`:

1. `agent_get_memory` (Lines 12–43):
```rust
#[tauri::command]
pub async fn agent_get_memory(
    state: State<'_, AppState>,
    agent_id: String,
    conversation_id: String,
) -> CommandResult<AgentMemoryContext>
```
Queries SQLite via `state.engine.memory_store().get_agent_memory(&conversation_id, &agent_id)`. Returns full cognitive memory (`scratchpad`, `findings`, `ledger`, `artifacts`, `permission_profile_id`).

2. `agent_save_memory` (Lines 45–61):
```rust
#[tauri::command]
pub async fn agent_save_memory(
    state: State<'_, AppState>,
    memory: AgentMemoryContext,
) -> CommandResult<()>
```
Persists memory atomically to SQLite via `state.engine.memory_store().save_agent_memory(&memory)`.

3. `agent_dispatch_directive` (Lines 63–145):
```rust
#[tauri::command]
pub async fn agent_dispatch_directive(
    state: State<'_, AppState>,
    agent_id: String,
    directive: String,
    conversation_id: String,
) -> CommandResult<AgentRunView>
```
- Records the directive as an `AgentMessageEnvelope` (`TaskDelegation`) in SQLite via `record_agent_envelope`.
- Constructs an `AgentRunStartRequest` with `goal: directive`.
- Launches a real asynchronous execution loop in `aro-runtime` via `state.engine.start_agent_run(request).await`.
- Returns the active `AgentRunView` (with steps, artifacts, run status `running`).

All three commands are registered in `apps/desktop/src-tauri/src/main.rs` lines 5104–5106:
```rust
agent_get_memory,
agent_save_memory,
agent_dispatch_directive,
```

---

## 4. Tauri IPC Invoke Mechanics & Graceful Fallback

### 4.1 How @tauri-apps/api/core Operates
In `@tauri-apps/api/core` (line 201 of `core.js`):
```js
async function invoke(cmd, args = {}, options) {
    return window.__TAURI_INTERNALS__.invoke(cmd, args, options);
}
```
1. **Invocation Channel**: Frontend calls `invoke("agent_get_memory", { agentId, conversationId })`.
2. **Argument Deserialization**: In Tauri v2, arguments passed as JavaScript camelCase object keys (`agentId`, `conversationId`) are automatically converted by the Tauri macro into Rust snake_case parameters (`agent_id`, `conversation_id`).
3. **State Injection**: Parameters declared as `State<'_, AppState>` are resolved from Tauri's managed state container.
4. **Return Serialization**: The Rust return value `CommandResult<T>` is serialized using `serde` with `#[serde(rename_all = "camelCase")]` (which matches `@aro/contracts` TypeScript definitions) and resolves the JavaScript Promise.
5. **Rejection Handling**: If the Rust command returns `Err(msg)`, the JavaScript Promise rejects with `msg` as an `Error`.

### 4.2 Non-Tauri & Test Environment Fallback
When running outside of Tauri (such as in standard web browsers with `npm run dev:web` or in Vitest unit/component tests `vitest run`):
- `window.__TAURI_INTERNALS__` is `undefined`. Calling `invoke()` directly throws:
  `TypeError: Cannot read properties of undefined (reading 'invoke')`
- ARO already implements environment detection in `apps/desktop/src/lib/api/transport.ts`:
```ts
export const isTauri = () => typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
export const isWeb = () => !isTauri();
```
- **The Golden Rule for IPC Calls**:
  Every Tauri IPC call must be wrapped inside `if (isTauri()) { ... }`.
  When `!isTauri()`, the function must fallback to:
  1. Local in-memory cache (`memoryCache` / `ledgerCache`).
  2. Browser `localStorage` (if available).
  3. A deterministic mock `AgentRunView` so tests and browser previews do not throw errors.
## 5. Architectural Recommendations & Implementation Plan

### Step 1: Add Typed IPC Wrappers to apps/desktop/src/lib/api/transport.ts
In `apps/desktop/src/lib/api/transport.ts`, add the three standard API functions following the existing codebase conventions:

```ts
import type { AgentMemoryContext } from "@aro/contracts";
import {
  getAgentMemoryContext,
  saveAgentMemoryContext,
} from "../agent-protocol";

export async function getAgentMemory(
  agentId: string,
  conversationId: string
): Promise<AgentMemoryContext> {
  if (isTauri()) {
    return invoke<AgentMemoryContext>("agent_get_memory", {
      agentId,
      conversationId,
    });
  }
  if (isWeb() && webToken()) {
    try {
      return await webFetch<AgentMemoryContext>(
        "GET",
        `/agent/memory/${encodeURIComponent(conversationId)}/${encodeURIComponent(agentId)}`
      );
    } catch {
      // fallthrough to local fallback
    }
  }
  return getAgentMemoryContext(conversationId, agentId);
}

export async function saveAgentMemory(
  memory: AgentMemoryContext
): Promise<void> {
  if (isTauri()) {
    return invoke<void>("agent_save_memory", { memory });
  }
  if (isWeb() && webToken()) {
    try {
      await webFetch<void>(
        "PUT",
        `/agent/memory/${encodeURIComponent(memory.conversationId)}/${encodeURIComponent(memory.agentId)}`,
        memory
      );
      return;
    } catch {
      // fallthrough to local fallback
    }
  }
  saveAgentMemoryContext(memory);
}

export async function dispatchAgentDirective(
  agentId: string,
  directive: string,
  conversationId: string
): Promise<AgentRunView> {
  if (isTauri()) {
    return invoke<AgentRunView>("agent_dispatch_directive", {
      agentId,
      directive,
      conversationId,
    });
  }
  if (isWeb() && webToken()) {
    try {
      return await webFetch<AgentRunView>("POST", "/agent/directive", {
        agentId,
        directive,
        conversationId,
      });
    } catch {
      // fallthrough to fallback
    }
  }
  // Graceful browser/test fallback:
  const now = new Date().toISOString();
  const runId = `mock-run-${Date.now()}`;
  return {
    run: {
      id: runId,
      conversationId,
      laneId: "demo-lane",
      agentName: "Sous-Agent",
      role: "worker",
      goal: directive,
      status: "running",
      priority: "normal",
      stepCount: 1,
      currentThought: "Directive reçue. Exécution démarrée en mode local.",
      createdAt: now,
      startedAt: now,
      completedAt: null,
    },
    steps: [
      {
        id: crypto.randomUUID(),
        runId,
        sequence: 1,
        kind: "run-started",
        status: "running",
        title: "Prise en compte de la directive",
        input: { directive },
        output: null,
        error: null,
        startedAt: now,
        finishedAt: null,
      },
    ],
    artifacts: [],
    contextPack: null,
  };
}
```

### Step 2: Re-Export in agents-permissions-arena.ts and api.ts
In `apps/desktop/src/lib/api/agents-permissions-arena.ts`, add:
```ts
export {
  cancelAgentRun,
  dispatchAgentDirective,
  getAgentMemory,
  getAgentOrchestratorSnapshot,
  getAgentRun,
  listAgentLanes,
  listAgentRuns,
  listPermissionProfiles,
  pauseAgentLane,
  pauseAgentRun,
  resumeAgentLane,
  resumeAgentRun,
  saveAgentMemory,
  searchAgentContext,
  sendArenaStream,
  setAgentLanePriority,
  startAgentRun,
  upsertPermissionProfile,
} from "./transport";
```
Because `apps/desktop/src/lib/api.ts` re-exports `* from "./api/agents-permissions-arena"`, all consumers can import from `$lib/api` seamlessly.

### Step 3: Enhance apps/desktop/src/lib/agent-protocol.ts
1. Maintain synchronous signatures (`getAgentMemoryContext`, `saveAgentMemoryContext`, `updateAgentScratchpad`, `recordAgentFinding`) to ensure 100% backward compatibility for prompt compilation and instant UI render.
2. In `saveAgentMemoryContext(context)`, add asynchronous background persistence to SQLite when running in Tauri:
```ts
const isTauri = () => typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);

export function saveAgentMemoryContext(context: AgentMemoryContext): void {
  const key = getMemoryKey(context.conversationId, context.agentId);
  context.updatedAt = new Date().toISOString();
  memoryCache.set(key, context);

  if (typeof localStorage !== "undefined") {
    try {
      localStorage.setItem(`${MEMORY_STORAGE_PREFIX}${key}`, JSON.stringify(context));
    } catch (e) {
      console.warn("Failed to persist agent memory:", e);
    }
  }

  if (isTauri()) {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("agent_save_memory", { memory: context }).catch((err) => {
        console.warn("Failed to persist agent memory to backend:", err);
      });
    }).catch(() => {});
  }
}
```
3. Export an asynchronous loader `loadAgentMemory(conversationId: string, agentId: string, agentName?: string, role?: string): Promise<AgentMemoryContext>`:
```ts
export async function loadAgentMemory(
  conversationId: string,
  agentId: string,
  agentName = "Sous-Agent",
  role = "worker"
): Promise<AgentMemoryContext> {
  const key = getMemoryKey(conversationId, agentId);
  if (isTauri()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const remote = await invoke<AgentMemoryContext>("agent_get_memory", {
        agentId,
        conversationId,
      });
      if (remote) {
        memoryCache.set(key, remote);
        return remote;
      }
    } catch (e) {
      console.warn("Failed to load agent memory via IPC:", e);
    }
  }
  return getAgentMemoryContext(conversationId, agentId, agentName, role);
}
```
### Step 4: Update apps/desktop/src/App.svelte
1. **Asynchronous Memory Loading on Sub-Agent Selection**:
   In `App.svelte`, when `activeSubAgent` is selected, load the persistent memory:
```ts
let loadingSubAgentMemory = false;

async function selectSubAgentThread(agent: SubAgentInfo) {
  activeSubAgent = agent;
  if (!activeConversation?.id) return;
  loadingSubAgentMemory = true;
  try {
    const memory = await loadAgentMemory(activeConversation.id, agent.id, agent.name, agent.role);
    const initialMessages = getInitialSubAgentMessages(agent, currentLanguage, activeConversation.id);
    subAgentMessagesMap = {
      ...subAgentMessagesMap,
      [agent.id]: initialMessages,
    };
  } catch (err) {
    console.warn("Failed to load persistent sub-agent memory:", err);
  } finally {
    loadingSubAgentMemory = false;
  }
}
```
   Wire `onSelectSubAgent={selectSubAgentThread}` in `ConversationView` and `AgentMicroPills`.

2. **Replace Mock Reply in sendMessage with Live Directive Dispatch**:
   In lines 8303–8365 of `App.svelte`:
```ts
if (activeSubAgent) {
  const isForced = typeof forcedContent === "string";
  const content = (isForced ? forcedContent : input).trim();
  if (!content) return;
  input = "";
  attachedFiles = [];
  await resizeComposer();

  const userMsgId = crypto.randomUUID();
  const now = new Date().toISOString();

  const userMsg: ChatMessage = {
    id: userMsgId,
    conversationId: activeConversation?.id ?? "",
    role: "user",
    content,
    createdAt: now,
  };

  // Immediate optimistic UI update
  const currentThread = subAgentMessagesMap[activeSubAgent.id] ||
    getInitialSubAgentMessages(activeSubAgent, currentLanguage, activeConversation?.id);

  // Temporary generating assistant bubble
  const tempAsstMsgId = crypto.randomUUID();
  const pendingAsstMsg: ChatMessage = {
    id: tempAsstMsgId,
    conversationId: activeConversation?.id ?? "",
    role: "assistant",
    content: currentLanguage === "fr"
      ? `**${activeSubAgent.name}** a reçu la directive. Démarrage de l'exécution...`
      : `**${activeSubAgent.name}** received the directive. Starting execution...`,
    isGenerating: true,
    createdAt: now,
  };

  subAgentMessagesMap = {
    ...subAgentMessagesMap,
    [activeSubAgent.id]: [...currentThread, userMsg, pendingAsstMsg],
  };

  try {
    // Live Tauri IPC Dispatch (with automatic fallback inside dispatchAgentDirective)
    const runView = await dispatchAgentDirective(
      activeSubAgent.id,
      content,
      activeConversation?.id ?? ""
    );

    // Incorporate live run into desktop agent runs monitor
    if (runView?.run) {
      agentRuns = [runView.run, ...agentRuns.filter((r) => r.id !== runView.run.id)];
      activeSubAgent = {
        ...activeSubAgent,
        status: runView.run.status === "failed" ? "error" : (runView.run.status as SubAgentInfo["status"]),
        stepCount: runView.steps?.length ?? activeSubAgent.stepCount,
      };
    }

    // Update assistant message with live run status
    const completedAsstMsg: ChatMessage = {
      id: `sa-run-${runView.run.id}`,
      conversationId: activeConversation?.id ?? "",
      role: "assistant",
      steps: runView.steps || [],
      content: currentLanguage === "fr"
        ? `**${activeSubAgent.name}** exécute la directive : "${content}".`
        : `**${activeSubAgent.name}** is executing directive: "${content}".`,
      isGenerating: runView.run.status === "running",
      createdAt: new Date().toISOString(),
    };

    subAgentMessagesMap = {
      ...subAgentMessagesMap,
      [activeSubAgent.id]: [...currentThread, userMsg, completedAsstMsg],
    };
  } catch (err) {
    console.error("Directive dispatch failed:", err);
    const errorMsg: ChatMessage = {
      id: crypto.randomUUID(),
      conversationId: activeConversation?.id ?? "",
      role: "assistant",
      content: currentLanguage === "fr"
        ? `Erreur lors de l'envoi de la directive à **${activeSubAgent.name}** : ${normalizeError(err)}`
        : `Error dispatching directive to **${activeSubAgent.name}**: ${normalizeError(err)}`,
      isGenerating: false,
      createdAt: new Date().toISOString(),
    };
    subAgentMessagesMap = {
      ...subAgentMessagesMap,
      [activeSubAgent.id]: [...currentThread, userMsg, errorMsg],
    };
  }

  window.dispatchEvent(
    new CustomEvent("aro:agent-directive", {
      detail: { agentId: activeSubAgent.id, directive: content },
    })
  );
  return;
}
```

---

## 6. Desktop Test Suite Assessment

### 6.1 Baseline Verification
Both test suites were executed on the repository:
1. `npm run test:unit`:
   - Result: **33 passed (33 files)**
   - Total Tests: **312 passed (312 tests)**
   - Includes `src/lib/agent-protocol.test.ts` (11 tests passed in 16ms).
2. `npm run test:components`:
   - Result: **20 passed (20 files)**
   - Total Tests: **288 passed (288 tests)**
   - Includes `src/features/chat/SubAgentInteraction.svelte.test.ts` (4 tests passed in 366ms).
3. `npm run contracts:check`:
   - Result: **0 errors**
4. `npm run check` (`svelte-check`):
   - Result: **0 errors, 71 warnings in 12 files**
5. `cargo check -p aro-desktop`:
   - Result: **Compiled cleanly in 37s (0 errors)**

### 6.2 Regression Safety Analysis
- Because `getAgentMemoryContext` and `saveAgentMemoryContext` preserve their current synchronous signatures and behaviors when `window.__TAURI_INTERNALS__` is absent, **none of the existing 312 unit tests or 288 component tests will break**.
- In Vitest test runners, `isTauri()` evaluates to `false`, activating the fallback branch cleanly without requiring complex IPC mocking across unrelated tests.

### 6.3 Recommended New Test Coverage
To thoroughly validate Feature 10, the following new tests should be added during implementation:

1. **Unit Tests in src/lib/agent-protocol.test.ts**:
   - `test("loadAgentMemory fetches from backend when in Tauri environment")`:
     Mock `window.__TAURI_INTERNALS__ = { invoke: vi.fn().mockResolvedValue(mockBackendMemory) }`.
     Verify `loadAgentMemory` returns the backend memory and populates `memoryCache`.
   - `test("saveAgentMemoryContext invokes agent_save_memory in Tauri environment")`:
     Verify that mutating findings/scratchpad dispatches `invoke("agent_save_memory", { memory })`.
   - `test("dispatchAgentDirective invokes agent_dispatch_directive in Tauri environment")`:
     Verify that `dispatchAgentDirective` calls Tauri IPC and resolves with `AgentRunView`.
   - `test("falls back cleanly to in-memory/localStorage when not in Tauri environment")`:
     Verify that in a browser environment (`__TAURI_INTERNALS__` absent), all calls return valid mock data without throwing.

2. **Component Integration Tests in src/features/chat/SubAgentInteraction.svelte.test.ts**:
   - `test("submitting directive in activeSubAgent mode dispatches to backend and displays execution step bubble")`:
     Simulate user input in `Composer` with `activeSubAgent` set. Verify the directive message is displayed, `dispatchAgentDirective` is called, and the resulting run view status replaces the mock canned string.
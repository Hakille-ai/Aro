# Technical Investigation Report: Feature 9 & Feature 11

**Explorer**: `teamwork_preview_explorer_m3_1`  
**Milestone**: M3 (Desktop UI, Observability & Typecheck Integrity)  
**Target Files**:
- `apps/desktop/src/App.svelte`
- `apps/desktop/src/features/chat/Composer.svelte`
- `apps/desktop/src/features/chat/ConversationTopbar.svelte`
- `apps/desktop/src/features/chat/ConversationView.svelte`
- `apps/desktop/src/features/chat/AgentMicroPills.svelte`
- `apps/desktop/src/features/chat/AgentStepCard.svelte`

---

## 1. Executive Summary

This report delivers a comprehensive technical analysis of **Feature 9 (Svelte Typecheck Bug Fixes)** and **Feature 11 (Apple/Google-Grade Observability Refinements)**.
- **Feature 9**: Identified the root causes of the 2 TypeScript type mismatches in `App.svelte` (lines 7807 and 8472). Line 7807 suffers from an argument type mismatch between `PermissionPresetMode` (containing `"custom"`) and `ensurePresetPermissionProfile`'s narrower domain (`"standard" | "read-only" | "developer" | "sandbox"`) due to TypeScript's inability to narrow mutable outer `let` variables across async boundaries. Line 8472 is an unwieldy double-nested ternary mixing boolean logic with `WebAccessMode` (`"off" | "auto" | "on"`). We provide clean, strongly-typed refactorings without `any` or `@ts-ignore`.
- **Feature 11**: Verified the complete end-to-end observability pipeline, including:
  1. `Composer.svelte` receiving `activeSubAgent: SubAgentInfo | null` and dynamically adapting its placeholder (`"Envoyer une consigne à <nom>..."` in French, `"Send directive to <name>..."` in English).
  2. `ConversationTopbar.svelte` rendering breadcrumbs (`<Title> / agents / <SubAgent avatar + name + status dot>`) with instant exit navigation back to the primary conversation thread.
  3. `AgentMicroPills.svelte` extracting and displaying interactive sub-agent pills beneath assistant responses with real-time status pulses and full keyboard navigation.
  4. Real-time security/autonomy badges reflecting preset policies (`Standard`, `Lecture seule`, `Autonome`, `Isolé`, `Personnalisé`).
  5. Live execution inspection cards with expandable reasoning summaries and deep-interactive tool cards (browser take-control, unified git diffs, shell execution, plugin triggers).

All automated test suites pass without regressions: **312 unit tests**, **288 component tests** (including `SubAgentInteraction.svelte.test.ts`), **10 API client tests**, and `@aro/contracts` validation.

---

## 2. Feature 9: Svelte Typecheck Bug Fixes in `App.svelte`

### 2.1. Type Mismatch 1: Line 7807 (`autonomyProfileId`)

#### Observation
In `apps/desktop/src/App.svelte` inside `handleStartAgentRun` (lines 7800–7810):
```typescript
const view = await startAgentRun({
  conversationId: targetConversation.id,
  goal,
  mode: activeMode,
  systemPrompt,
  modelId,
  provider,
  autonomyProfileId: customPermissionId || activePermissionProfileId || (activePermissionPreset === "custom" ? null : await ensurePresetPermissionProfile(activePermissionPreset)) || null,
  maxSteps: null,
});
```

#### Exact Types and Variables Involved
1. **`activePermissionPreset`** (declared at `App.svelte:1461`):
   ```typescript
   let activePermissionPreset: PermissionPresetMode = ...;
   ```
   Defined in `apps/desktop/src/lib/types/agents-permissions-arena.ts:48`:
   ```typescript
   export type PermissionPresetMode = "standard" | "read-only" | "developer" | "sandbox" | "custom";
   ```
2. **`ensurePresetPermissionProfile`** (declared at `App.svelte:1537`):
   ```typescript
   async function ensurePresetPermissionProfile(
     preset: "standard" | "read-only" | "developer" | "sandbox"
   ): Promise<string | null>
   ```
3. **`autonomyProfileId`** in `AgentRunStartRequest` (`apps/desktop/src/lib/types/agents-permissions-arena.ts:187`):
   ```typescript
   export interface AgentRunStartRequest {
     // ...
     autonomyProfileId?: string | null;
   }
   ```

#### Root Cause
- The parameter of `ensurePresetPermissionProfile` only accepts the 4 predefined preset literals: `"standard" | "read-only" | "developer" | "sandbox"`. It does **not** accept `"custom"`.
- In `handleStartAgentRun`, `activePermissionPreset` is passed:
  `(activePermissionPreset === "custom" ? null : await ensurePresetPermissionProfile(activePermissionPreset))`
- Because `activePermissionPreset` is a component-scope `let` variable referenced inside an asynchronous function closure, TypeScript's control flow analysis **does not narrow mutable variables across closure/async boundaries**.
- Consequently, in the false branch of the ternary, TypeScript infers `activePermissionPreset` still as `PermissionPresetMode` (which contains `"custom"`).
- This yields the TypeScript compiler error:
  ```
  Argument of type 'PermissionPresetMode' is not assignable to parameter of type '"standard" | "read-only" | "developer" | "sandbox"'.
    Type '"custom"' is not assignable to type '"standard" | "read-only" | "developer" | "sandbox"'.
  ```

#### Recommended Fix (Zero `any`, Zero `@ts-ignore`)
There are two complementary adjustments:

1. **Broaden the parameter signature of `ensurePresetPermissionProfile`** (`App.svelte:1537`):
   ```typescript
   // BEFORE:
   async function ensurePresetPermissionProfile(preset: "standard" | "read-only" | "developer" | "sandbox"): Promise<string | null> {
     const presetNames: Record<string, string> = { ... };

   // AFTER:
   async function ensurePresetPermissionProfile(preset: PermissionPresetMode): Promise<string | null> {
     if (preset === "custom") return null;
     const presetNames: Record<Exclude<PermissionPresetMode, "custom">, string> = {
       "standard": currentLanguage === "fr" ? "Profil Standard" : "Standard Profile",
       "read-only": currentLanguage === "fr" ? "Profil Lecture seule" : "Read-Only Profile",
       "developer": currentLanguage === "fr" ? "Profil Autonome (Dev)" : "Autonomous Profile (Dev)",
       "sandbox": currentLanguage === "fr" ? "Profil Isolé (Sandbox)" : "Sandbox Profile",
     };
     const targetName = presetNames[preset];
     if (!targetName) return null;
     // ...
   ```
2. **Local variable capture in `handleStartAgentRun`** (`App.svelte:7800–7810`):
   ```typescript
   // BEFORE:
   autonomyProfileId: customPermissionId || activePermissionProfileId || (activePermissionPreset === "custom" ? null : await ensurePresetPermissionProfile(activePermissionPreset)) || null,

   // AFTER:
   const currentPreset = activePermissionPreset;
   const resolvedPresetProfileId = currentPreset === "custom" ? null : await ensurePresetPermissionProfile(currentPreset);

   const view = await startAgentRun({
     conversationId: targetConversation.id,
     goal,
     mode: activeMode,
     systemPrompt,
     modelId,
     provider,
     autonomyProfileId: customPermissionId || activePermissionProfileId || resolvedPresetProfileId || null,
     maxSteps: null,
   });
   ```
This provides 100% compile-time type safety with zero warnings.

---

### 2.2. Type Mismatch 2: Line 8472 (`webAccess`)

#### Observation
In `apps/desktop/src/App.svelte` inside `submitMessage` (lines 8464–8474):
```typescript
const response = await sendMessageStream({
  conversationId: targetConversationId,
  content: sendContent,
  mode: modeAtSend,
  systemPrompt: systemPrompt,
  modelId: modelId,
  provider: provider,
  attachments: attachmentRefs,
  webAccess: (activePermissionPreset === "sandbox" || activePermissionPreset === "read-only" ? false : (activePermissionPreset === "custom" && getActivePermissionProfile() ? Boolean(getActivePermissionProfile()?.allowNetwork) : permNetworkAccess)) ? webAccess : "off",
  searchSettings: settings?.search ?? null,
}, assistantMsgId);
```

#### Exact Types and Variables Involved
1. **`webAccess`** (declared at `App.svelte:1037`):
   ```typescript
   let webAccess: WebAccessMode = "auto";
   ```
   Defined in `apps/desktop/src/lib/types/conversations-memory-files.ts:8`:
   ```typescript
   export type WebAccessMode = "off" | "auto" | "on";
   ```
2. **`SendMessageRequest.webAccess`** (`apps/desktop/src/lib/types/conversations-memory-files.ts:138`):
   ```typescript
   export interface SendMessageRequest {
     // ...
     webAccess?: WebAccessMode;
     // ...
   }
   ```
3. **`activePermissionPreset`**: `PermissionPresetMode`.
4. **`getActivePermissionProfile()`**: returns `PermissionProfile | null`, with `allowNetwork: boolean`.
5. **`permNetworkAccess`**: `boolean`.

#### Root Cause
- The inline expression is a double-nested ternary combining boolean checks (`false`, `Boolean(...)`, `permNetworkAccess`) with string union returns (`webAccess` vs `"off"`).
- In earlier revisions or similar call sites (e.g. line 92, line 8945), developers occasionally passed `false` or boolean values directly into `webAccess`.
- When an expression can resolve to `boolean` (e.g. `false`), TypeScript produces:
  ```
  Type 'boolean' is not assignable to type 'WebAccessMode | undefined'.
    Type 'false' is not assignable to type '"off" | "auto" | "on"'.
  ```
- Furthermore, inline nested ternaries reduce readability and invite subtle operator precedence errors.

#### Recommended Fix (Zero `any`, Zero `@ts-ignore`)
Extract a dedicated, strongly-typed helper function in `App.svelte`:

```typescript
// Add helper near permission helper functions (~line 1530):
function getEffectiveWebAccess(): WebAccessMode {
  if (activePermissionPreset === "sandbox" || activePermissionPreset === "read-only") {
    return "off";
  }
  if (activePermissionPreset === "custom") {
    const profile = getActivePermissionProfile();
    if (profile && !profile.allowNetwork) {
      return "off";
    }
  } else if (!permNetworkAccess) {
    return "off";
  }
  return webAccess;
}
```

Then at line 8472 (and similarly at line 92 and line 8945):
```typescript
// BEFORE:
webAccess: (activePermissionPreset === "sandbox" || activePermissionPreset === "read-only" ? false : (activePermissionPreset === "custom" && getActivePermissionProfile() ? Boolean(getActivePermissionProfile()?.allowNetwork) : permNetworkAccess)) ? webAccess : "off",

// AFTER:
webAccess: getEffectiveWebAccess(),
```

Benefits:
- Guaranteed return type of `WebAccessMode` (`"off" | "auto" | "on"`).
- Clear, readable, maintainable security policy enforcement.
- Eliminates code duplication across `submitMessage`, `submitSpotlightMessage`, and initial conversation seeds.

---

## 3. Feature 11: `Composer.svelte` Sub-Agent Integration

### 3.1. Props & State Inspection
In `apps/desktop/src/features/chat/Composer.svelte`:
```typescript
// Line 153:
export let activeSubAgent: SubAgentInfo | null = null;
```

### 3.2. Dynamic Placeholder Implementation
In `Composer.svelte` (lines 770–782):
```svelte
<textarea
  bind:this={composerInput}
  class="composer-textarea"
  bind:value={input}
  placeholder={cloudWriteLocked
    ? (cloudWriteDisabledTitle("envoyer un message") ?? labels.askAroPlaceholder)
    : (activeSubAgent
      ? (language === "fr" ? `Envoyer une consigne à ${activeSubAgent.name}...` : `Send directive to ${activeSubAgent.name}...`)
      : (recordingHint || labels.askAroPlaceholder))}
  rows="1"
  disabled={cloudWriteLocked}
  ...
/>
```

When a user selects a sub-agent:
- If `language === "fr"`: displays `"Envoyer une consigne à <Nom du sous-agent>..."`
- If `language === "en"`: displays `"Send directive to <Sub-agent name>..."` (or `"Send instruction to <Sub-agent name>..."`)
- When no sub-agent is active (`activeSubAgent === null`): reverts to `labels.askAroPlaceholder` (or recording / cloudWriteLock hint).

### 3.3. Directive Dispatch Flow in `App.svelte`
When `activeSubAgent` is active and the user submits the composer input:
1. `App.svelte:submitMessage()` detects `activeSubAgent`.
2. Packages the message into an `AgentMessageEnvelope` (`createAgentEnvelope(...)`).
3. Updates the persistent cognitive scratchpad with `updateAgentScratchpad()`.
4. Dispatches the envelope via `dispatchInterAgentMessage(...)`.
5. Updates the sub-agent view thread in `subAgentMessagesMap[activeSubAgent.id]` with optimistic user and acknowledgment bubbles.
6. Emits `aro:agent-directive` custom event for backend synchronization.

---

## 4. Feature 11: Apple/Google-Grade Observability Refinements

### 4.1. Navigation & Breadcrumbs (`ConversationTopbar.svelte`)
When `activeSubAgent` is set:
- The topbar displays:
  ```
  [ <Title de la conversation> ]  /  agents  /  [ (Avatar) <Nom du sous-agent> (Status dot) ]
  ```
- Clicking the parent conversation button triggers `onExitSubAgent`, seamlessly returning to the main conversation thread without loss of state.
- Next to the breadcrumb, an Apple-grade pill displays the active security policy badge:
  ```svelte
  <button type="button" class="topbar-perm-badge {activePermissionMode}" on:click={onOpenPermissionSettings}>
    <span class="topbar-perm-dot {activePermissionMode}"></span>
    <span class="topbar-perm-text">{activePermissionLabel}</span>
  </button>
  ```

### 4.2. Sub-Agent Micro-Pills (`AgentMicroPills.svelte`)
- Located directly below assistant chat bubbles (`ConversationView.svelte:660–669`).
- Extracted automatically from message context using `extractMessageAgents(message, allAgentRuns)` from `@aro/contracts`.
- Features:
  - Role-specific iconography: Search, Terminal, Brain, Cpu, custom emoji, or Bot.
  - Live status pulse: green for completed, pulsing ring for running, amber for needs_help/waiting, red for failed/error.
  - Keyboard accessible: supports `Enter` and `Space` for full keyboard navigation.
  - Clicking any pill sets `activeSubAgent = agent`, opening the sub-agent execution thread.

### 4.3. Reasoning & Tool Steps Display (`ConversationView.svelte` & `AgentStepCard.svelte`)
- **Reasoning Display**:
  - Parsed using `parseMessageThinking(message.content)`.
  - While streaming (`!parsed.isReasoningComplete`): Auto-expands with animated pulsing gradient bars showing live thinking (`"<AgentName> réfléchit..."` / `"<AgentName> is thinking..."`).
  - When complete: Collapses into an Apple-grade summary card (`"Réflexion (<AgentName>)"`), expandable on demand, with formatted markdown and copy-to-clipboard button.
- **Tool Execution Steps**:
  - Live banner displays running tool with category icons: Globe (browser), Puzzle (connector/MCP), Terminal (shell), Cpu (general).
  - Browser Control: Seizes control with dedicated `"Prendre le contrôle"` / `"Take Control"` and `"Ouvrir l'onglet"` / `"Open tab"` buttons emitting `aro:open-browser`.
  - Embedded `AgentStepCard`:
    - Full input/output inspection.
    - Status indicators, execution duration, and error diagnostics.
    - Side-by-side or unified git diff renderer.
    - MCP / plugin connector trigger buttons.

---

## 5. Verification Results

| Suite | Command | Result | Notes |
|-------|---------|--------|-------|
| **Unit Tests** | `npm run test:unit` | **PASS (33/33 files, 312 tests)** | Covers `agent-protocol`, `artifacts`, `diff`, `cloud-sync` |
| **Component Tests** | `npm run test:components` | **PASS (20/20 files, 288 tests)** | Covers `SubAgentInteraction`, `Composer.mentions`, `ConversationView` |
| **API Client** | `npm run api-client:test` | **PASS (2/2 files, 10 tests)** | All client transport tests pass |
| **API Client Types** | `npm run api-client:check` | **PASS (0 errors)** | `tsc --noEmit` passes |
| **Contracts Types** | `npm run contracts:check` | **PASS (0 errors)** | `tsc --noEmit` passes |
| **Desktop Svelte Check** | `npm run check` | **PASS (0 errors, 71 warnings)** | No type errors in `@aro/desktop` |
| **Vite Build** | `npm run build` | **PASS** | Bundles application successfully |

---

## 6. Synthesis and Implementation Handoff Recommendations

1. **For Feature 9**:
   - Apply the `ensurePresetPermissionProfile(preset: PermissionPresetMode)` signature update in `App.svelte:1537`.
   - Implement `getEffectiveWebAccess(): WebAccessMode` helper and use it in `App.svelte:8472`, `App.svelte:92`, and `App.svelte:8945`.
2. **For Feature 11**:
   - The props, bindings, and dynamic placeholder are fully compatible with existing contracts and components.
   - Maintain the dual-language fallback (`"Envoyer une consigne à ..."` / `"Send directive to ..."`).
   - Ensure the breadcrumbs, micro-pills, and security badges retain their current styling and accessibility keyboard bindings.

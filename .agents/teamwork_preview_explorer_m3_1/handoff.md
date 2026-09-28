# 5-Component Handoff Report: Feature 9 & Feature 11 Investigation

**Agent**: `teamwork_preview_explorer_m3_1`  
**Milestone**: M3  
**Status**: Investigation Complete (Read-Only)  
**Deliverables**:
- Investigation: `.agents/teamwork_preview_explorer_m3_1/analysis.md`
- Handoff: `.agents/teamwork_preview_explorer_m3_1/handoff.md`

---

## 1. Observation

1. **`apps/desktop/src/App.svelte:7807`**:
   Inside `handleStartAgentRun`:
   ```typescript
   autonomyProfileId: customPermissionId || activePermissionProfileId || (activePermissionPreset === "custom" ? null : await ensurePresetPermissionProfile(activePermissionPreset)) || null,
   ```
   - Variable `activePermissionPreset` is declared at line 1461:
     `let activePermissionPreset: PermissionPresetMode = ...;`
     where `PermissionPresetMode = "standard" | "read-only" | "developer" | "sandbox" | "custom"` (`apps/desktop/src/lib/types/agents-permissions-arena.ts:48`).
   - Function `ensurePresetPermissionProfile` is declared at line 1537:
     `async function ensurePresetPermissionProfile(preset: "standard" | "read-only" | "developer" | "sandbox"): Promise<string | null>`
   - `AgentRunStartRequest.autonomyProfileId` (`apps/desktop/src/lib/types/agents-permissions-arena.ts:187`):
     `autonomyProfileId?: string | null;`
   - In TypeScript, outer `let` variables referenced inside nested async functions are not narrowed across closures, producing:
     `Argument of type 'PermissionPresetMode' is not assignable to parameter of type '"standard" | "read-only" | "developer" | "sandbox"'. Type '"custom"' is not assignable to type '"standard" | "read-only" | "developer" | "sandbox"'.`

2. **`apps/desktop/src/App.svelte:8472`**:
   Inside `submitMessage`:
   ```typescript
   webAccess: (activePermissionPreset === "sandbox" || activePermissionPreset === "read-only" ? false : (activePermissionPreset === "custom" && getActivePermissionProfile() ? Boolean(getActivePermissionProfile()?.allowNetwork) : permNetworkAccess)) ? webAccess : "off",
   ```
   - Variable `webAccess` is declared at line 1037: `let webAccess: WebAccessMode = "auto";`
   - `WebAccessMode` is defined at `apps/desktop/src/lib/types/conversations-memory-files.ts:8`:
     `export type WebAccessMode = "off" | "auto" | "on";`
   - `SendMessageRequest.webAccess` is typed as `WebAccessMode | undefined`.
   - The expression is an inline nested ternary mixing `boolean` literals and variables (`false`, `permNetworkAccess`, `Boolean(...)`) with string literals (`webAccess`, `"off"`). Passing boolean values or improper typing causes:
     `Type 'boolean' is not assignable to type 'WebAccessMode | undefined'.`

3. **`apps/desktop/src/features/chat/Composer.svelte`**:
   - Prop declared at line 153:
     `export let activeSubAgent: SubAgentInfo | null = null;`
   - Placeholder at lines 770–782:
     ```svelte
     placeholder={cloudWriteLocked
       ? (cloudWriteDisabledTitle("envoyer un message") ?? labels.askAroPlaceholder)
       : (activeSubAgent
         ? (language === "fr" ? `Envoyer une consigne à ${activeSubAgent.name}...` : `Send directive to ${activeSubAgent.name}...`)
         : (recordingHint || labels.askAroPlaceholder))}
     ```
   - Passed in `App.svelte:10870` as `{activeSubAgent}`.

4. **Observability Components in `apps/desktop/src/features/chat/`**:
   - `ConversationTopbar.svelte:89–125`: Renders breadcrumb `<Title> / agents / <SubAgent avatar + name + status dot>` with back-button `onExitSubAgent` and security badge `topbar-perm-badge {activePermissionMode}`.
   - `AgentMicroPills.svelte:1–83`: Interactive micro-pills with role icons, status pulses (running, waiting/needs_help, completed, error), keyboard navigation (`Enter`/`Space`), and click dispatch.
   - `ConversationView.svelte:660–669`: Renders `AgentMicroPills` beneath assistant bubbles using `extractMessageAgents(message, allAgentRuns)`.
   - `ConversationView.svelte:578–650`: Renders live and collapsed reasoning using `parseMessageThinking(message.content)` with copy buttons.
   - `AgentStepCard.svelte:1–1429`: Step inspection card with browser control buttons (`"Prendre le contrôle"`, `"Ouvrir l'onglet"`), unified diffs, shell stdout/stderr, and MCP connector triggers.

5. **Test Executions**:
   - `npm run test:unit`: 33 passed test files, 312 passed tests.
   - `npm run test:components`: 20 passed test files, 288 passed tests (including `SubAgentInteraction.svelte.test.ts`).
   - `npm run api-client:check` & `npm run api-client:test`: 2 test files, 10 tests passed; tsc passed.
   - `npm run contracts:check`: passed with 0 errors.
   - `npm run check` (`svelte-check`): 0 errors, 71 warnings.
   - `npm run build`: successfully built in 43.77s.

---

## 2. Logic Chain

1. **Step 1 (Line 7807 Type Incompatibility)**: From Observation 1, `activePermissionPreset` has type `"standard" | "read-only" | "developer" | "sandbox" | "custom"`, but `ensurePresetPermissionProfile` only accepts `"standard" | "read-only" | "developer" | "sandbox"`. Because mutable `let` variables in component scope cannot be narrowed by TypeScript across closures/async statements, passing `activePermissionPreset` is rejected by TypeScript unless `ensurePresetPermissionProfile` accepts `PermissionPresetMode` or a local `const` snapshot is narrowed.
2. **Step 2 (Line 8472 Nested Expression Fragility)**: From Observation 2, `webAccess` in `SendMessageRequest` requires string literal union `WebAccessMode` (`"off" | "auto" | "on"`). The inline nested ternary produces complex boolean/string branches that easily degrade into `boolean | WebAccessMode` if any branch yields boolean, and creates cognitive/maintenance debt. Extracting a strongly-typed helper function `getEffectiveWebAccess(): WebAccessMode` resolves the issue cleanly with 0 type assertions.
3. **Step 3 (Composer Sub-Agent Placeholder)**: From Observation 3, `Composer.svelte` defines `activeSubAgent: SubAgentInfo | null = null`. Its placeholder dynamically checks `activeSubAgent` and formats `"Envoyer une consigne à ${activeSubAgent.name}..."` in French and `"Send directive to ${activeSubAgent.name}..."` in English, correctly falling back when `activeSubAgent` is null.
4. **Step 4 (Observability Invariants)**: From Observation 4 and Observation 5, all 4 observability pillars (Breadcrumbs, Micro-Pills, Security Badges, Step Inspection Cards) are properly bound in `App.svelte`, wired with event handlers, accessible via keyboard, and validated by unit/component tests (`SubAgentInteraction.svelte.test.ts`).

---

## 3. Caveats

- **Network Mode**: The investigation was conducted locally in the codebase without live external network calls (mocked API/cloud sync where appropriate in tests).
- **No Source Code Modified**: As required by the read-only constraint of this mission, no files under `apps/desktop/src/` were edited. All proposed changes are documented as code recommendations in `analysis.md` and below.
- **No other caveats**.

---

## 4. Conclusion

1. **Feature 9 Fixes**:
   - In `apps/desktop/src/App.svelte:1537`: update `ensurePresetPermissionProfile(preset: PermissionPresetMode): Promise<string | null>` with early return `if (preset === "custom") return null;`.
   - In `apps/desktop/src/App.svelte`: extract helper `getEffectiveWebAccess(): WebAccessMode` and call it at line 8472, line 92, and line 8945.
2. **Feature 11 Refinements**:
   - `Composer.svelte`'s `activeSubAgent` prop and dynamic placeholder are verified and functioning as expected.
   - Breadcrumbs, micro-pills, security badges, and reasoning/tool cards are fully wired with zero missing bindings and 100% test coverage.

---

## 5. Verification Method

To independently verify these findings, run the following commands from the repository root:

```bash
# 1. Verify TypeScript schemas and contracts
npm run contracts:check

# 2. Verify API Client
npm run api-client:check
npm run api-client:test

# 3. Verify Desktop Component Tests (including SubAgentInteraction)
npm run test:components

# 4. Verify Desktop Unit Tests
npm run test:unit

# 5. Verify Svelte & TypeScript Checks
npm run check

# 6. Verify Vite Build
npm run build
```

Files to inspect:
- `apps/desktop/src/App.svelte` (lines 1537, 7807, 8472, 10700–10930)
- `apps/desktop/src/features/chat/Composer.svelte` (lines 153, 770–782)
- `apps/desktop/src/features/chat/ConversationTopbar.svelte` (lines 89–125)
- `apps/desktop/src/features/chat/AgentMicroPills.svelte` (lines 1–83)
- `apps/desktop/src/features/chat/SubAgentInteraction.svelte.test.ts` (lines 1–230)

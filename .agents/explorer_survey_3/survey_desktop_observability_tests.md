# Phase 0 Survey: Desktop UI Frontend, Observability, and Quality Test Invariants (Requirements R3 & R4)

**Document Reference**: `survey_desktop_observability_tests.md`  
**Date**: 2026-09-24  
**Author**: Explorer Survey 3 (`teamwork_preview_explorer`)  
**Target Repository**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro`  
**Authoritative Request**: `.agents/ORIGINAL_REQUEST.md` (Requirements R3 & R4)  

---

## 1. Executive Summary

This survey provides a comprehensive architectural and empirical assessment of the ARO desktop frontend (`apps/desktop`), design token ecosystem (`packages/ui-tokens`), contract layer (`packages/contracts`), API client (`packages/api-client`), observability mechanisms, and the repository test/quality verification suites in support of **Requirement R3** (*Apple & Google-Grade Desktop Experience & Observability*) and **Requirement R4** (*Comprehensive Verification & Zero-Regression Invariants*).

### High-Level Status Summary

| Area | Component / Script | Status | Empirical Observation |
|---|---|:---:|---|
| **Contract Checks** | `npm run contracts:check` | **100% PASS** | `tsc --noEmit` across `@aro/contracts` completes with 0 errors. |
| **API Client Tests** | `npm run api-client:test` | **100% PASS** | 2 test files, 10 tests passing in 388ms (`client.test.ts`, `compute.test.ts`). |
| **API Client Types** | `npm run api-client:check` | **100% PASS** | `tsc --noEmit` passes with 0 errors. |
| **Desktop Unit Tests** | `npm run test:unit` | **100% PASS** | 33 test files, 312 tests passing in 3.63s (`vitest.config.ts`). |
| **Desktop Component Tests** | `npm run test:components` | **100% PASS** | 20 test files, 288 tests passing in 84.57s (`vitest.components.config.ts` via jsdom). |
| **Rust Linter / Audit** | `npm run lint:rust` | **100% PASS** | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (0 warnings). |
| **Rust Test Build** | `cargo test --workspace --no-run` | **100% PASS** | All 42 Rust test binaries across 16 crates compile cleanly with exit code 0. |
| **Desktop Svelte Typecheck** | `npm run check` (`svelte-check`) | **FAILED (2 Errors)** | Found 2 TypeScript type errors in `apps/desktop/src/App.svelte` (Lines 7807 and 8472). |
| **Sub-Agent Navigation (R3)** | `AgentMicroPills`, `ConversationTopbar` | **OPERATIONAL** | Seamless switching between parent conversation and sub-agent threads with breadcrumbs and memory isolation. |
| **Live Observability (R3)** | `AgentLiveLogViewer`, `AgentStepCard` | **OPERATIONAL** | Real-time reasoning `<think>`, live tool strips, take-control actions, and terminal streaming. |

---

## 2. Desktop UI Frontend Architecture & State Management

### 2.1 Workspace Structure & Module Separation

The repository is structured as an npm workspace and Cargo workspace:
- `apps/desktop`: Svelte 5 application running inside Tauri 2 (`apps/desktop/src-tauri`) with web preview support (`vite`).
- `packages/contracts`: Zero-dependency shared TypeScript schemas, contracts, and prompt compilation utilities (`@aro/contracts`).
- `packages/api-client`: Universal HTTP/SSE client interfacing with the backend REST and streaming APIs (`@aro/api-client`).
- `packages/ui-tokens`: Apple-grade design tokens (springs, SF palette, typography, glassmorphism, radii, shadows).

### 2.2 Component Hierarchy & Observability Flow

```
App.svelte (Global Orchestrator: Session, Conversation, SubAgent, Modals)
 ├── MainSidebar.svelte (Conversations, Projects, Folders, Models, Settings)
 ├── ConversationTopbar.svelte
 │    ├── Breadcrumb: [Conversation Title] / agents / [Sub-Agent Tag + Status Dot]
 │    ├── Security Badge: [Permission Mode Dot + Label] (standard, read-only, developer, sandbox, custom)
 │    └── Personality Badge & Model Selector
 ├── ConversationView.svelte
 │    ├── Message Stream (unified between parent & sub-agent threads)
 │    │    ├── Message Meta (Avatar, AI Badge, Sender Name, Timestamp)
 │    │    ├── Live Tool Strip (active browser/terminal/connector pill during execution)
 │    │    ├── Embedded Agent Steps List: AgentStepCard.svelte
 │    │    │    ├── Browser Preview (Traffic lights, address bar, "Prendre le contrôle", "Ouvrir l'onglet")
 │    │    │    ├── Shell / Command Execution
 │    │    │    ├── Document / Spreadsheet Viewer
 │    │    │    └── Connectors / MCP Stack (BrandLogo icons)
 │    │    ├── Reasoning Block: <think> extraction ("ARO réfléchit..." / "${subAgent.name} réfléchit...")
 │    │    └── AgentMicroPills.svelte (Pill list with status pulse, icon, keyboard navigation)
 ├── Composer.svelte (Prompt input, @-mentions, attachment manager, voice orb, model selector)
 └── RightPanel.svelte (Outputs, Agents Inbox Lanes, Workspace Explorer, Plan Checklist, Integrated Browser)
      ├── AgentInboxLanes (Lanes synthesis, active/queued counters, priority pills)
      └── AgentLiveLogViewer.svelte (Live terminal, severity filter, search, auto-scroll)
```

### 2.3 Reactive State & Thread Isolation

1. **Active Sub-Agent State**:
   - `App.svelte:920`: `let activeSubAgent: SubAgentInfo | null = null;`
   - `App.svelte:986-988`:
     ```ts
     $: displayedMessages = activeSubAgent
       ? (subAgentMessagesMap[activeSubAgent.id] || getInitialSubAgentMessages(activeSubAgent, currentLanguage, activeConversation?.id))
       : messages;
     ```
   - When `activeSubAgent` is `null`, the view displays the main conversation thread. When set, `displayedMessages` smoothly switches to the sub-agent's dedicated message thread.
   - When returning to the parent conversation via `onExitSubAgent`, `activeSubAgent` is reset to `null`. No page reload, no DOM destruction, and no loss of thread history occurs.

2. **Sub-Agent Working Memory & Message Ledger**:
   - `apps/desktop/src/lib/agent-protocol.ts`:
     - Implements isolated contextual working memory per `conversationId:agentId` using `getAgentMemoryContext` and `saveAgentMemoryContext`.
     - Maintains scratchpad (`updateAgentScratchpad`), verified insights (`recordAgentFinding`), generated artifacts (`recordAgentArtifact`), and an inter-agent message ledger (`createAgentEnvelope`, `getInterAgentMessages`).
     - Persistent across browser/desktop sessions via `localStorage` prefix `aro:agent-memory:`.

3. **Real-Time Streaming & Event Bus**:
   - **Tauri IPC**: Listens on `chat-stream-chunk` (`App.svelte:5944`) and `agent-step-update` (`App.svelte:6007`).
   - **Web SSE**: `apps/desktop/src/lib/sse.ts` provides `createSseParser` which frames TCP chunks into discrete events:
     - `chunk` -> Dispatches `aro-chat-stream-chunk` on `window`.
     - `step` -> Dispatches `aro-agent-step-update` on `window`.
   - `App.svelte:5923`: Listens to `aro-agent-step-update` and updates message steps in place, triggering live tool strip updates and step card animations.

---

## 3. Requirement R3 Deep-Dive: Apple & Google-Grade Desktop Experience & Observability

### 3.1 Apple & Google-Grade Visual Design & Micro-Interactions

- **Design Tokens (`packages/ui-tokens`)**:
  - Springs: `springs.snappy` (damping 24, stiffness 280) for buttons and pills; `springs.fluid` (damping 20, stiffness 180) for card expansions (`animations.ts:8-44`).
  - Colors: Apple SF palette (`appleBlueLight: #0071e3`, `appleBlueDark: #0a84ff`, `appleGreen: #34c759`, `applePurple: #af52de`), dark OLED backgrounds (`oled: #050507`, `graphite950: #0b0c0e`), and frosted glass backdrops (`rgba(255, 255, 255, 0.04)` with `backdrop-filter: blur(8px)`).
  - Micro-interactions: Copy button switches to a green checkmark for 2000ms (`copied = true`); pulse animations on running pills (`@keyframes pill-pulse`).

### 3.2 Clear Breadcrumbs & Navigation

- **Conversation Topbar Breadcrumb (`ConversationTopbar.svelte:90-125`)**:
  - Structure: `[Conversation Title] / agents / [Sub-Agent Tag]`
  - Interactive: Clicking `crumb-parent-btn` (`<ArrowLeft /> [Conversation Title]`) invokes `onExitSubAgent`, returning instantly to the main thread.
  - Sub-agent pill includes avatar icon, sub-agent name, and a live status dot.
- **Workspace Breadcrumb (`WorkspaceBreadcrumb.svelte:25-52`)**:
  - Displays `[Project] > [Folder] > [File Path]` with visual hierarchy and one-click path copying to clipboard.

### 3.3 Sub-Agent Switching & Keyboard Navigation

- **Micro-Pills (`AgentMicroPills.svelte:33-83`)**:
  - Synthesizes assigned agents into rounded status pills.
  - Keyboard accessible: Handles `Enter` and `Space` keydown events (`AgentMicroPills.svelte:42-48`).
  - Direct selection: Clicking invokes `onSelectAgent(agent)`, updating `activeSubAgent` in `App.svelte`.
- **Direct Directives**:
  - `App.svelte:8303-8365`: When submitting a message while `activeSubAgent` is active, the prompt is packaged into an `AgentMessageEnvelope` (`createAgentEnvelope`) directed to `activeSubAgent.id`, appended to `subAgentMessagesMap`, and recorded in the agent's ledger.

### 3.4 Real-Time State Indicators & Security Badges

- **Status Pulse Indicators**:
  - `running`: Blue pulsing ring (`animation: pill-pulse 1.8s infinite`).
  - `waiting` / `needs_help`: Amber pulsing ring with `HelpCircle` icon.
  - `completed`: Green checkmark icon.
  - `failed` / `error`: Red `AlertCircle` icon.
- **Security Badges (`ConversationTopbar.svelte:118-137`)**:
  - Interactive pill displaying active permission mode (`Standard`, `Lecture seule` / `Read-only`, `Développeur` / `Developer`, `Sandbox`, `Custom`).
  - Color-coded dot: Green for standard, Blue for read-only, Orange for developer, Purple for sandbox.
  - Clicking badge opens the permission policy configuration panel (`onOpenPermissionSettings`).
- **Browser Address Bar Security**:
  - `AgentStepCard.svelte:294-298`: Renders Apple-style traffic lights (red, yellow, green), a locked padlock icon (`Lock size={11}`), and formatted domain pill.

### 3.5 Detailed Live Inspection View (Reasoning, Execution Steps, Tool Calls)

- **Live Reasoning Stream (`ConversationView.svelte:578-620`)**:
  - Live `<think>` parsing via `parseMessageThinking`.
  - While generating: Pulsing indicator (`ARO réfléchit...` or `${activeSubAgent.name} réfléchit...`).
  - Upon completion: Collapsible card labeled `Réflexion` or `Réflexion (${activeSubAgent.name})` with duration and token counts.
- **Live Tool Strip (`ConversationView.svelte:517-560`)**:
  - Shows active in-flight tool with icon, title, target URL, and actionable buttons (`Prendre le contrôle`, `Ouvrir l'onglet`).
- **Rich Step Cards (`AgentStepCard.svelte`)**:
  - Dedicated cards for Browser, Shell/Terminal, Document/Spreadsheet, MCP Connectors, and Diffs.
- **Live Terminal Log Viewer (`AgentLiveLogViewer.svelte`)**:
  - Embedded in `RightPanel.svelte` (under the "Agents" tab when viewing an agent run).
  - Features: Real-time event log stream, category filters (`all`, `tool`, `error`, `info`), search filter, copy all logs, and toggleable auto-scroll.

### 3.6 UX Gaps & Opportunities Identified

1. **Composer Placeholder & Sub-Agent Mode**:
   - In `App.svelte:10870`, `{activeSubAgent}` is passed to `<Composer>`, but `Composer.svelte` does not declare `export let activeSubAgent: SubAgentInfo | null = null;`.
   - Consequently, the Composer input placeholder remains "Demandez à ARO..." rather than "Directive pour ${activeSubAgent.name}...".
2. **Step Duration Display**:
   - While `AgentStepCard` displays start time and title, explicit elapsed duration (e.g. `245ms`, `1.2s`) is computed in log entries but not consistently badged in the step card header.

---

## 4. Requirement R4 Deep-Dive: Test Suites & Quality Verification Invariants

### 4.1 Verification Scripts Execution & Audit Results

| Script | Command | Target / Scope | Result | Details |
|---|---|---|:---:|---|
| `contracts:check` | `npm run contracts:check` | `@aro/contracts` (`tsc --noEmit`) | **PASS** | 14 contract files checked. 0 errors. |
| `api-client:check` | `npm run api-client:check` | `@aro/api-client` (`tsc --noEmit`) | **PASS** | 7 client files checked. 0 errors. |
| `api-client:test` | `npm run api-client:test` | `@aro/api-client` (`vitest run`) | **PASS** | 2 test files, 10 tests passed (100%). |
| `test:unit` | `npm run test:unit` | `@aro/desktop` (`vitest run`) | **PASS** | 33 test files, 312 tests passed (100%). |
| `test:components` | `npm run test:components` | `@aro/desktop` (`vitest components`) | **PASS** | 20 test files, 288 tests passed (100%). |
| `lint:rust` | `npm run lint:rust` | Rust workspace (fmt + clippy) | **PASS** | Zero format diffs, zero clippy warnings. |
| `cargo test (build)` | `cargo test --workspace --no-run` | 16 Rust crates, 42 test binaries | **PASS** | All 42 test executables compile cleanly. |
| `check` (desktop) | `npm run check` | `@aro/desktop` (`svelte-check`) | **FAIL** | **2 TypeScript errors** in `App.svelte`. |

### 4.2 Detailed Analysis of the 2 `svelte-check` Type Errors

Running `npm run check` (`svelte-check --tsconfig ./tsconfig.json` in `apps/desktop`) surfaced 2 compilation errors:

#### Defect 1: Type Incompatibility in `ensurePresetPermissionProfile` Call
- **File**: `apps/desktop/src/App.svelte`
- **Line**: 7807, Column 116
- **Verbatim Error**:
  ```
  c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\apps\desktop\src\App.svelte:7807:116
  Error: Argument of type 'PermissionPresetMode' is not assignable to parameter of type '"standard" | "read-only" | "developer" | "sandbox"'.
    Type '"custom"' is not assignable to type '"standard" | "read-only" | "developer" | "sandbox"'. (ts)
          provider,
          autonomyProfileId: customPermissionId || activePermissionProfileId || (await ensurePresetPermissionProfile(activePermissionPreset)) || null,
          maxSteps: null,
  ```
- **Root Cause**:
  `activePermissionPreset` is typed as `PermissionPresetMode = "standard" | "read-only" | "developer" | "sandbox" | "custom"`. However, the helper function `ensurePresetPermissionProfile` (`App.svelte:1537`) only accepts `"standard" | "read-only" | "developer" | "sandbox"`. When `activePermissionPreset === "custom"`, passing it directly violates TypeScript strictness.
- **Recommended Fix**:
  Guard the call: `(activePermissionPreset !== "custom" ? await ensurePresetPermissionProfile(activePermissionPreset) : null)`.

#### Defect 2: Type Incompatibility in `webAccess` Assignment
- **File**: `apps/desktop/src/App.svelte`
- **Line**: 8472, Column 9
- **Verbatim Error**:
  ```
  c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\apps\desktop\src\App.svelte:8472:9
  Error: Type 'false | WebAccessMode' is not assignable to type 'WebAccessMode | undefined'.
    Type 'false' is not assignable to type 'WebAccessMode | undefined'. (ts)
          attachments: attachmentRefs,
          webAccess: (activePermissionPreset === "sandbox" || activePermissionPreset === "read-only" ? false : (activePermissionPreset === "custom" && getActivePermissionProfile() ? Boolean(getActivePermissionProfile()?.allowNetwork) : permNetworkAccess)) ? webAccess : false,
          searchSettings: settings?.search ?? null,
  ```
- **Root Cause**:
  The `SendMessageRequest` interface types `webAccess?: WebAccessMode` (where `WebAccessMode = "off" | "search-only" | "full"`). In line 8472, if network permission is denied, the ternary expression evaluates to the boolean `false` instead of `"off"` or `undefined`.
- **Recommended Fix**:
  Replace `: false` with `: "off"` (or `undefined`).

---

## 5. Defect Catalog & Invariants Matrix

| ID | Category | Location | Severity | Description | Remediated Status |
|---|---|---|:---:|---|:---:|
| **DEF-01** | Type Safety | `apps/desktop/src/App.svelte:7807` | **High** | `ensurePresetPermissionProfile` called with `"custom"` | Documented (Needs fix in Phase 1) |
| **DEF-02** | Type Safety | `apps/desktop/src/App.svelte:8472` | **High** | `webAccess` receives boolean `false` instead of `WebAccessMode` | Documented (Needs fix in Phase 1) |
| **DEF-03** | UX / Prop | `apps/desktop/src/features/chat/Composer.svelte` | **Low** | Missing `export let activeSubAgent` declaration | Documented (Needs fix in Phase 1) |
| **INV-01** | Invariant | `@aro/contracts` | **Critical** | 100% contracts check passes | **Verified (PASS)** |
| **INV-02** | Invariant | `@aro/api-client` | **Critical** | 100% client test passes | **Verified (PASS)** |
| **INV-03** | Invariant | `@aro/desktop` Unit | **Critical** | 100% unit tests pass without regression | **Verified (PASS - 312 tests)** |
| **INV-04** | Invariant | `@aro/desktop` Comp | **Critical** | 100% component tests pass without regression | **Verified (PASS - 288 tests)** |
| **INV-05** | Invariant | Rust Workspace | **Critical** | Zero clippy warnings & fmt compliance | **Verified (PASS)** |

---

## 6. Implementation Recommendations & Next Steps

1. **Phase 1 Remediation (Zero-Regression & Type Cleanliness)**:
   - Resolve the 2 TypeScript type errors in `App.svelte` so that `npm run check` completes with 0 errors.
   - Add `export let activeSubAgent: SubAgentInfo | null = null;` to `Composer.svelte` and display an interactive sub-agent directive indicator inside the Composer input area.
2. **Phase 2 UX Polish (Apple & Google Standards)**:
   - Add step execution duration badges (e.g. `120ms`) to `AgentStepCard` headers.
   - Add sound / haptic micro-interactions for sub-agent completion and step errors.
3. **Phase 3 Test Invariant Hardening**:
   - Add dedicated test cases in `SubAgentInteraction.svelte.test.ts` verifying that permission mode changes correctly propagate down to sub-agent step cards.

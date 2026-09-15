# Project: ARO AI Workspace Key Features Implementation

## Architecture
ARO Desktop is a high-performance desktop application built with Tauri 2 (Rust backend) and Svelte 5 / TypeScript frontend, styled with Tailwind CSS and macOS/Apple glassmorphic aesthetic standards.
- **Backend**: Rust crates in `crates/` (`aro-core`, `aro-memory`, `aro-runtime`, `aro-tools`, `aro-agent`) and Tauri app in `apps/desktop/src-tauri/`.
- **Frontend**: Svelte 5 application in `apps/desktop/src/`, organized into `features/` (`chat`, `shell`, `workspace`, `models`, `organizations`, `files`, etc.) and `lib/` (`api/transport.ts`, `markdown.ts`, `richContent.ts`, etc.).
- **Data Flow**:
  - Chat and AI commands execute via `transport.ts` communicating with Tauri backend via IPC `invoke` and event streaming (`listen`).
  - Workspace files and project tree indexed through `workspace_tree_get`.
  - Plans persisted in SQLite `plans` table and manipulated via `plans_list`, `plan_create`, `plan_update`, `plan_delete`.
  - Multi-agent runs tracked via `startAgentRun`, `listAgentRuns`, `getAgentOrchestratorSnapshot`, and `agent-step-update` events.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | F1.1 | Split & Unified Diff Viewer with syntax highlighting | M2 | ORIGINAL_REQUEST §R1 |
| 2 | F1.2 | Direct actions: Appliquer au projet, Rejeter, Copier le code | M2 | ORIGINAL_REQUEST §R1 |
| 3 | F1.3 | Disk writing/diff application via secure Tauri commands | M1 | ORIGINAL_REQUEST §R1, AC2 |
| 4 | F1.4 | Live Artifacts & Deliverables card list in RightPanel Outputs tab | M2 | ORIGINAL_REQUEST §R1, AC1 |
| 5 | F1.5 | Chat code/diff block preview and inspect actions | M2 | ORIGINAL_REQUEST §R1 |
| 6 | F2.1 | Extended TaskStep schema (status: pending/in_progress/completed/error, error) | M1 | ORIGINAL_REQUEST §R2 |
| 7 | F2.2 | Dynamic Plan de Travail checklist with 4-state visual indicators in RightPanel | M3 | ORIGINAL_REQUEST §R2, AC3 |
| 8 | F2.3 | Manual task addition and interactive status cycling | M3 | ORIGINAL_REQUEST §R2, AC3 |
| 9 | F2.4 | Assistant AI roadmap generation trigger button | M3 | ORIGINAL_REQUEST §R2 |
| 10 | F2.5 | Real-time plan synchronization via `aro:plans-updated` event | M3 | ORIGINAL_REQUEST §R2 |
| 11 | F3.1 | Multi-Agent lanes view without static empty screen (virtual default lane) | M3 | ORIGINAL_REQUEST §R3, AC4 |
| 12 | F3.2 | Synchronized live activity counters (Actifs, En file, Terminés) | M3 | ORIGINAL_REQUEST §R3 |
| 13 | F3.3 | AgentLiveLogViewer connected in Subagents tab for live reasoning/tools inspection | M3 | ORIGINAL_REQUEST §R3 |
| 14 | F4.1 | Pure @ mention detection, filtering, token insertion, path extraction | M4 | ORIGINAL_REQUEST §R4 |
| 15 | F4.2 | Composer @ mention popover with keyboard navigation & file/dir icons | M4 | ORIGINAL_REQUEST §R4 |
| 16 | F4.3 | Workspace file tree indexing & context injection in submitMessage | M4 | ORIGINAL_REQUEST §R4 |
| 17 | F5.1 | 0 typecheck errors on `npm --workspace @aro/desktop run check` | M5 | ORIGINAL_REQUEST §AC5 |
| 18 | F5.2 | 100% passing unit tests on `npm --workspace @aro/desktop run test:unit` | M5 | ORIGINAL_REQUEST §AC6 |
| 19 | F5.3 | 100% passing component tests on `npm --workspace @aro/desktop run test:components` | M5 | ORIGINAL_REQUEST §AC6 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Backend IPC & Schemas | Tauri commands (`workspace_file_read`, `workspace_file_write`, `workspace_diff_apply`, `workspace_git_diff`), TaskStep schema extension in Rust & TS transport, pure diff utility `diff.ts` | none | DONE |
| 2 | M2: Visual Diff & Artifacts | Enhanced `CodeDiffViewer` (split/unified, highlight, copy, apply), RightPanel outputs tab revamp, chat diff preview | M1 | DONE |
| 3 | M3: Roadmap & Multi-Agent | RightPanel plan tab dynamic checklist, 4-state indicators, AI roadmap button, aro:plans-updated event; Subagents tab AgentLiveLogViewer, lane fix, synchronized counters | M1 | DONE |
| 4 | M4: Composer @ Mentions | mention-model.ts, Composer.svelte @ popover, keyboard navigation, context injection in App.svelte | none | DONE |
| 5 | M5: Final E2E Pass & Audit | Full test suite execution, 0 check errors, 100% unit & component tests, adversarial hardening | M1, M2, M3, M4 | DONE |
| E2E | E2E Testing Track | Independent opaque-box test suite (Tiers 1-4) published via TEST_READY.md | none | DONE |

## Interface Contracts

### Backend ↔ Frontend File I/O
- `workspace_file_read(conversation_id: Option<String>, path: String) -> Result<String, String>`
- `workspace_file_write(conversation_id: Option<String>, path: String, content: String) -> Result<(), String>`
- `workspace_diff_apply(conversation_id: Option<String>, path: String, diff_patch: String) -> Result<(), String>`
- `workspace_git_diff(conversation_id: Option<String>, path: Option<String>) -> Result<String, String>`

### Plan Data Model Contract
```typescript
export type TaskStepStatus = "pending" | "in_progress" | "completed" | "error";

export interface TaskStep {
  id: string;
  text: string;
  completed: boolean;
  status?: TaskStepStatus;
  error?: string | null;
}

export interface Plan {
  id: string;
  conversationId: string;
  title: string;
  description?: string;
  tasks: TaskStep[];
  status: "active" | "completed" | "archived";
  createdAt: string;
  updatedAt: string;
}
```

### Mention Model Contract
```typescript
export interface MentionTrigger {
  active: boolean;
  query: string;
  startIndex: number;
  endIndex: number;
}

export interface WorkspaceMentionEntry {
  name: string;
  path: string;
  isDir: boolean;
  size?: number;
  relativePath: string;
  extension: string;
}
```

## Code Layout
- `apps/api/src/main.rs`: Axum cloud backend HTTP API router, server lifecycle, and CLI (serve, migrate, worker).
- `apps/api/src/handlers.rs`: REST and SSE endpoints for auth, conversations, assistant, files, agents, integrations.
- `apps/desktop/src-tauri/src/main.rs`: Tauri commands registration and file I/O IPC.
- `crates/aro-core/src/plan.rs`: Rust Plan and TaskStep structs.
- `apps/desktop/src/lib/api/transport.ts`: API clients and type definitions.
- `apps/desktop/src/lib/api/conversations-memory-files.ts`: Domain exports for conversations, files, plans.
- `apps/desktop/src/lib/api.ts`: Master API re-export façade.
- `apps/desktop/src/lib/diff.ts`: Pure diff calculation and split/unified row generation.
- `apps/desktop/src/lib/diff.test.ts`: Unit tests for diff calculation.
- `apps/desktop/src/lib/plan-utils.ts`: Plan progress, task status cycling, step validation.
- `apps/desktop/src/lib/agent-utils.ts`: Agent lane synthesis, live activity counters, step→log mapping.
- `apps/desktop/src/lib/artifacts.ts`: Artifact extraction and status model.
- `apps/desktop/src/features/workspace/CodeDiffViewer.svelte`: Interactive Split/Unified Diff viewer.
- `apps/desktop/src/features/workspace/WorkspaceTreeExplorer.svelte`: Hierarchical workspace tree, active file, root badge.
- `apps/desktop/src/features/workspace/WorkspaceFileViewer.svelte`: Right-side file preview with root breadcrumb.
- `apps/desktop/src/features/workspace/QuickOpenModal.svelte`: Fuzzy file opener (flat `WorkspaceTreeEntry[]`).
- `apps/desktop/src/features/conversations/ConversationDestinationPicker.svelte`: Pre-send project/folder picker.
- `apps/desktop/src/features/shell/RightPanel.svelte`: Outputs tab (artifacts & diffs), Plan tab (dynamic checklist), Subagents tab (lanes & AgentLiveLogViewer), Explorer tab (tree + viewer).
- `apps/desktop/src/features/chat/mention-model.ts`: Pure logic for @ mentions (detection, filtering, ranking, path extraction, tree flattening).
- `apps/desktop/src/features/chat/mention-model.test.ts`: Unit tests for mention detection and filtering.
- `apps/desktop/src/features/chat/Composer.svelte`: Chat input with @ autocomplete popover.
- `apps/desktop/src/App.svelte`: App coordinator, workspace entry indexing, message submission context injection, agent lane synthesis.

## Runtime Notes
- Agent model/tool loop is **unlimited**: it runs until the model returns final/pause (or a validation/provider error). The old 8-default/32-hard-cap only survives as legacy constants in old migrations; current budget ceiling is 32767 (`202607260001_agent_unlimited_steps.sql`).
- `POST /v1/assistant/stream` performs **real server-side generation**: active local provider first, then local-first Ollama fallback (first non-embedding chat model from `/api/tags`), otherwise an explicit actionable error message — never a stub.
- Hardening suites (`*.adversarial.test.ts`, `challenger-m2-*`, `RightPanel.adversarial.svelte.test.ts`) are non-contractual stress tests, not feature gates.

## Security Notes
- Direct server-side tool execution (`POST /tools/code/execute`, `/tools/document/create`) is closed unless `ARO_AGENT_DIRECT_TOOL_EXECUTION=true`; invocations are warn-logged with tenant identity.
- Workspace file tools confine every path carrying a root: absolute paths and `..` outside the root are rejected (`contained_in_root`), with symlink revalidation when targets exist.
- Shell guard is a tripwire only (normalized whitespace, destructive/persistence/exfil patterns); real authorization stays the agent permission profile.
- TOTP MFA enrollment and disable both require a valid RFC 6238 code (SHA-1, ±1 step, constant-time compare); provider API keys stay in the desktop OS keyring and never reach the server.
- Conversation placement (`project_id`/`folder_id`) is tenant-checked server-side: foreign ids are rejected instead of silently linked.
- Agent connectors (`connector.list`/`connector.call`) are backed by installed plugin MCP servers, not stubs; destructive shell patterns (incl. obfuscated `| sh`, encoded PowerShell, shadow-copy deletion) are rejected and covered by tests.

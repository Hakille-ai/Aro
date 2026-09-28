# Project: ARO Architecture — Multi-Agent Collaboration, Advanced Tooling & Observability

## Architecture
- **Rust Backend Crates**:
  - `crates/aro-core`: Domain models, contracts, agent run status, cognitive memory schemas (`AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope`).
  - `crates/aro-memory`: Local SQLite persistence for runs, steps, artifacts, and cognitive memory (`agent_memories`, `agent_findings`, `agent_message_envelopes`).
  - `crates/aro-store`: Cloud PostgreSQL migrations and models mirroring SQLite persistence schemas.
  - `crates/aro-tools`: Tool execution engine (`ToolExecutor`), path confinement (`resolve_workspace_path`), shell/code execution sandboxing (`scrubbed_env`).
  - `crates/aro-runtime`: Orchestration kernel, sub-agent execution scheduler (`tokio::spawn` background task loop), tool loop interception with `ToolAuthorizationGuard`.
  - `crates/aro-agent`: Agent lifecycle, prompt synthesis, tool registry (`ToolRegistry`).
- **Frontend Packages & Applications**:
  - `packages/contracts`: Shared TypeScript data schemas, inter-agent envelope definitions, permission directives.
  - `packages/api-client`: Universal HTTP/IPC client for cloud and desktop backends.
  - `apps/desktop`: Svelte 5 / Tauri desktop application with Apple/Google-grade UX, sub-agent micro-pills, breadcrumbs, live inspection cards, and terminal logs.

---

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Cognitive Memory Persistence Schema | Add `AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope` models & SQLite tables (`agent_memories`, `agent_findings`, `agent_message_envelopes`) | M1 | Survey 1 |
| 2 | Sub-Agent Asynchronous Execution Loop | Launch genuine async loop (`tokio::spawn`) in `aro-runtime` on desktop so sub-agents run model turns and execute tools instead of freezing at step 2 | M1 | Survey 1 |
| 3 | Cloud Worker Delegation Handling | Implement delegation handlers in `apps/api/src/agent_tools.rs` to support multi-agent delegation without failing as unsupported | M1 | Survey 1 |
| 4 | Persistent Cognitive Memory IPC & API | Provide Tauri IPC commands and API endpoints to save, retrieve, and sync agent scratchpads, findings, and message ledgers | M1 | Survey 1 |
| 5 | Kernel-Grade Tool Authorization Guard | Hard pre-execution check in `aro-runtime` & `aro-tools` validating active `PermissionPreset` (`standard`, `read-only`, `developer`, `sandbox`) before every tool call | M2 | Survey 2 |
| 6 | Strict Workspace Path Confinement | Fix `resolve_workspace_path` in `aro-tools` to fail closed if `root_path` is missing or if path escapes workspace root | M2 | Survey 2 |
| 7 | Code & Shell Execution Sandboxing | Apply `scrubbed_env` (stripping secrets/tokens) and execution resource limits to `core.code.execute` and `core.shell.execute` | M2 | Survey 2 |
| 8 | Tool Registry Catalogue Parity | Register missing tools (`workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, `artifact.create`) in `ToolRegistry` | M2 | Survey 2 |
| 9 | Svelte Typecheck Bug Fixes | Fix the 2 TypeScript type mismatches in `apps/desktop/src/App.svelte` (lines 7807 and 8472) to restore clean typechecking | M3 | Survey 3 |
| 10 | Persistent Sub-Agent Thread UI Integration | Replace volatile `localStorage` and canned mock replies in `App.svelte` / `agent-protocol.ts` with live backend memory & directive dispatch | M3 | Survey 3 |
| 11 | Apple/Google-Grade Observability Refinements | Update `Composer.svelte` with `activeSubAgent` prop and dynamic placeholder; verify micro-pills, breadcrumbs, badges, inspection cards | M3 | Survey 3 |
| 12 | Opaque-Box E2E Test Suite (Tiers 1-4) | Comprehensive E2E tests validating multi-agent flow, permission guard enforcement, and UI invariants; publish `TEST_READY.md` | M4 | Survey 3 |
| 13 | Zero-Regression Quality Invariants & Audit | 100% pass on contracts, api-client, unit, component tests, rust clippy (0 warnings), adversarial coverage (Tier 5), and Forensic Audit | M4 | Survey 3 |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Rust Multi-Agent Core & Persistence Engine | Features 1, 2, 3, 4: Cognitive memory models, SQLite schemas, async sub-agent execution loop in `aro-runtime`, cloud worker delegation handling | none | PLANNED |
| 2 | Kernel-Grade Tool Authorization Guard & Sandboxing | Features 5, 6, 7, 8: `ToolAuthorizationGuard` interceptor, workspace containment fix, `scrubbed_env` execution, tool registry parity | M1 | PLANNED |
| 3 | Desktop UI, Observability & Typecheck Integrity | Features 9, 10, 11: Fix `App.svelte` type errors, wire desktop UI to persistent cognitive backend, polish breadcrumbs/composer placeholder | M1, M2 | PLANNED |
| 4 | E2E Testing Suite, Zero-Regression Invariants & Victory Audit | Features 12, 13: 4-Tier E2E test suite (`TEST_READY.md`), all 5 verification suites passing, Tier 5 adversarial tests, Forensic Audit | M1, M2, M3 | PLANNED |

---

## Interface Contracts

### Multi-Agent Cognitive Memory Contract (`aro-core` ↔ `aro-memory` ↔ `@aro/contracts`)
- `AgentMemoryFinding`:
  ```rust
  pub struct AgentMemoryFinding {
      pub id: String,
      pub summary: String,
      pub category: String, // "fact", "constraint", "decision", "discovery"
      pub source_tool: Option<String>,
      pub timestamp: i64,
  }
  ```
- `AgentMemoryContext`:
  ```rust
  pub struct AgentMemoryContext {
      pub agent_id: String,
      pub agent_name: String,
      pub role: String,
      pub conversation_id: String,
      pub scratchpad: String,
      pub findings: Vec<AgentMemoryFinding>,
      pub ledger: Vec<AgentMessageEnvelope>,
      pub artifacts: Vec<AgentArtifactRef>,
      pub permission_profile_id: Option<String>,
      pub updated_at: i64,
  }
  ```

### Tool Authorization Guard Contract (`aro-runtime` ↔ `aro-tools`)
- In `crates/aro-tools/src/lib.rs`:
  ```rust
  pub enum PermissionPreset {
      Standard,
      ReadOnly,
      Developer,
      Sandbox,
      Custom,
  }
  
  pub struct ToolAuthorizationGuard {
      pub preset: PermissionPreset,
      pub allowed_tools: Option<HashSet<String>>,
      pub denied_tools: Option<HashSet<String>>,
  }
  
  impl ToolAuthorizationGuard {
      pub fn check_permission(&self, tool_name: &str) -> Result<(), ToolAuthorizationError>;
  }
  ```

### Desktop IPC & Backend Sync (`apps/desktop/src-tauri` ↔ `apps/desktop/src`)
- Tauri Commands:
  - `agent_get_memory(agent_id: String, conversation_id: String) -> Result<AgentMemoryContext, String>`
  - `agent_save_memory(memory: AgentMemoryContext) -> Result<(), String>`
  - `agent_dispatch_directive(agent_id: String, directive: String, conversation_id: String) -> Result<AgentRunView, String>`

---

## Code Layout
- `crates/aro-core/src/`: Core domain models & contracts (`agent.rs`, `memory.rs`)
- `crates/aro-memory/src/`: SQLite persistence & migrations (`lib.rs`, `sqlite.rs`)
- `crates/aro-tools/src/`: Tool implementations & security guards (`lib.rs`, `security.rs`)
- `crates/aro-runtime/src/`: Agent runtime execution & sub-agent loop (`lib.rs`, `guard.rs`)
- `crates/aro-agent/src/`: Tool registry & descriptors (`lib.rs`)
- `packages/contracts/src/`: TypeScript schemas (`agent.ts`, `tools.ts`)
- `apps/desktop/src/`: Svelte UI frontend (`App.svelte`, `Composer.svelte`, `lib/agent-protocol.ts`)
- `apps/desktop/src-tauri/src/`: Tauri IPC bindings (`commands.rs`, `main.rs`)
- `tests/e2e/`: Requirement-driven opaque-box E2E test suite

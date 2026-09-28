# Master Plan: ARO Autonomous Multi-Agent, Tooling & Observability

## Objective
Fulfill all requirements of ORIGINAL_REQUEST.md:
1. Multi-Agent Collaboration & Autonomous Orchestration (Rust backend, persistent cognitive memory, async scheduler).
2. Advanced Agent Tooling & Security Sandboxing (Tool authorization guard, path confinement, env scrubbing).
3. Apple & Google-Grade Desktop Experience & Observability (Typecheck fixes, sub-agent threads, breadcrumbs, live indicators).
4. Comprehensive Verification & Zero-Regression Invariants (All TypeScript, Svelte, API-client, Rust clippy, and E2E suites passing 100%).

---

## Milestone Execution Strategy

### Milestone 1: Rust Multi-Agent Core & Persistence Engine
- **Features**:
  - Feature 1: Cognitive Memory Persistence Schema (`aro-memory` SQLite tables `agent_memories`, `agent_findings`, `agent_message_envelopes`).
  - Feature 2: Sub-Agent Async Execution Loop (`aro-runtime` tokio background task loop).
  - Feature 3: Cloud Worker Delegation Handling (`apps/api/src/agent_tools.rs`).
  - Feature 4: Persistent Cognitive Memory IPC (`apps/desktop/src-tauri`).
- **Iteration Protocol**:
  - 3 Explorers: analyze `aro-core`, `aro-memory`, `aro-runtime`, `apps/desktop/src-tauri`.
  - 1 Worker: implement SQLite tables, migrations, async scheduler loop, IPC commands.
  - 2 Reviewers: review correctness, memory leaks, concurrency safety, interface conformance.
  - 2 Challengers: stress test concurrent memory access and async subagent execution.
  - 1 Forensic Auditor: verify authentic implementation (no mocks/facades).
  - Milestone Gate Check.

### Milestone 2: Kernel-Grade Tool Authorization Guard & Sandboxing
- **Features**:
  - Feature 5: Kernel-Grade `ToolAuthorizationGuard` (`aro-runtime`, `aro-tools`).
  - Feature 6: Strict Workspace Path Confinement (`aro-tools`).
  - Feature 7: Code & Shell Execution Sandboxing (`scrubbed_env`).
  - Feature 8: Tool Registry Catalogue Parity (`aro-agent`).
- **Iteration Protocol**:
  - 3 Explorers -> 1 Worker -> 2 Reviewers -> 2 Challengers -> 1 Auditor -> Gate.

### Milestone 3: Desktop UI, Observability & Typecheck Integrity
- **Features**:
  - Feature 9: Fix Svelte typecheck errors in `App.svelte`.
  - Feature 10: Persistent Sub-Agent Thread UI Integration.
  - Feature 11: Apple/Google-Grade Observability Refinements (`Composer.svelte`, breadcrumbs, inspection cards).
- **Iteration Protocol**:
  - 3 Explorers -> 1 Worker -> 2 Reviewers -> 2 Challengers -> 1 Auditor -> Gate.

### Milestone 4: Comprehensive Verification, Invariants & Final Audit
- **Verification Invariants**:
  - `npm run contracts:check` (100% pass)
  - `npm run api-client:test` (100% pass)
  - `npm run test:unit` & `npm run test:components` (100% pass)
  - `npm run lint:rust` (cargo check/clippy 0 warnings)
  - `npm run test:e2e:opaque` (127/127 E2E tests passing)
- **Phase 2 Hardening**:
  - Tier 5 Adversarial Coverage Hardening with Challengers.
  - Final Forensic Audit verification.

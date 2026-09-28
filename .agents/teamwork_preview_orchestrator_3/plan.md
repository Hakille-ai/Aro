# Milestone Execution Plan — Orchestrator 3

## Overview
Continue and complete end-to-end execution of ARO architecture across Milestones 2, 3, and 4, ensuring all functional and acceptance criteria are met with zero regressions and clean forensic audit.

---

## Milestone 2: Kernel-Grade Tool Authorization Guard & Sandboxing
### Target Features
- **Feature 5**: Kernel-Grade Tool Authorization Guard: Hard pre-execution check in `aro-runtime` & `aro-tools` validating active `PermissionPreset` (`standard`, `read-only`, `developer`, `sandbox`) before every tool call.
- **Feature 6**: Strict Workspace Path Confinement: Fix `resolve_workspace_path` in `aro-tools` to fail closed if `root_path` is missing or if path escapes workspace root.
- **Feature 7**: Code & Shell Execution Sandboxing: Apply `scrubbed_env` (stripping secrets/tokens) and execution resource limits to `core.code.execute` and `core.shell.execute`.
- **Feature 8**: Tool Registry Catalogue Parity: Register missing tools (`workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, `artifact.create`) in `ToolRegistry`.

### Execution Cycle
1. **Exploration**:
   - `teamwork_preview_explorer_m2_1`: Investigate `ToolAuthorizationGuard`, `PermissionPreset` enum, `check_permission`, and runtime interception points in `crates/aro-tools` and `crates/aro-runtime`.
   - `teamwork_preview_explorer_m2_2`: Investigate path confinement `resolve_workspace_path`, `scrubbed_env` implementation, and sandbox environment variables/limits in `crates/aro-tools`.
   - `teamwork_preview_explorer_m2_3`: Investigate `ToolRegistry` in `crates/aro-agent`, missing tool implementations in `crates/aro-tools` (`workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, `artifact.create`), and contracts in `@aro/contracts`.
2. **Implementation**:
   - Worker implements all M2 features, runs Rust unit/integration tests and clippy.
3. **Verification & Audit**:
   - Reviewer 1 & Reviewer 2 evaluate code correctness, completeness, and interface contracts.
   - Challenger 1 & Challenger 2 adversarially stress-test path escaping, secret scrubbing, and unauthorized tool calls.
   - Forensic Auditor audits integrity (no hardcoded stubs or fake pass).
   - Gate verdict evaluation.

---

## Milestone 3: Desktop UI, Observability & Typecheck Integrity
### Target Features
- **Feature 9**: Svelte Typecheck Bug Fixes: Fix TypeScript mismatches in `apps/desktop/src/App.svelte` (lines 7807 and 8472) to restore clean typechecking (`npm run contracts:check`, svelte-check).
- **Feature 10**: Persistent Sub-Agent Thread UI Integration: Replace volatile `localStorage` and canned mock replies in `App.svelte` / `agent-protocol.ts` with live backend memory & directive dispatch (`agent_get_memory`, `agent_save_memory`, `agent_dispatch_directive`).
- **Feature 11**: Apple/Google-Grade Observability Refinements: Update `Composer.svelte` with `activeSubAgent` prop and dynamic placeholder; verify micro-pills, breadcrumbs, badges, inspection cards.

### Execution Cycle
1. Exploration (UI typecheck errors, IPC wiring, Composer and breadcrumb UX).
2. Worker implementation & tests (`npm run test:unit`, `npm run test:components`, `npm run contracts:check`).
3. Gate verification (Reviewers, Challengers, Auditor).

---

## Milestone 4: Comprehensive Verification, Zero-Regression Invariants & Final Audit
### Acceptance Invariants
- 100% of TypeScript checks and `@aro/contracts` pass (`npm run contracts:check`).
- 100% of `@aro/api-client` tests pass (`npm run api-client:test`).
- 100% of `@aro/desktop` unit & component tests pass (`npm run test:unit`, `npm run test:components`).
- Rust code compiles and passes quality audit with zero warnings (`npm run lint:rust`).
- All 127 E2E tests pass (`npm run test:e2e:opaque`).
- Phase 2 adversarial coverage hardening (Tier 5).
- Final Forensic Audit confirmation.
- Completion report to Sentinel.

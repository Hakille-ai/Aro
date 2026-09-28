# BRIEFING — 2026-09-25T09:34:00Z

## Mission
Investigate Feature 10 (Persistent Sub-Agent Thread UI Integration) in apps/desktop: identify localStorage/mock usage, map live Tauri IPC integration, design graceful fallback, and evaluate test suites.

## 🔒 My Identity
- Archetype: explorer
- Roles: Teamwork explorer (Read-only investigation)
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m3_2
- Original parent: 279e94fd-d099-4039-9ebd-159c33f6194c
- Milestone: M3 Feature 10

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Provide concrete file paths, line numbers, function signatures, and exact code recommendations
- No code modification in workspace

## Current Parent
- Conversation ID: 279e94fd-d099-4039-9ebd-159c33f6194c
- Updated: not yet

## Investigation State
- **Explored paths**:
  * `apps/desktop/src/lib/agent-protocol.ts` & `agent-protocol.test.ts`
  * `apps/desktop/src/App.svelte` (lines 920–988, 8303–8365)
  * `apps/desktop/src-tauri/src/commands.rs` (lines 12–145) & `main.rs` (lines 5104–5106)
  * `apps/desktop/src/lib/api/transport.ts` & `api/agents-permissions-arena.ts`
  * `crates/aro-core/src/agent.rs` (`AgentMemoryContext`, `AgentMemoryFinding`)
  * `@tauri-apps/api/core` internals
  * Test suites: `npm run test:unit`, `npm run test:components`, `contracts:check`, `npm run check`, `cargo check -p aro-desktop`
- **Key findings**:
  * Ephemeral storage in `agent-protocol.ts` uses `localStorage` (`aro:agent-memory:`, `aro:agent-ledger:`) and in-memory Maps.
  * `App.svelte` (lines 8343–8352) emits a canned mock reply on sub-agent directives and never triggers backend execution.
  * Rust Tauri backend already implements `agent_get_memory`, `agent_save_memory`, and `agent_dispatch_directive` wired to SQLite and async runtime.
  * Tauri v2 `invoke` requires `window.__TAURI_INTERNALS__`. Safe fallback via `isTauri()` is required for automated Vitest suites and web previews.
  * Baseline test status: 312 unit tests passed (33 files), 288 component tests passed (20 files), 0 typecheck errors, clean Rust compilation.
- **Unexplored areas**: None within Feature 10 scope.

## Key Decisions Made
- Architecture blueprint completed for wrapping IPC calls in `transport.ts` and re-exporting in `agents-permissions-arena.ts`.
- Retain synchronous signatures in `agent-protocol.ts` for backward compatibility while providing async loaders and background sync.
- Wire `App.svelte` directive dispatch to `dispatchAgentDirective`, updating `agentRuns` and `subAgentMessagesMap` with real live status.
- Designed complete graceful fallback preventing test regressions.

## Artifact Index
- analysis.md — Full technical investigation report
- handoff.md — 5-component handoff report
- progress.md — Heartbeat and status tracking
- DISPATCH.md — User request record
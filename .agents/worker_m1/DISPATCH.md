## 2026-09-24T10:47:16Z
Implement Milestone 1 — Rust Multi-Agent Core & Persistence Engine.
1. In `crates/aro-core`: Add `AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope`, `AgentMessageType`, and `AgentParticipant` with Serde serialization. Ensure perfect alignment with TypeScript schemas in `packages/contracts/src/agent.ts`.
2. In `crates/aro-memory`: Add SQLite migration & tables (`agent_memories`, `agent_findings`, `agent_message_envelopes`). Implement database methods to upsert, get, list, and query memory contexts, findings, and message ledgers.
3. In `crates/aro-runtime`: Fix `start_agent_run` / `execute_orchestration_tool` for `core.agent.delegate` and `core.agent.spawn`. Launch a genuine asynchronous background execution loop (`tokio::spawn`) on desktop so sub-agents progress beyond step 2, execute their assigned goals, call model/tools, and record their steps and output artifacts.
4. In `apps/api/src/agent_tools.rs`: Add support for multi-agent delegation tools (`core.agent.delegate`, `core.agent.spawn`) instead of rejecting them with `worker_unsupported_tool`.
5. In `apps/desktop/src-tauri/src/commands.rs`: Implement Tauri IPC commands for cognitive memory retrieval and persistence (`agent_get_memory`, `agent_save_memory`, `agent_dispatch_directive`).

File ownership (exclusive):
- `crates/aro-core/`
- `crates/aro-memory/`
- `crates/aro-runtime/`
- `crates/aro-store/`
- `apps/api/src/agent_tools.rs`
- `apps/desktop/src-tauri/`

Verification:
- Run `npm run lint:rust` (must pass with 0 warnings).
- Run `cargo test -p aro-core -p aro-memory -p aro-runtime` or relevant unit tests to verify your implementation.
- Document all changes, files touched, commands run, and test outputs in `handoff.md`.
- Send a completion message to parent orchestrator (`a4cc995f-7135-4215-8715-da435049fa02`).

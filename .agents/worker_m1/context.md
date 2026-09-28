# Context for Worker M1 — Rust Multi-Agent Core & Persistence Engine

- Mission: Implement Milestone 1 (Features 1, 2, 3, 4 from PROJECT.md)
  1. Add `AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope`, `AgentMessageType` to `crates/aro-core`.
  2. Implement SQLite persistence tables (`agent_memories`, `agent_findings`, `agent_message_envelopes`) and queries in `crates/aro-memory`.
  3. Implement genuine background asynchronous execution loop (`tokio::spawn`) in `crates/aro-runtime` for sub-agents (`start_agent_run` / delegation) so sub-agents run model turns and execute tools without freezing.
  4. Implement delegation handling in `apps/api/src/agent_tools.rs`.
  5. Add Tauri IPC commands for cognitive memory read/write.
- Authoritative Request: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\ORIGINAL_REQUEST.md
- Project Scope: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\PROJECT.md
- Explorer Findings:
  - c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_1\survey_rust_orchestration.md
  - c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_1\handoff.md
- Write Ownership:
  - `crates/aro-core/`
  - `crates/aro-memory/`
  - `crates/aro-runtime/`
  - `crates/aro-store/`
  - `apps/api/src/agent_tools.rs`
  - `apps/desktop/src-tauri/`
- Requirements:
  - Rust code must compile cleanly and pass `npm run lint:rust` with 0 warnings.
  - Run `cargo test --workspace` or targeted tests to verify implementation.
  - Document all changes and test outputs in `handoff.md`.

# BRIEFING — 2026-09-24T10:44:00Z

## Mission
Phase 0 Survey 2: Investigate the agent tooling ecosystem, execution engine, and security sandboxing mechanisms for Requirement R2 (Advanced Agent Tooling & Security Sandboxing).

## 🔒 My Identity
- Archetype: teamwork_preview_explorer
- Roles: explorer, analyst, investigator
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_2
- Original parent: a4cc995f-7135-4215-8715-da435049fa02
- Milestone: Phase 0 Architecture & Tooling Survey

## 🔒 Key Constraints
- Read-only investigation — do NOT implement or modify source code
- Files for content delivery, messages for coordination
- Self-contained handoff report (handoff.md) with 5 components
- Output findings in survey_tooling_sandboxing.md and handoff.md

## Current Parent
- Conversation ID: a4cc995f-7135-4215-8715-da435049fa02
- Updated: 2026-09-24T10:44:00Z

## Investigation State
- **Explored paths**:
  - `ORIGINAL_REQUEST.md` (Req R2: Advanced Agent Tooling & Security Sandboxing)
  - `crates/aro-tools/src/lib.rs`: ToolExecutor, resolve_workspace_path, web/workspace/shell/code/browser/computer tools
  - `crates/aro-agent/src/lib.rs`: ToolRegistry, core descriptors, PermissionPolicy
  - `crates/aro-core/src/agent.rs` & `tool.rs`: PermissionProfile, ToolDescriptor, category, execution spec
  - `crates/aro-runtime/src/lib.rs`: AssistantEngine, run_tool_loop, execute_agent_tool, execute_tool_request
  - `crates/aro-policy/src/lib.rs`: PolicyEngine, AuthorizationRequest, DecisionConstraints, attenuation
  - `crates/aro-skills/src/sandbox.rs`: SandboxLimits, scrubbed_env, StagedSkillDir
  - `apps/api/src/agent_tools.rs` & `agent_runner.rs`: execute_worker_tool, authorize_worker_tool
  - `apps/desktop/src-tauri/src/main.rs` & `state.rs`: Tauri command dispatch, execute_tool_with_workspace_root
  - `packages/contracts/src/agent.ts`: compilePermissionDirective, AgentPermissionProfile, PermissionPresetMode
  - `apps/desktop/src/App.svelte` & `transport.ts`: ensurePresetPermissionProfile, activePermissionPreset
- **Key findings**:
  - Desktop execution engine does NOT enforce `PermissionProfile` or preset policies (passes `autonomy_profile_id: None` in `send_message`, evaluates only `WebAccessPolicy`).
  - Presets (Standard, Lecture seule, Développeur autonome, Sandbox) are currently only soft prompt instructions in TypeScript contracts.
  - Path confinement escape exists in `resolve_workspace_path` if `root_path` is not provided.
  - Unsandboxed code and shell execution in `ToolExecutor` (runs with host environment).
  - Parity gaps in `ToolRegistry` (missing descriptors for `workspace.delete`, `replace_in_files`, `git_diff`, `artifact.create`).
  - All test invariants verified: contracts check (0), api-client tests (10/10), unit tests (312/312), component tests (159/159), rust lint (0 errors, 0 warnings).
- **Unexplored areas**: None for Phase 0 Survey 2.

## Key Decisions Made
- Authored comprehensive architecture and survey report in `survey_tooling_sandboxing.md`.
- Prepared self-contained 5-component handoff in `handoff.md`.

## Artifact Index
- `DISPATCH.md` — Initial dispatch log
- `BRIEFING.md` — Persistent working memory
- `progress.md` — Liveness heartbeat
- `survey_tooling_sandboxing.md` — Detailed survey report
- `handoff.md` — 5-component handoff report

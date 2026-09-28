# Handoff Report: Feature 5 — Kernel-Grade Tool Authorization Guard & Sandboxing

**Agent**: teamwork_preview_explorer_m2_1  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m2_1`  
**Parent Orchestrator ID**: `279e94fd-d099-4039-9ebd-159c33f6194c`  
**Milestone**: Milestone 2  
**Handoff Type**: Hard (Task Complete)  

---

## 1. Observation

1. **E2E Test Specifications for Feature 5**:
   - `tests/e2e/helpers/security-guard-engine.ts:88–197` defines `ToolAuthorizationGuard` with `preset: PermissionPresetMode`, `profile?: AgentPermissionProfile`, `allowedTools?: Set<string>`, `deniedTools?: Set<string>`, and `checkPermission(toolName: string)`.
   - `tests/e2e/tier1-feature-coverage.test.ts:388–445` verifies:
     - T1.5.1: `Standard` allows read/write and flags shell for user confirmation (`Requires user confirmation before execution`).
     - T1.5.2: `ReadOnly` permits read (`workspace.read`, `workspace.list_dir`), strictly blocks writes (`workspace.write`, `workspace.delete`) and shell (`core.shell.execute`) with `/forbidden in read-only/i`.
     - T1.5.3: `Sandbox` strictly blocks filesystem (`workspace.read`), write, shell, and network (`external.fetch`) with `/forbidden in sandbox/i`.
     - T1.5.4: `Developer` grants full access across read, write, shell, and network.
     - T1.5.5: `Custom` enforces granular profile flags (`allowRead`, `allowWrite`, `allowShell`, `allowNetwork`).
   - `tests/e2e/tier2-boundary-corner.test.ts:317–366` verifies:
     - T2.5.1: Normalizes mixed-case and leading/trailing whitespace (`"  Workspace.Read  "`).
     - T2.5.2: Rejects empty or whitespace-only tool names with `ToolAuthorizationError("Tool name cannot be empty")`.
     - T2.5.3: `denied_tools` takes highest precedence (overrides `Developer` preset).
     - T2.5.4: Lockdown custom profile blocks all tools.
     - T2.5.5: Custom preset without profile throws `Custom preset requires an active AgentPermissionProfile`.
   - `tests/e2e/tier3-cross-feature.test.ts:82–105` (Combo 3: F2 + F5) verifies:
     - Sub-agent in Read-Only mode attempting file write is blocked:
       `expect(result.status).toBe("failed");`
       `expect(result.error).toContain("Tool Authorization Denied");`
       `expect(result.error).toContain("forbidden in Read-Only mode");`
       An `error_escalation` envelope is automatically dispatched to the memory ledger with `"Security Guard Interception"`.
   - `tests/e2e/tier3-cross-feature.test.ts:216–229` (Combo 9) verifies:
     - Whitelist `allowedTools` blocks uncatalogued tools (`Tool not found in explicit allowed_tools whitelist`).
   - `tests/e2e/tier4-application-scenarios.test.ts:136–166` (Scenario 2) verifies:
     - In `Sandbox` mode, `agent.read_context` is permitted, while filesystem, write, shell, and network are blocked.

2. **Backend Codebase State**:
   - `crates/aro-core/src/tool.rs:693–700`: `ToolExecutionStatus` already has `Running`, `Completed`, `Failed`, `Blocked`.
   - `crates/aro-core/src/agent.rs:220–237`: `AgentRun` has `autonomy_profile_id: Option<Uuid>`.
   - `crates/aro-memory/src/lib.rs:197–203`: Table `agent_permission_profiles` exists with `id`, `name`, `profile_json`, `created_at`, `updated_at`. Line 2170 has `upsert_permission_profile`, but lacks `get_permission_profile` and `get_permission_profile_by_name`.
   - `crates/aro-tools/src/lib.rs`: Does not yet define `PermissionPreset` or `ToolAuthorizationGuard`.
   - `crates/aro-runtime/src/lib.rs:1505–1588`: `execute_tool_request` is the central funnel executing all tools (`execute_agent_tool`, `execute_tool`, `prefetch_web_context`), but currently performs no authorization check prior to tool invocation.
   - `apps/desktop/src-tauri/src/commands.rs:64–140`: `agent_dispatch_directive` retrieves `existing_memory`, but does not map `permission_profile_id` to `autonomy_profile_id` in `AgentRunStartRequest`.

---

## 2. Logic Chain

1. **Component Placement (Observation 1 & 2)**:
   - `PROJECT.md` line 77 mandates `PermissionPreset` and `ToolAuthorizationGuard` in `crates/aro-tools/src/lib.rs`.
   - Placing the implementation in `crates/aro-tools/src/security.rs` and re-exporting it in `crates/aro-tools/src/lib.rs` isolates the authorization logic, avoids cluttering the 4,967-line `aro-tools/src/lib.rs`, and allows clean usage by `aro-runtime` and test suites.

2. **Permission Evaluation Hierarchy (Observation 1)**:
   - Step 1: Tool name validation (empty/whitespace check fails closed).
   - Step 2: `denied_tools` blacklist check takes highest priority (T2.5.3: overrides `Developer`).
   - Step 3: `allowed_tools` whitelist check (if specified, tool must be present; Combo 9).
   - Step 4: Tool classification via `classify_tool` (`Shell`, `Network`, `Write`, `Read`, `Other`).
   - Step 5: Preset enforcement:
     - `ReadOnly`: Blocks `Shell`, `Write`, `Network`. Allows `Read` and `Other` (e.g. `core.agent.status`).
     - `Standard`: Allows `Read`, `Write`, `Network`; flags `Shell` with `Requires user confirmation before execution`.
     - `Developer`: Full access.
     - `Sandbox`: Blocks `Shell`, `Write`, `Network`, and `Read` (except context inspection like `agent.read_context`).
     - `Custom`: Requires profile, enforces `allow_read`, `allow_write`, `allow_shell`, `allow_network`.

3. **Pre-Execution Interception Locus (Observation 2)**:
   - In `crates/aro-runtime/src/lib.rs`, `execute_tool_request` is called for every tool call from agents, sub-agents, and direct API invocations.
   - Placing `guard.check_permission(&request.tool_id)` at line 1513 guarantees the underlying tool (`self.tools.execute`, `execute_shell`, etc.) is NEVER invoked if authorization fails.
   - When denied:
     - Generates a `ToolExecutionResult` with `ToolExecutionStatus::Blocked`.
     - Records the step in SQLite via `self.store_tool_execution` with `AgentStepStatus::Failed`.
     - Dispatches an `ErrorEscalation` envelope into the cognitive memory ledger (satisfying Combo 3).
     - Returns `Err(AroError::Security(error_msg))` to immediately abort the unauthorized action.

4. **Sub-Agent Loop Invariant (Observation 1 & 2)**:
   - In `crates/aro-runtime/src/lib.rs:344–348`, `execute_agent_tool(...).await?` propagates the `AroError::Security`.
   - The loop catches the error, updates `run.status = AgentRunStatus::Failed`, sets `run.last_error = Some(err.to_string())`, saves to SQLite, and halts the sub-agent task loop cleanly.

5. **Directive Persistence Bridge (Observation 2)**:
   - In `apps/desktop/src-tauri/src/commands.rs`, extracting `permission_profile_id` from existing agent memory and setting `autonomy_profile_id` ensures background directive executions retain and enforce the agent's frozen permission preset.

---

## 3. Caveats

- **Network-Level SSRF / Domain Restrictions**: Feature 5 focuses on tool-level authorization (`PermissionPreset`). Domain allowlisting for HTTP requests is handled by `WebAccessPolicy::from_permission_profile` in `crates/aro-tools/src/lib.rs:3723`, which functions synergistically with the guard.
- **Confirmation Flow in Headless Sub-Agents**: In `Standard` preset, shell tools return `allowed: true, reason: "Requires user confirmation before execution"`. In headless sub-agent execution, command approval mode should be resolved from `profile.command_approval`. If set to `Always`, automated commands will require pause/approval.
- **No Source Code Modified**: As an explorer agent, no repository code files were modified. All implementations are presented in `analysis.md` and this handoff.

---

## 4. Conclusion

The design for Feature 5 is complete, fully specified, and aligned with all 5 verification suites.
- `crates/aro-tools/src/security.rs` will house `PermissionPreset`, `ToolAuthorizationGuard`, `ToolAuthorizationError`, `classify_tool`, and `ToolCategory`.
- `crates/aro-tools/src/lib.rs` will re-export these types.
- `crates/aro-memory/src/lib.rs` will add `get_permission_profile` and `get_permission_profile_by_name`.
- `crates/aro-runtime/src/lib.rs` will intercept tool invocations in `execute_tool_request` before dispatching, persisting blocked steps, recording error escalation envelopes, and aborting runs on violations.
- `apps/desktop/src-tauri/src/commands.rs` will link `agent_dispatch_directive` to the agent's cognitive memory permission profile.

---

## 5. Verification Method

Once implemented, verify with the following commands:

1. **Compile & Lint Backend**:
   ```powershell
   cargo check -p aro-tools -p aro-runtime -p aro-memory
   cargo clippy -p aro-tools -p aro-runtime -p aro-memory -- -D warnings
   ```
2. **Execute Rust Integration Tests**:
   ```powershell
   cargo test -p aro-runtime --test tool_authorization_guard_tests
   cargo test -p aro-runtime --test memory_tools_adversarial_tests
   ```
3. **Execute Full TypeScript E2E Test Suite**:
   ```powershell
   npm run test:e2e
   # Specific feature checks:
   npx vitest run tests/e2e/tier1-feature-coverage.test.ts
   npx vitest run tests/e2e/tier2-boundary-corner.test.ts
   npx vitest run tests/e2e/tier3-cross-feature.test.ts
   npx vitest run tests/e2e/tier4-application-scenarios.test.ts
   ```

**Invalidation Conditions**:
- If `guard.check_permission("workspace.write")` under `ReadOnly` does not throw an error matching `/forbidden in read-only/i`.
- If `guard.check_permission("core.shell.execute")` under `Sandbox` does not throw an error matching `/forbidden in sandbox/i`.
- If an explicitly blacklisted tool in `denied_tools` is permitted under `Developer` preset.
- If an unauthorized sub-agent tool call fails to dispatch an `error_escalation` envelope or leaves the run status as `Running`.

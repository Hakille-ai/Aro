# Handoff Report: Feature 8 — Tool Registry Catalogue Parity

## 1. Observation

### 1.1 Source Code State in `crates/aro-agent/src/lib.rs`
In `crates/aro-agent/src/lib.rs:540-590`, `ToolRegistry::default()` instantiates exactly 36 tool descriptors:
```rust
        let descriptors = vec![
            // Web tools (4)
            core_web_search_descriptor(), core_web_page_read_descriptor(),
            core_browser_navigate_descriptor(), core_browser_action_descriptor(),
            // System & computer use tools (1)
            core_computer_use_descriptor(),
            // Workspace tools (5)
            workspace_write_descriptor(), workspace_read_descriptor(),
            workspace_list_descriptor(), workspace_grep_descriptor(),
            workspace_search_descriptor(),
            // Shell & Code execution & Document creation (3)
            shell_execute_descriptor(), core_code_execute_descriptor(),
            core_document_create_descriptor(),
            // Plan tools (4)
            plan_create_descriptor(), plan_update_descriptor(),
            plan_delete_descriptor(), plan_list_descriptor(),
            // Memory tools (7)
            memory_save_descriptor(), memory_search_descriptor(),
            memory_recall_descriptor(), memory_update_descriptor(),
            memory_forget_descriptor(), memory_list_descriptor(),
            memory_delete_descriptor(),
            // Context tools (1)
            context_search_descriptor(),
            // Orchestration tools (3)
            agent_delegate_descriptor(), agent_spawn_descriptor(),
            agent_status_descriptor(),
            // MCP bridge (1)
            mcp_call_descriptor(),
            // Skill tools (2)
            skill_list_descriptor(), skill_invoke_descriptor(),
            // Connector/plugin tools (2)
            connector_list_descriptor(), connector_call_descriptor(),
            // Communication & notification tools (3)
            notification_send_descriptor(), email_send_descriptor(),
            notification_schedule_descriptor(),
        ];
```
Notice: The four target tools (`workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, `artifact.create`) are **not registered** in `ToolRegistry`.
At `crates/aro-agent/src/lib.rs:3809`, unit test `built_in_registry_exposes_only_valid_executable_descriptors` asserts:
```rust
assert_eq!(registry.descriptors().len(), 36);
```

### 1.2 Execution State in `crates/aro-tools/src/lib.rs`
In `crates/aro-tools/src/lib.rs:235-249`, `ToolExecutor::execute` router handles these tools:
```rust
            TOOL_WORKSPACE_DELETE | "core.workspace.delete" => {
                self.execute_workspace_delete(request).await
            }
            TOOL_WORKSPACE_REPLACE_IN_FILES | "core.workspace.replace_in_files" => {
                self.execute_workspace_replace_in_files(request).await
            }
            TOOL_WORKSPACE_GIT_DIFF | "core.workspace.git_diff" => {
                self.execute_workspace_git_diff(request).await
            }
            ...
            TOOL_ARTIFACT_CREATE | "core.artifact.create" => self.execute_artifact(request).await,
```
And all four underlying executor methods are implemented:
- `execute_workspace_delete` (`crates/aro-tools/src/lib.rs:726-779`): deletes file or directory, returns `{ "path": ..., "status": "deleted" }`.
- `execute_workspace_replace_in_files` (`crates/aro-tools/src/lib.rs:1590-1690`): walks directories recursively, replaces `target` with `replacement`, returns `{ "modifiedFiles": [...], "count": ... }`.
- `execute_workspace_git_diff` (`crates/aro-tools/src/lib.rs:1691-1767`): executes `git diff` on Windows and Unix, returns `{ "diff": ..., "stderr": ..., "exitCode": ... }`.
- `execute_artifact` (`crates/aro-tools/src/lib.rs:1531-1589`): creates `AgentArtifact`, returns `{ "artifactId": ..., "title": ... }`.

### 1.3 Constants and Normalizer in `crates/aro-core/src/tool.rs`
At lines 208-217, 2-segment aliases exist:
```rust
pub const TOOL_WORKSPACE_DELETE: &str = "workspace.delete";
pub const TOOL_WORKSPACE_REPLACE_IN_FILES: &str = "workspace.replace_in_files";
pub const TOOL_WORKSPACE_GIT_DIFF: &str = "workspace.git_diff";
pub const TOOL_ARTIFACT_CREATE: &str = "artifact.create";
```
Canonical 3-segment constants (`TOOL_CORE_WORKSPACE_DELETE`, `TOOL_CORE_WORKSPACE_REPLACE_IN_FILES`, `TOOL_CORE_WORKSPACE_GIT_DIFF`, `TOOL_CORE_ARTIFACT_CREATE`) are absent, and `normalize_tool_id` (lines 73-158) does not map them.

### 1.4 Test Suite Baseline Results
- `cargo test -p aro-agent`: 30 passed (24 unit + 6 integration stress), 0 failed.
- `cargo test -p aro-tools`: 20 passed, 0 failed.
- `cargo check --workspace --all-targets`: compiles cleanly with 0 errors.
- `npm run contracts:check`: passes with code 0.
- `npm run test:e2e:opaque`: all 127 tests pass, including T1.8.1 and T1.8.2.

---

## 2. Logic Chain

1. **Observation**: All 4 target tools have complete executor routines in `crates/aro-tools/src/lib.rs` and are already connected in `ToolExecutor::execute`.
2. **Observation**: In `crates/aro-agent/src/lib.rs`, `ToolRegistry::default()` omits descriptors for `workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, and `artifact.create`.
3. **Inference**: Because `ToolRegistry` generates the `ContextPack.tools` list sent to LLMs, models are not instructed on the parameter schemas for these tools.
4. **Inference**: Furthermore, `AgentRuntime::validate_action` enforces that any requested tool must be enabled in `ContextPack.tools`. Thus, any model turn issuing `workspace.delete` or `workspace.replace_in_files` currently fails validation.
5. **Observation**: Parameter naming in `execute_workspace_replace_in_files` checks only `target` and `replacement`, whereas TypeScript contract `tool-registry-engine.ts:104-107` expects `search` and `replace`.
6. **Inference**: To prevent subtle parameter mismatches, `execute_workspace_replace_in_files` must accept `target` || `search` || `pattern` and `replacement` || `replace`.
7. **Observation**: In `crates/aro-agent/src/lib.rs:3809`, `assert_eq!(registry.descriptors().len(), 36);` will fail when 4 new descriptors are added.
8. **Inference**: The unit test assertion must be updated to 40, alongside assertions validating the IDs of all 4 new tools.

---

## 3. Caveats

1. **Out-of-Scope Artifact Tools**:
   `tests/e2e/helpers/tool-registry-engine.ts` catalogues `artifact.update` and `artifact.list` in addition to `artifact.create`. These two tools are not implemented in `crates/aro-tools` and are outside the scope of Feature 8 as defined in `PROJECT.md:28` (`Register missing tools (workspace.delete, workspace.replace_in_files, workspace.git_diff, artifact.create) in ToolRegistry`).
2. **Feature 5 (Tool Authorization Guard) Coordination**:
   Feature 5 introduces `ToolAuthorizationGuard`. Under `PermissionPreset::ReadOnly`:
   - `workspace.delete`, `workspace.replace_in_files`, and `artifact.create` must be rejected as mutating.
   - `workspace.git_diff` must be allowed as read-only.
   The descriptors designed here explicitly tag capabilities and side effects (`Irreversible`, `Reversible`, `ReadOnly`) to guarantee guard compliance.

---

## 4. Conclusion

Feature 8 implementation is clear, low-risk, and requires no architectural changes:
1. **Core**: Add 3-segment canonical constants and mapping in `normalize_tool_id` in `crates/aro-core/src/tool.rs`.
2. **Agent**: Implement four descriptor constructors (`workspace_delete_descriptor`, `workspace_replace_in_files_descriptor`, `workspace_git_diff_descriptor`, `artifact_create_descriptor`), register them in `ToolRegistry::default()`, and update descriptor count assertion in `crates/aro-agent/src/lib.rs:3809` from 36 to 40.
3. **Tools**: Enhance parameter extraction aliases in `execute_workspace_replace_in_files` and optional `id` in `execute_artifact` in `crates/aro-tools/src/lib.rs`.
4. **Contracts**: Create `packages/contracts/src/tools.ts` and re-export in `packages/contracts/src/index.ts`.

---

## 5. Verification Method

To independently verify the implementation:
1. **Agent Registry & Validation Test**:
   `ash
   cargo test -p aro-agent
   `
   *Expected*: Passes all 30 tests, verifying descriptor count is 40 and all descriptors pass .validate().
2. **Tool Execution Engine Test**:
   `ash
   cargo test -p aro-tools
   `
   *Expected*: Passes all tests including workspace and artifact executions.
3. **TypeScript Contracts Test**:
   `ash
   npm run contracts:check
   `
   *Expected*: TypeScript 	sc --noEmit exits with 0 errors.
4. **E2E Opaque-Box Suite**:
   `ash
   npm run test:e2e:opaque
   `
   *Expected*: All 127 tests pass, specifically validating T1.8.1 (workspace catalogue parity) and T1.8.2 (artifact parity).
5. **Clippy & Linter**:
   `ash
   cargo clippy --workspace --all-targets -- -D warnings
   `
   *Expected*: 0 warnings across the entire workspace.

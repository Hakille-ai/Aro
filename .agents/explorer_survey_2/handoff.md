# Handoff Report — Explorer Survey 2: Tooling & Security Sandboxing

**Agent:** Explorer Survey 2 (`teamwork_preview_explorer`)  
**Parent Orchestrator:** `a4cc995f-7135-4215-8715-da435049fa02`  
**Working Directory:** `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_2`  
**Primary Deliverable:** `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_2\survey_tooling_sandboxing.md`  
**Requirement Focus:** R2 (Advanced Agent Tooling & Security Sandboxing)

---

## 1. Observation

1. **Preset Directives are Only Soft Prompt Injections**:
   - In `packages/contracts/src/agent.ts:434-541`, `compilePermissionDirective(preset, profile, language)` transforms presets (`"standard"`, `"read-only"`, `"developer"`, `"sandbox"`, `"custom"`) into text strings injected into the LLM system prompt (e.g., `"[POLITIQUE DE SÉCURITÉ ET PERMISSIONS : LECTURE SEULE STRICTE]..."`).
   - In `apps/desktop/src/App.svelte:3647-3665`, the same text directive generation occurs directly in the UI before message submission.

2. **Desktop Runtime Tool Execution Disregards Permission Profiles**:
   - In `crates/aro-runtime/src/lib.rs:140-156` (`send_message`), the `AgentRunStartRequest` hardcodes:
     ```rust
     autonomy_profile_id: None,
     ```
   - In `crates/aro-runtime/src/lib.rs:948`, `run_tool_loop` instantiates only:
     ```rust
     let policy = web_policy_for(web_access.clone(), search_settings);
     ```
   - In `crates/aro-runtime/src/lib.rs:1253-1256`, the loop executes tools via:
     ```rust
     match self.tools.execute(request.clone(), policy).await {
         Ok(result) => result,
         Err(err) => failed_tool_result(&request, err.to_string()),
     }
     ```
   - In `crates/aro-tools/src/lib.rs:213-289`, `ToolExecutor::execute` checks `policy: &WebAccessPolicy` only for web search, fetch, and browser navigate. For `execute_workspace_write`, `execute_workspace_delete`, `execute_shell`, `execute_code`, and `execute_computer_use`, no permission check against `PermissionProfile` or the active preset is performed.

3. **Workspace Path Resolution Fails Open Without Explicit Root**:
   - In `crates/aro-tools/src/lib.rs:118-135` (`resolve_workspace_path`):
     ```rust
     match (raw_path, root_buf) {
         (Some(p), Some(root)) => {
             let path = std::path::Path::new(p);
             let joined = if path.is_absolute() {
                 path.to_path_buf()
             } else {
                 root.join(path)
             };
             contained_in_root(&root, &joined)
         }
         (Some(p), None) => Some(std::path::PathBuf::from(p)),
     ```
     When `root_buf` is `None` (omitted from input), `resolve_workspace_path` returns `Some(PathBuf::from(p))` without verifying confinement in any root.

4. **Code & Shell Execution Run Unsandboxed on Desktop**:
   - In `crates/aro-tools/src/lib.rs:781-874` (`execute_shell`), command security relies solely on a substring blacklist (`forbidden_patterns`), which is easily bypassed.
   - In `crates/aro-tools/src/lib.rs:1009-1101` (`execute_code`), code scripts are written to `temp_dir().join("aro_code_exec")` and executed with the full desktop host environment (including all ambient API keys, tokens, and credentials).
   - In contrast, `crates/aro-skills/src/sandbox.rs:54-83` implements `scrubbed_env()` which strips sensitive variables (`SECRET`, `PASSWORD`, `TOKEN`, `ARO_`, `DATABASE`, `REDIS_`, `API_KEY`, `AUTH`, `SESSION`, etc.) and `SandboxLimits` with staged copy directories. This sandboxing pattern is currently used exclusively for `Skill` executions and not for `core.code.execute` or `core.shell.execute`.

5. **Tool Registry Catalogue Parity Gap**:
   - In `crates/aro-agent/src/lib.rs:539-598` (`ToolRegistry::default()`), descriptors exist for 36 core tools.
   - Several tools implemented in `crates/aro-tools/src/lib.rs` are **missing** from `ToolRegistry`:
     * `core.workspace.delete` / `workspace.delete` (`lib.rs:726`)
     * `core.workspace.replace_in_files` / `workspace.replace_in_files` (`lib.rs:1590`)
     * `core.workspace.git_diff` / `workspace.git_diff` (`lib.rs:1691`)
     * `core.artifact.create` / `artifact.create` (`lib.rs:1531`)
   - Furthermore, `core.browser.action` (`lib.rs:2303`) returns a mocked response (`"Browser action executed"`), not driving a real browser engine.

6. **Current Test Baseline Verified 100% Green**:
   - `npm run contracts:check`: exit code 0 (`tsc --noEmit` passed).
   - `npm run api-client:test`: exit code 0 (2 test files, 10 tests passed).
   - `npm run test:unit`: exit code 0 (33 test files, 312 tests passed).
   - `npm run test:components`: exit code 0 (12 test files, 159 tests passed).
   - `npm run lint:rust`: exit code 0 (`cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` passed with 0 errors and 0 warnings).

---

## 2. Logic Chain

1. **From Observation 1 & 2 to Vulnerability A (Bypassed Security Presets)**:
   - Observation 1 proves presets are compiled only into system prompt strings.
   - Observation 2 proves `crates/aro-runtime` passes `autonomy_profile_id: None` and does not check profile permissions during tool loop execution.
   - *Inference*: If a model ignores the prompt directive (via jailbreak, prompt injection, confusion, or subagent instruction), the backend will execute mutating tools (such as `core.workspace.write`, `core.workspace.delete`, or `core.shell.execute`) even when the user selected `Lecture seule` (Read-Only) or `Sandbox`. Hard pre-execution enforcement is mandatory.

2. **From Observation 3 to Vulnerability B (Filesystem Traversal)**:
   - Observation 3 shows `resolve_workspace_path` returns `Some(PathBuf::from(p))` if `root_path` is not passed in the tool's JSON arguments.
   - *Inference*: Any tool call specifying an absolute path (e.g. `C:\Windows\...` or `/etc/...`) without providing `root_path` circumvents `contained_in_root`, allowing reads, writes, and deletions outside the project workspace.

3. **From Observation 4 to Vulnerability C (Credential Exfiltration via Code Execution)**:
   - Observation 4 shows `execute_code` inherits the host process environment without filtering.
   - Observation 4 also shows a complete, working environment scrubber (`scrubbed_env`) already exists in `crates/aro-skills/src/sandbox.rs`.
   - *Inference*: Extracting and reusing `scrubbed_env` and `SandboxLimits` for `core.code.execute` and `core.shell.execute` will eliminate secret leakage risks with zero new external dependencies.

4. **From Observation 5 to Gap D (Tool Catalogue Incompleteness)**:
   - Observation 5 shows `core.workspace.delete`, `replace_in_files`, `git_diff`, and `artifact.create` exist in `ToolExecutor` but are missing from `ToolRegistry`.
   - *Inference*: The agent cannot inspect or discover these tools through standard introspection (`built_in_tool_descriptors`), and their risk/effect classifications are not defined in the catalog.

5. **From Observation 6 to Stability Baseline**:
   - The entire codebase is in a clean, passing state across TypeScript contracts, frontend tests, and Rust clippy.
   - *Inference*: Any hardening changes for R2 must maintain this zero-regression invariant.

---

## 3. Caveats

- **No Caveats.** Every crate relevant to tooling, execution, and security was examined down to line numbers, and all test suites were executed and verified.

---

## 4. Conclusion

Requirement R2 requires transitioning ARO from **advisory prompt-based security** to **strict kernel-grade runtime security**.

Specifically, the implementation plan must:
1. Introduce a unified `ToolAuthorizationGuard` interceptor in `crates/aro-runtime` and `crates/aro-tools` that validates the active `PermissionPreset` / `PermissionProfile` before *every* tool call, returning `status: Blocked` with an informative error reason for disallowed tools.
2. In `Lecture seule` (Read-Only) mode, hard-block all write, delete, shell, code, and network egress tools.
3. In `Sandbox` mode, hard-block all filesystem I/O, process execution, and network tools (pure in-memory reasoning).
4. In `Standard` mode, enforce workspace confinement and require user confirmation for shell/destructive commands.
5. In `Développeur autonome` (Developer) mode, permit workspace mutations and shell commands while maintaining safety circuit-breakers.
6. Fix `resolve_workspace_path` to strictly require and enforce containment in a verified `trusted_root`.
7. Apply `scrubbed_env` and sandboxing to `core.code.execute` and `core.shell.execute`.
8. Complete `ToolRegistry` with full `ToolDescriptor`s for `workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, and `artifact.create`.

All detailed designs, schemas, and architecture blueprints are documented in `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_2\survey_tooling_sandboxing.md`.

---

## 5. Verification Method

To independently verify the findings and code paths:

1. **Inspect Code Points**:
   - Preset directive compilation: `packages/contracts/src/agent.ts:434-541`
   - Hardcoded `autonomy_profile_id: None` in desktop: `crates/aro-runtime/src/lib.rs:150`
   - Unenforced tool dispatch loop: `crates/aro-runtime/src/lib.rs:948, 1253`
   - Open path resolution: `crates/aro-tools/src/lib.rs:118-135`
   - Shell blacklist and code execution: `crates/aro-tools/src/lib.rs:807-866, 1009-1101`
   - Existing skill sandbox: `crates/aro-skills/src/sandbox.rs:54-83`
   - Missing descriptors in `ToolRegistry`: `crates/aro-agent/src/lib.rs:540-598`
2. **Execute Full Test Suite Verification**:
   - `npm run contracts:check` (must pass 0 errors)
   - `npm run api-client:test` (must pass 10/10 tests)
   - `npm run test:unit` (must pass 312/312 tests)
   - `npm run test:components` (must pass 159/159 tests)
   - `npm run lint:rust` (must pass with 0 warnings)

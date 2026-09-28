# Feature 8: Tool Registry Catalogue Parity — Technical Investigation Report

## 1. Executive Summary & Objective

This technical report documents the complete investigation and implementation blueprint for **Feature 8: Tool Registry Catalogue Parity** in the ARO architecture.

### Objectives
1. **Tool Catalogue Parity in crates/aro-agent/src/lib.rs (ToolRegistry)**:
   - Audit all tools currently registered in ToolRegistry vs tools implemented in crates/aro-tools and declared in TypeScript contracts (@aro/contracts).
   - Specifically resolve the status of missing tools:
     * workspace.delete (core.workspace.delete)
     * workspace.replace_in_files (core.workspace.replace_in_files)
     * workspace.git_diff (core.workspace.git_diff)
     * rtifact.create (core.artifact.create)
2. **Implementation Inspection in crates/aro-tools**:
   - Determine whether these 4 tools are already implemented in crates/aro-tools or require new execution routines.
   - Validate parameter names, JSON schemas, capabilities, risks, and descriptions against contract specifications (	ests/e2e/helpers/tool-registry-engine.ts, PROJECT.md, and @aro/contracts).
3. **Execution Handling in ToolExecutor**:
   - Verify if ToolExecutor::execute routes and executes these tools cleanly.

### High-Level Investigation Findings
1. **Implementation Discovery**:
   All 4 tools **are already fully implemented** in crates/aro-tools/src/lib.rs:
   - execute_workspace_delete (crates/aro-tools/src/lib.rs:726-779)
   - execute_artifact (crates/aro-tools/src/lib.rs:1531-1589)
   - execute_workspace_replace_in_files (crates/aro-tools/src/lib.rs:1590-1690)
   - execute_workspace_git_diff (crates/aro-tools/src/lib.rs:1691-1767)
   Furthermore, ToolExecutor::execute router (lines 235-249) already dispatches both 2-segment legacy names and 3-segment string literals to these methods.
2. **Catalogue Registry Gap**:
   ToolRegistry::default() in crates/aro-agent/src/lib.rs (lines 540-590) registers 36 descriptors. **None of the 4 missing tools are currently registered in ToolRegistry**. Their descriptor constructors do not exist.
   A unit test in crates/aro-agent/src/lib.rs:3809 strictly asserts ssert_eq!(registry.descriptors().len(), 36);.
3. **Constants & Normalization Gap**:
   crates/aro-core/src/tool.rs has 2-segment constants (TOOL_WORKSPACE_DELETE, etc.) but lacks canonical 3-segment constants (TOOL_CORE_WORKSPACE_DELETE, etc.) and does not yet handle them in 
ormalize_tool_id.
4. **TypeScript Contracts**:
   packages/contracts/src/tools.ts is not yet created. E2E tests in 	ests/e2e/helpers/tool-registry-engine.ts currently define the contract and catalogue locally. Creating 	ools.ts and exporting it from index.ts completes the architectural contract.

---

## 2. Tool Inventory & Parity Matrix

| Tool Name | Dotted / Canonical ID | In ro-tools Executor | In ro-agent Registry | In ro-core Constants | In TypeScript Contracts | Guard Classification |
|---|---|---|---|---|---|---|
| workspace.read | core.workspace.read | Yes (execute_workspace_read) | Yes (workspace_read_descriptor) | Yes (TOOL_CORE_WORKSPACE_READ) | Yes (	ool-registry-engine.ts) | Read (Allowed in ReadOnly) |
| workspace.write | core.workspace.write | Yes (execute_workspace_write) | Yes (workspace_write_descriptor) | Yes (TOOL_CORE_WORKSPACE_WRITE) | Yes (	ool-registry-engine.ts) | Write (Forbidden in ReadOnly) |
| workspace.list | core.workspace.list | Yes (execute_workspace_list) | Yes (workspace_list_descriptor) | Yes (TOOL_CORE_WORKSPACE_LIST) | Yes (workspace.list_dir) | Read (Allowed in ReadOnly) |
| workspace.grep | core.workspace.grep | Yes (execute_workspace_grep) | Yes (workspace_grep_descriptor) | Yes (TOOL_CORE_WORKSPACE_GREP) | Yes | Read (Allowed in ReadOnly) |
| workspace.search | core.workspace.search | Yes (execute_workspace_search) | Yes (workspace_search_descriptor) | Yes (TOOL_CORE_WORKSPACE_SEARCH) | Yes | Read (Allowed in ReadOnly) |
| **workspace.delete** | core.workspace.delete | **Yes** (execute_workspace_delete:726) | **NO (Missing)** | 2-seg only (TOOL_WORKSPACE_DELETE) | Yes (workspace.delete) | Write (Forbidden in ReadOnly) |
| **workspace.replace_in_files** | core.workspace.replace_in_files | **Yes** (execute_workspace_replace_in_files:1590) | **NO (Missing)** | 2-seg only (TOOL_WORKSPACE_REPLACE_IN_FILES) | Yes (workspace.replace_in_files) | Write (Forbidden in ReadOnly) |
| **workspace.git_diff** | core.workspace.git_diff | **Yes** (execute_workspace_git_diff:1691) | **NO (Missing)** | 2-seg only (TOOL_WORKSPACE_GIT_DIFF) | Yes (workspace.git_diff) | Read (Allowed in ReadOnly) |
| **rtifact.create** | core.artifact.create | **Yes** (execute_artifact:1531) | **NO (Missing)** | 2-seg only (TOOL_ARTIFACT_CREATE) | Yes (rtifact.create) | Write (Forbidden in ReadOnly) |
| core.code.execute | core.code.execute | Yes (execute_code) | Yes (core_code_execute_descriptor) | Yes (TOOL_CORE_CODE_EXECUTE) | Yes (core.code.execute) | Shell / Code |
| core.shell.execute | core.shell.execute | Yes (execute_shell) | Yes (shell_execute_descriptor) | Yes (TOOL_CORE_SHELL_EXECUTE) | Yes (core.shell.execute) | Shell / Command |


---

## 3. Investigation of `crates/aro-agent` (`ToolRegistry`)

### 3.1 Registry Structure & Lifecycle
`ToolRegistry` in `crates/aro-agent/src/lib.rs` (lines 535-621) owns a `Vec<ToolDescriptor>` representing all built-in capabilities available to agents during runtime execution.
When `ToolRegistry::default()` runs:
1. It instantiates all built-in descriptors into a vector.
2. It calls `.validate()` on each descriptor, asserting that schemas, timeouts, retries, capabilities, and aliases conform to kernel invariants.
3. It exposes `enabled_tools(&self) -> Vec<ToolRef>` which converts active descriptors into `ToolRef` models fed into the `ContextPack` prompt synthesizer.
4. It exposes `pub fn built_in_tool_descriptors() -> Vec<ToolDescriptor>` for use by cloud workers (`apps/api/src/handlers.rs`) and storage layers (`crates/aro-store`).

### 3.2 Catalogue Discrepancy
While web, browser, memory, computer use, and core workspace tools (`read`, `write`, `list`, `grep`, `search`) are registered, four crucial tools are absent:
1. `workspace.delete` (`core.workspace.delete`)
2. `workspace.replace_in_files` (`core.workspace.replace_in_files`)
3. `workspace.git_diff` (`core.workspace.git_diff`)
4. `artifact.create` (`core.artifact.create`)

Because these tools are not registered:
- They are omitted from `ContextPack.tools`, preventing model prompts from learning their existence and parameter schemas.
- If an agent model emits a tool action targeting one of these tools, `AgentRuntime::validate_action` (`crates/aro-agent/src/lib.rs:266-279`) rejects the step with: `tool <tool_id> is not enabled in the context pack`.

### 3.3 Test Assertions in `crates/aro-agent/src/lib.rs`
In `crates/aro-agent/src/lib.rs:3806-3860`, the test `built_in_registry_exposes_only_valid_executable_descriptors` states:
```rust
#[test]
fn built_in_registry_exposes_only_valid_executable_descriptors() {
    let registry = ToolRegistry::default();
    assert_eq!(registry.descriptors().len(), 36);
    ...
}
```
When registering the 4 missing tools, this count increases from 36 to 40. The test must be updated accordingly to prevent test suite regressions.

---

## 4. Investigation of `crates/aro-tools` (`ToolExecutor` & Implementations)

### 4.1 Implementation Status in `crates/aro-tools/src/lib.rs`
All 4 missing tools are already implemented in `crates/aro-tools/src/lib.rs`:

#### 1. `execute_workspace_delete` (`crates/aro-tools/src/lib.rs:726-779`)
```rust
pub async fn execute_workspace_delete(
    &self,
    request: ToolExecutionRequest,
) -> AroResult<ToolExecutionResult> {
    let started_at = Utc::now();
    let target_path = resolve_workspace_path(&request.input, "path", false)
        .ok_or_else(|| AroError::Configuration("path parameter is required".to_string()))?;

    if target_path.is_dir() {
        std::fs::remove_dir_all(&target_path).map_err(|err| {
            AroError::Unexpected(format!(
                "failed to remove directory '{}': {}",
                target_path.display(), err
            ))
        })?;
    } else {
        std::fs::remove_file(&target_path).map_err(|err| {
            AroError::Unexpected(format!(
                "failed to remove file '{}': {}",
                target_path.display(), err
            ))
        })?;
    }
    let output = json!({ "path": target_path.display().to_string(), "status": "deleted" });
    ...
}
```
- **Confinement**: Uses `resolve_workspace_path(&request.input, "path", false)` which fails closed on missing root or path traversal.
- **Output format**: Returns JSON with `path` and `status: "deleted"`.
- **Context Sources**: Emits a `ContextSource` with `kind: "workspace-delete"`, title `Deleted <path>`, score `0.95`.

#### 2. `execute_workspace_replace_in_files` (`crates/aro-tools/src/lib.rs:1590-1690`)
```rust
pub async fn execute_workspace_replace_in_files(
    &self,
    request: ToolExecutionRequest,
) -> AroResult<ToolExecutionResult> {
    let started_at = Utc::now();
    let target_str = request.input.get("target").and_then(|v| v.as_str()).unwrap_or("");
    let replacement_str = request.input.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
    let base_path = resolve_workspace_path(&request.input, "path", true)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    ...
}
```
- **Recursive Replacement**: Implements `walk_and_replace`, skipping `.`, `target`, and `node_modules`. Replaces all occurrences in files matching `target_str`.
- **Output format**: Returns JSON with `modifiedFiles` array and `count` integer.
- **Context Sources**: Emits `kind: "workspace-replace"` containing excerpt of modified files.
- **Robustness Observation**: While the function currently expects `target` and `replacement`, `@aro/contracts` and `tests/e2e/helpers/tool-registry-engine.ts` specify parameters as `search` and `replace`.
  * Fix: Enhance extraction to accept `target` || `search` || `pattern` and `replacement` || `replace`.

#### 3. `execute_workspace_git_diff` (`crates/aro-tools/src/lib.rs:1691-1767`)
```rust
pub async fn execute_workspace_git_diff(
    &self,
    request: ToolExecutionRequest,
) -> AroResult<ToolExecutionResult> {
    let started_at = Utc::now();
    let target_dir = resolve_workspace_path(&request.input, "path", true)
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", "git", "diff"]);
        c
    } else {
        let mut c = tokio::process::Command::new("git");
        c.arg("diff");
        c
    };
    ...
}
```
- **Cross-Platform Handling**: Gracefully handles Windows (`cmd /C git diff`) and Unix (`git diff`).
- **Path Filter**: Applies directory or file arguments if specified.
- **Output format**: Returns JSON with `diff`, `stderr`, and `exitCode`.
- **Context Sources**: Emits `kind: "git-diff"` with stdout preview.

#### 4. `execute_artifact` (`crates/aro-tools/src/lib.rs:1531-1589`)
```rust
pub async fn execute_artifact(
    &self,
    request: ToolExecutionRequest,
) -> AroResult<ToolExecutionResult> {
    let started_at = Utc::now();
    let title = request.input.get("title").and_then(|v| v.as_str()).unwrap_or("Artifact");
    let content = request.input.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let kind = request.input.get("kind").and_then(|v| v.as_str()).unwrap_or("markdown");
    let artifact_id = Uuid::new_v4();
    ...
}
```
- **Artifact Creation**: Generates an `AgentArtifact` model attached to `ToolExecutionResult.artifacts`.
- **Output format**: Returns JSON with `artifactId` and `title`.
- **Context Sources**: Emits `kind: "artifact"`.
- **Robustness Observation**: Currently ignores caller-supplied `id` if present. Should optionally parse `id` as `Uuid` if provided.

### 4.2 Router Parity in `ToolExecutor::execute`
Lines 235-249 in `crates/aro-tools/src/lib.rs`:
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
**Conclusion**: `ToolExecutor::execute` already routes these 4 tools seamlessly.

---

## 5. Investigation of `crates/aro-core` (Constants & Normalizer)

### 5.1 Constant Naming & Organization
In `crates/aro-core/src/tool.rs` (lines 207-217):
```rust
// Legacy 2-segment aliases (kept for backwards compat with aro-tools router)
pub const TOOL_WORKSPACE_SEARCH: &str = "workspace.search";
pub const TOOL_WORKSPACE_READ: &str = "workspace.read";
pub const TOOL_WORKSPACE_WRITE: &str = "workspace.write";
pub const TOOL_WORKSPACE_LIST: &str = "workspace.list";
pub const TOOL_WORKSPACE_GREP: &str = "workspace.grep";
pub const TOOL_WORKSPACE_DELETE: &str = "workspace.delete";
pub const TOOL_WORKSPACE_REPLACE_IN_FILES: &str = "workspace.replace_in_files";
pub const TOOL_WORKSPACE_GIT_DIFF: &str = "workspace.git_diff";
pub const TOOL_SHELL_EXECUTE: &str = "shell.execute";
pub const TOOL_ARTIFACT_CREATE: &str = "artifact.create";
```

Notice:
- The legacy 2-segment constants exist.
- However, 3-segment canonical constants (`TOOL_CORE_WORKSPACE_DELETE`, `TOOL_CORE_WORKSPACE_REPLACE_IN_FILES`, `TOOL_CORE_WORKSPACE_GIT_DIFF`, `TOOL_CORE_ARTIFACT_CREATE`) are missing from lines 33-37 where workspace tools are defined.

### 5.2 Normalization in `normalize_tool_id`
In `crates/aro-core/src/tool.rs:73-158`:
```rust
pub fn normalize_tool_id(tool_id: &str) -> &str {
    match tool_id {
        // Workspace tools
        "workspace_read"
        | "workspace.read"
        | "fs.read"
        | "fs.read-file"
        | "core.workspace.read" => TOOL_CORE_WORKSPACE_READ,
        "workspace_write"
        | "workspace.write"
        | "fs.write"
        | "fs.create-file"
        | "core.workspace.write" => TOOL_CORE_WORKSPACE_WRITE,
        "workspace_list" | "workspace.list" | "fs.list" | "fs.list-dir" | "core.workspace.list" => {
            TOOL_CORE_WORKSPACE_LIST
        }
        "workspace_grep" | "workspace.grep" | "fs.grep" | "core.workspace.grep" => {
            TOOL_CORE_WORKSPACE_GREP
        }
        "workspace_search" | "workspace.search" | "fs.search" | "core.workspace.search" => {
            TOOL_CORE_WORKSPACE_SEARCH
        }
        ...
```
The 4 missing tools are not present in this match expression. Adding them ensures that any variation (snake_case, 2-segment, or 3-segment) canonicalizes cleanly in prompt synthesis, step validation, and logging.

---

## 6. Investigation of TypeScript Contracts (`@aro/contracts`)

### 6.1 Current Packages Status
- `packages/contracts/src/agent.ts` defines cognitive memory context and run status.
- `packages/contracts/src/artifacts.ts` defines diff viewer and artifact types.
- `packages/contracts/src/files.ts` defines file tree objects.
- `packages/contracts/src/tools.ts` does not yet exist.
- In `PROJECT.md:112`: `- packages/contracts/src/: TypeScript schemas (agent.ts, tools.ts)`.
- In `tests/e2e/helpers/tool-registry-engine.ts:1-160`: defines `ToolParameterDescriptor`, `ToolDescriptor`, and `ToolRegistryEngine` locally.

### 6.2 Contract Parity Requirements
To satisfy both TypeScript compilation (`npm run contracts:check`) and end-to-end tests:
1. Create `packages/contracts/src/tools.ts` defining shared interfaces and default catalogue.
2. Re-export in `packages/contracts/src/index.ts`.
3. Update `tests/e2e/helpers/tool-registry-engine.ts` to import interfaces from `@aro/contracts`.

---

## 7. Concrete Implementation Blueprint

### Step 1: Add Canonical Constants & Normalization in `crates/aro-core/src/tool.rs`

#### In `crates/aro-core/src/tool.rs` around line 37:
```rust
// Workspace tools
pub const TOOL_CORE_WORKSPACE_WRITE: &str = "core.workspace.write";
pub const TOOL_CORE_WORKSPACE_READ: &str = "core.workspace.read";
pub const TOOL_CORE_WORKSPACE_LIST: &str = "core.workspace.list";
pub const TOOL_CORE_WORKSPACE_GREP: &str = "core.workspace.grep";
pub const TOOL_CORE_WORKSPACE_SEARCH: &str = "core.workspace.search";
pub const TOOL_CORE_WORKSPACE_DELETE: &str = "core.workspace.delete";
pub const TOOL_CORE_WORKSPACE_REPLACE_IN_FILES: &str = "core.workspace.replace_in_files";
pub const TOOL_CORE_WORKSPACE_GIT_DIFF: &str = "core.workspace.git_diff";
pub const TOOL_CORE_ARTIFACT_CREATE: &str = "core.artifact.create";
```

#### In `normalize_tool_id` around line 104:
```rust
        "workspace_delete"
        | "workspace.delete"
        | "fs.delete"
        | "fs.remove"
        | "core.workspace.delete" => TOOL_CORE_WORKSPACE_DELETE,
        "workspace_replace_in_files"
        | "workspace.replace_in_files"
        | "core.workspace.replace_in_files" => TOOL_CORE_WORKSPACE_REPLACE_IN_FILES,
        "workspace_git_diff"
        | "workspace.git_diff"
        | "git.diff"
        | "core.workspace.git_diff" => TOOL_CORE_WORKSPACE_GIT_DIFF,
        "artifact_create"
        | "artifact.create"
        | "core.artifact.create" => TOOL_CORE_ARTIFACT_CREATE,
```

---

### Step 2: Implement Descriptors & Register in `crates/aro-agent/src/lib.rs`

#### Import in `crates/aro-agent/src/lib.rs:24`:
```rust
    TOOL_CORE_WORKSPACE_DELETE, TOOL_CORE_WORKSPACE_GIT_DIFF, TOOL_CORE_WORKSPACE_GREP,
    TOOL_CORE_WORKSPACE_LIST, TOOL_CORE_WORKSPACE_READ, TOOL_CORE_WORKSPACE_REPLACE_IN_FILES,
    TOOL_CORE_WORKSPACE_SEARCH, TOOL_CORE_WORKSPACE_WRITE, TOOL_CORE_ARTIFACT_CREATE,
    TOOL_WORKSPACE_DELETE, TOOL_WORKSPACE_GIT_DIFF, TOOL_WORKSPACE_REPLACE_IN_FILES,
    TOOL_ARTIFACT_CREATE,
```

#### In `ToolRegistry::default()` (line 554):
```rust
            // Workspace tools
            workspace_write_descriptor(),
            workspace_read_descriptor(),
            workspace_list_descriptor(),
            workspace_grep_descriptor(),
            workspace_search_descriptor(),
            workspace_delete_descriptor(),
            workspace_replace_in_files_descriptor(),
            workspace_git_diff_descriptor(),
            // Artifact tools
            artifact_create_descriptor(),
```

#### Add Constructor Functions in `crates/aro-agent/src/lib.rs`:
```rust
fn workspace_delete_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_DELETE.to_string(),
        version: "1.0.0".to_string(),
        name: "Delete file or directory".to_string(),
        description: "Delete a file or directory within the active project workspace.".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "status": { "type": "string" }
            },
            "required": ["path", "status"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement {
            action: "file.write".to_string(),
            resource: "workspace:".to_string(),
        }],
        risk: ToolRisk {
            level: ToolRiskLevel::High,
            effects: vec![ToolPermissionEffect::Delete],
            confirmation: ToolConfirmationPolicy::Always,
        },
        capabilities: vec!["workspace.delete".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.delete.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Irreversible,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 60,
            max_input_bytes: 1024 * 1024,
            max_output_bytes: 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_workspace_delete".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![
            TOOL_WORKSPACE_DELETE.to_string(),
            "fs.delete".to_string(),
            "fs.remove".to_string(),
        ],
        tags: vec!["workspace".to_string(), "file".to_string(), "delete".to_string()],
    }
}

fn workspace_replace_in_files_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_REPLACE_IN_FILES.to_string(),
        version: "1.0.0".to_string(),
        name: "Replace text across files".to_string(),
        description: "Find and replace occurrences of a pattern across files in the workspace.".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": { "type": "string", "minLength": 1 },
                "replacement": { "type": "string" },
                "path": { "type": "string" }
            },
            "required": ["target", "replacement"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "modifiedFiles": { "type": "array", "items": { "type": "string" } },
                "count": { "type": "integer" }
            },
            "required": ["modifiedFiles", "count"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement {
            action: "file.write".to_string(),
            resource: "workspace:".to_string(),
        }],
        risk: ToolRisk {
            level: ToolRiskLevel::Medium,
            effects: vec![ToolPermissionEffect::ReversibleWrite],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["workspace.replace".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.replace_in_files.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 15_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 60,
            max_input_bytes: 5 * 1024 * 1024,
            max_output_bytes: 5 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_workspace_replace_in_files".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![
            TOOL_WORKSPACE_REPLACE_IN_FILES.to_string(),
            "fs.replace_in_files".to_string(),
        ],
        tags: vec!["workspace".to_string(), "file".to_string(), "replace".to_string()],
    }
}

fn workspace_git_diff_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_GIT_DIFF.to_string(),
        version: "1.0.0".to_string(),
        name: "Git diff".to_string(),
        description: "Compute git diff patch between current workspace state and repository HEAD, optionally filtered by path.".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "diff": { "type": "string" },
                "stderr": { "type": "string" },
                "exitCode": { "type": ["integer", "null"] }
            },
            "required": ["diff", "stderr"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement {
            action: "file.read".to_string(),
            resource: "workspace:".to_string(),
        }],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![ToolPermissionEffect::Read],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["workspace.git_diff".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.git_diff.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 15_000,
        retry: ToolRetryPolicy {
            max_attempts: 2,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 60,
            max_input_bytes: 1024 * 1024,
            max_output_bytes: 5 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_workspace_git_diff".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![
            TOOL_WORKSPACE_GIT_DIFF.to_string(),
            "git.diff".to_string(),
        ],
        tags: vec!["workspace".to_string(), "git".to_string(), "diff".to_string()],
    }
}

fn artifact_create_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_ARTIFACT_CREATE.to_string(),
        version: "1.0.0".to_string(),
        name: "Create artifact".to_string(),
        description: "Create an output artifact reference (markdown document, code snippet, report, or data file) associated with the run.".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "minLength": 1 },
                "content": { "type": "string" },
                "kind": { "type": "string" },
                "id": { "type": "string" }
            },
            "required": ["title"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "artifactId": { "type": "string" },
                "title": { "type": "string" }
            },
            "required": ["artifactId", "title"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement {
            action: "artifact.create".to_string(),
            resource: "run:".to_string(),
        }],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![ToolPermissionEffect::ReversibleWrite],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["artifact.create".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "artifact.create.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 60,
            max_input_bytes: 10 * 1024 * 1024,
            max_output_bytes: 10 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_artifact_create".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![
            TOOL_ARTIFACT_CREATE.to_string(),
        ],
        tags: vec!["artifact".to_string(), "output".to_string()],
    }
}
```

#### Update Test in `crates/aro-agent/src/lib.rs:3809`:
```rust
        assert_eq!(registry.descriptors().len(), 40);
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_WORKSPACE_DELETE));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_WORKSPACE_REPLACE_IN_FILES));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_WORKSPACE_GIT_DIFF));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_ARTIFACT_CREATE));
```

---

### Step 3: Polish Parameter Extraction in `crates/aro-tools/src/lib.rs`

#### In `execute_workspace_replace_in_files` (line 1595):
```rust
        let target_str = request
            .input
            .get("target")
            .or_else(|| request.input.get("search"))
            .or_else(|| request.input.get("pattern"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let replacement_str = request
            .input
            .get("replacement")
            .or_else(|| request.input.get("replace"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
```

#### In `execute_artifact` (line 1552):
```rust
        let artifact_id = request
            .input
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);
```

---

### Step 4: Create `packages/contracts/src/tools.ts`
```typescript
export interface ToolParameterDescriptor {
  name: string;
  type: "string" | "number" | "boolean" | "object" | "array";
  description: string;
  required: boolean;
}

export type ToolCategoryName = "workspace" | "artifact" | "core" | "system";

export interface ToolContractDescriptor {
  name: string;
  category: ToolCategoryName;
  description: string;
  parameters: ToolParameterDescriptor[];
  requiredPermissions: string[];
}

export const DEFAULT_WORKSPACE_TOOLS: ToolContractDescriptor[] = [
  {
    name: "workspace.read",
    category: "workspace",
    description: "Read the contents of a file within the workspace",
    parameters: [{ name: "path", type: "string", description: "Relative file path", required: true }],
    requiredPermissions: ["file:read"],
  },
  {
    name: "workspace.write",
    category: "workspace",
    description: "Write content to a file within the workspace",
    parameters: [
      { name: "path", type: "string", description: "Relative file path", required: true },
      { name: "content", type: "string", description: "File content", required: true },
    ],
    requiredPermissions: ["file:write"],
  },
  {
    name: "workspace.delete",
    category: "workspace",
    description: "Delete a file or directory within the workspace",
    parameters: [{ name: "path", type: "string", description: "Relative file path", required: true }],
    requiredPermissions: ["file:write"],
  },
  {
    name: "workspace.replace_in_files",
    category: "workspace",
    description: "Find and replace text patterns across files in the workspace",
    parameters: [
      { name: "search", type: "string", description: "Search pattern", required: true },
      { name: "replace", type: "string", description: "Replacement text", required: true },
    ],
    requiredPermissions: ["file:write"],
  },
  {
    name: "workspace.git_diff",
    category: "workspace",
    description: "Compute git diff patch between current state and repository HEAD",
    parameters: [{ name: "path", type: "string", description: "Optional path filter", required: false }],
    requiredPermissions: ["file:read"],
  },
  {
    name: "workspace.list_dir",
    category: "workspace",
    description: "List directory contents in the workspace",
    parameters: [{ name: "path", type: "string", description: "Relative directory path", required: false }],
    requiredPermissions: ["file:read"],
  },
];
```

---

## 8. Verification Strategy & Invariants

1. **Rust Test Suite**:
   - cargo test -p aro-core (ensures 
ormalize_tool_id succeeds for all 4 tools)
   - cargo test -p aro-agent (ensures 40 descriptors validate cleanly and tests pass)
   - cargo test -p aro-tools (ensures all tool executions pass)
2. **TypeScript Contracts**:
   - 
pm run contracts:check (ensures clean typechecking for @aro/contracts)
3. **E2E Test Suite**:
   - 
pm run test:e2e:opaque (verifies all 127 tests pass, including T1.8.1 to T1.8.5)
4. **Rust Linter**:
   - cargo clippy --workspace --all-targets -- -D warnings (0 warnings)

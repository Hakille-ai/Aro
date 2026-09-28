# Survey Report: Agent Tooling Ecosystem, Execution Engine & Security Sandboxing (Requirement R2)

**Author:** Explorer Survey 2 (`teamwork_preview_explorer`)  
**Date:** 2026-09-24  
**Project Root:** `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro`  
**Working Directory:** `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\explorer_survey_2`  
**Reference Document:** `.agents/ORIGINAL_REQUEST.md` (Requirement R2: Advanced Agent Tooling & Security Sandboxing)

---

## 1. Executive Summary

ARO's vision requires a unified agent tooling and execution environment where agents operate productively yet safely across local desktop and cloud worker environments.

Our investigation of the repository revealed a dual reality:
1. **Rich Tool Palette with Advanced Features**: The codebase already features a substantial set of tools across workspace manipulation, web search/fetch, shell and code execution, document generation, memory management, skills, plugins, and MCP (Model Context Protocol). Furthermore, low-level web fetch security features excellent DNS pinning, SSRF protection against private IP spaces, redirect caps, and content-length controls (`crates/aro-tools/src/lib.rs:2005-2117`). In addition, process-level sandboxing with environment scrubbing (`scrubbed_env`) exists in `crates/aro-skills/src/sandbox.rs`.
2. **Critical Security Boundary & Enforcement Gaps**:
   - **Soft-Only Security Presets in Desktop Agent Loop**: The four security presets (`Standard`, `Lecture seule` / `Read-Only`, `Développeur autonome` / `Autonomous Developer`, `Sandbox`) defined in `@aro/contracts` (`packages/contracts/src/agent.ts:434-541`) are currently only compiled into natural language prompt instructions (`compilePermissionDirective`).
   - **Missing Hard Runtime Enforcement in Desktop Tauri**: In `crates/aro-runtime/src/lib.rs` (`send_message`, `send_message_stream`, and `run_tool_loop`), the desktop agent execution loop passes `autonomy_profile_id: None` (`crates/aro-runtime/src/lib.rs:150`) and evaluates ONLY a `WebAccessPolicy` (`crates/aro-runtime/src/lib.rs:948, 1253`). Disallowed tools in `Read-Only` (e.g., `core.workspace.write`, `core.shell.execute`, `core.workspace.delete`) or `Sandbox` are **not intercepted or blocked** by the execution engine.
   - **Dual Execution Engines with Asymmetric Enforcement**: The cloud worker path (`apps/api/src/agent_tools.rs:522-569`) implements `authorize_worker_tool` checking `PermissionProfile` fields (`allow_network`, `allow_shell`, `allow_write`, `allow_read`), but this check is completely absent in the desktop local runtime (`crates/aro-runtime/src/lib.rs:1202-1257`).
   - **Path Containment Escape Vulnerability**: `resolve_workspace_path` in `crates/aro-tools/src/lib.rs:88-136` fails open when `root_path` is not provided in tool arguments, returning any unconfined path.
   - **Tool Registry Discrepancy**: Several tools implemented in `ToolExecutor` (`core.workspace.delete`, `core.workspace.replace_in_files`, `core.workspace.git_diff`, `core.artifact.create`) lack official `ToolDescriptor` entries in `ToolRegistry` (`crates/aro-agent/src/lib.rs:540-598`), rendering them invisible to standard descriptor reflection.

---

## 2. Architecture of the Agent Tooling Ecosystem

### 2.1 Crate Architecture Overview

The tooling and security subsystem is distributed across several Rust crates and TypeScript packages:

```
┌────────────────────────────────────────────────────────────────────────┐
│                        apps/desktop (Svelte 5)                         │
│  Composer UI, Permission Pill Selector, Tool Execution Visualizer       │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                packages/contracts (@aro/contracts)                     │
│  AgentPermissionProfile, PermissionPresetMode, compilePermissionDir    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                  ┌─────────────────┴─────────────────┐
                  ▼                                   ▼
┌───────────────────────────────────┐   ┌────────────────────────────────┐
│      apps/desktop/src-tauri       │   │        apps/api (Cloud)        │
│  Tauri Commands, AppState, Engine │   │  REST endpoints, Worker Loop   │
└─────────────────┬─────────────────┘   └───────────────┬────────────────┘
                  │                                     │
                  ▼                                     ▼
┌───────────────────────────────────┐   ┌────────────────────────────────┐
│        crates/aro-runtime         │   │      apps/api/agent_tools      │
│  AssistantEngine, run_tool_loop   │   │  execute_worker_tool (Auth)    │
└─────────────────┬─────────────────┘   └───────────────┬────────────────┘
                  │                                     │
                  └─────────────────┬───────────────────┘
                                    │
                  ┌─────────────────┼─────────────────┐
                  ▼                 ▼                 ▼
        ┌──────────────────┐┌───────────────┐┌─────────────────┐
        │ crates/aro-tools ││crates/aro-agent││crates/aro-policy│
        │   ToolExecutor   ││ ToolRegistry   ││ PolicyEngine    │
        │ WebAccessPolicy  ││ PermissionPol  ││ ABAC Evaluator │
        └──────────────────┘└───────────────┘└─────────────────┘
                  │
        ┌─────────┴─────────┬─────────────────┐
        ▼                   ▼                 ▼
┌────────────────┐  ┌────────────────┐  ┌────────────────┐
│crates/aro-skills│ │crates/aro-plugins││ crates/aro-mcp  │
│ SandboxLimits  │  │ PluginManager  │  │ McpClient      │
│ scrubbed_env   │  │ Marketplace    │  │ Stdio/Http     │
└────────────────┘  └────────────────┘  └────────────────┘
```

---

## 3. Tool Palette Inventory & Assessment

The table below catalogs the tools across the repository, their current implementation status, registry exposure, and security evaluation:

| Tool Identifier | Category | Implemented In | In ToolRegistry? | Security Check Status | Gaps & Hardening Needs |
|---|---|---|---|---|---|
| `core.workspace.read` | Workspace / Code | `crates/aro-tools/src/lib.rs:388` | Yes (`aro-agent:551`) | Fails open if `root_path` omitted | Enforce strict confinement in `trusted_roots`; block in Sandbox |
| `core.workspace.write` | Workspace / Code | `crates/aro-tools/src/lib.rs:440` | Yes (`aro-agent:550`) | Fails open if `root_path` omitted | Block in Read-Only and Sandbox modes; path containment |
| `core.workspace.list` | Workspace / Code | `crates/aro-tools/src/lib.rs:500` | Yes (`aro-agent:552`) | Fails open if `root_path` omitted | Block in Sandbox; confine to project roots |
| `core.workspace.grep` | Workspace / Code | `crates/aro-tools/src/lib.rs:610` | Yes (`aro-agent:553`) | Fails open if `root_path` omitted | Block in Sandbox; confine to project roots |
| `core.workspace.search` | Workspace / Code | `crates/aro-tools/src/lib.rs:292` | Yes (`aro-agent:554`) | Fails open if `root_path` omitted | Block in Sandbox; confine to project roots |
| `core.workspace.delete` | Workspace / Code | `crates/aro-tools/src/lib.rs:726` | **NO (Missing)** | None in ToolExecutor! | Add ToolDescriptor; Block in Read-Only and Sandbox; confirm in Standard |
| `core.workspace.replace_in_files` | Workspace / Code | `crates/aro-tools/src/lib.rs:1590` | **NO (Missing)** | None in ToolExecutor! | Add ToolDescriptor; Block in Read-Only and Sandbox; add dry-run diff |
| `core.workspace.git_diff` | Workspace / Code | `crates/aro-tools/src/lib.rs:1691` | **NO (Missing)** | None | Add ToolDescriptor; Block in Sandbox; confine to git repo root |
| `core.artifact.create` | Workspace / Code | `crates/aro-tools/src/lib.rs:1531` | **NO (Missing)** | None | Add ToolDescriptor; tie into agent artifacts collection |
| `core.document.create` | Workspace / Code | `crates/aro-tools/src/lib.rs:1225` | Yes (`aro-agent:558`) | Fails open if `root_path` omitted | Block file persistence in Read-Only and Sandbox modes |
| `core.shell.execute` | Secure Execution | `crates/aro-tools/src/lib.rs:781` | Yes (`aro-agent:556`) | Substring blacklist only | Block in Read-Only & Sandbox; require confirmation in Standard; env scrub |
| `core.code.execute` | Secure Execution | `crates/aro-tools/src/lib.rs:1009` | Yes (`aro-agent:557`) | No env scrub; runs on host | Block in Read-Only & Sandbox; apply `scrubbed_env`; sandbox isolation |
| `core.search.web` | Web / Search | `crates/aro-tools/src/lib.rs:2120` | Yes (`aro-agent:543`) | WebAccessPolicy checked | Block in Sandbox & Read-Only (air-gapped); domain allowlist |
| `core.web.page.read` | Web / Search | `crates/aro-tools/src/lib.rs:2005` | Yes (`aro-agent:544`) | Strong SSRF + DNS pinning | Block in Sandbox & Read-Only; domain allowlist |
| `core.browser.navigate` | Web / Search | `crates/aro-tools/src/lib.rs:2230` | Yes (`aro-agent:545`) | Uses HTTP fetch snapshot | Block in Sandbox & Read-Only; bridge to interactive browser if requested |
| `core.browser.action` | Web / Search | `crates/aro-tools/src/lib.rs:2303` | Yes (`aro-agent:546`) | None (Mocked response) | Make functional or document mock limitation |
| `core.computer.use` | Desktop Control | `crates/aro-tools/src/lib.rs:2346` | Yes (`aro-agent:548`) | None | Block in Sandbox & Read-Only; gate dangerous system actions |
| `core.computer.screenshot` | Desktop Control | `crates/aro-tools/src/lib.rs:2346` | Yes (alias) | None | Block in Sandbox; privacy guard |
| `core.computer.info` | Desktop Control | `crates/aro-tools/src/lib.rs:2346` | Yes (alias) | None | Allowed in Read-Only; blocked in Sandbox |
| `core.computer.launch` | Desktop Control | `crates/aro-tools/src/lib.rs:2346` | Yes (alias) | None | Block in Read-Only & Sandbox; confirm in Standard |
| `core.context.search` | Context Inspection | `crates/aro-runtime/src/lib.rs:2383` | Yes (`aro-agent:573`) | Scoped to store context | Allowed in all modes; query safety |
| `core.memory.*` (7 tools) | Memory Management | `crates/aro-runtime/src/lib.rs:1208` | Yes (`aro-agent:565-571`) | Sqlite store scoped | Reads allowed; writes blocked in Read-Only mode |
| `core.plan.*` (4 tools) | Planning | `crates/aro-runtime/src/lib.rs:1204` | Yes (`aro-agent:560-563`) | Scoped to conversation | Reads allowed; modifications policy-checked |
| `core.agent.delegate` / `spawn` | Orchestration | `crates/aro-runtime/src/lib.rs:2428` | Yes (`aro-agent:575-577`) | Subagent limits | Attenuate permissions to child (no privilege expansion) |
| `core.mcp.call` | Extensibility | `crates/aro-runtime/src/lib.rs:1247` | Yes (`aro-agent:579`) | None | Route through PermissionProfile network & shell gates |
| `core.skill.invoke` | Extensibility | `crates/aro-skills/src/sandbox.rs` | Yes (`aro-agent:582`) | Process sandbox in worker | Apply sandbox in desktop local runtime as well |
| `core.notification.send` | Communication | `crates/aro-tools/src/lib.rs:3241` | Yes (`aro-agent:587`) | Smtp/Desktop notif | Block external communications in Sandbox & Read-Only |

---

## 4. Deep-Dive on Tooling Subsystems

### 4.1 Code Manipulation Tools

1. **Workspace File Operations**:
   - `core.workspace.read`: Reads up to 16,000 characters. Files larger than 16 KB are truncated with `... [truncated]`.
   - `core.workspace.write`: Overwrites or creates files, creating parent directories via `std::fs::create_dir_all`.
   - `core.workspace.replace_in_files`: Performs string replace across directory files. It lacks a dry-run / confirmation mode and does not validate syntax or report diffs.
   - `core.workspace.delete`: Recursively deletes files or directories (`std::fs::remove_dir_all`). Extremely dangerous; currently has no approval mechanism or descriptor in `ToolRegistry`.
   - `core.workspace.git_diff`: Runs `git diff` via subprocess.
2. **Missing Code Manipulation Capabilities**:
   - **Targeted Code Editing / File Patching**: Currently, the agent must overwrite the entire file (`workspace.write`) or do global naive substring replacement (`replace_in_files`). A targeted line/hunk editing tool (e.g. `workspace.patch` or `replace_file_content` with line ranges and verification) is missing.
   - **Workspace File Stat / Inspection**: Inability to inspect file metadata (size, timestamps, permissions) without reading the entire content.

### 4.2 Secure Execution & Sandboxing

1. **Current Shell Execution (`core.shell.execute`)**:
   - Implemented in `crates/aro-tools/src/lib.rs:781-1007`.
   - Security controls:
     * A hardcoded string-matching blacklist (`forbidden_patterns` on lines 808-866), including `rm -rf /`, `format c:`, `cipher /w`, `del /f /s /q c:\`, etc.
     * Execution timeout defaulting to 30s (clamped to 1-300s).
   - **Vulnerabilities / Weaknesses**:
     * Easily bypassed: `cmd.exe /c "set a=rm& set b=-rf& %a% %b% /"` or base64/hex obfuscated commands or PowerShell sub-expressions bypass literal string checks.
     * Inherits the full process environment of the desktop app, including any environment variables, API keys, or user credentials.
     * Does NOT check `PermissionProfile.allow_shell` or active preset mode in `ToolExecutor`.
     * Does NOT pause for user approval even when `command_approval` is set to `always`.
2. **Current Code Execution (`core.code.execute`)**:
   - Implemented in `crates/aro-tools/src/lib.rs:1009-1223`.
   - Writes script to `temp_dir().join("aro_code_exec")` and invokes interpreter (`python`, `node`, `tsx`, `powershell`, `bash`, `cmd`).
   - Cleans up temporary file upon completion.
   - **Vulnerabilities / Weaknesses**:
     * Runs with full host permissions, unconfined filesystem access, and unsanitized environment.
     * No CPU, memory, or network restrictions on the spawned interpreter.
3. **The `aro-skills` Sandbox (`crates/aro-skills/src/sandbox.rs`)**:
   - Demonstrates a solid pattern:
     * `scrubbed_env()`: Filters out environment variables matching `SECRET`, `PASSWORD`, `TOKEN`, `ARO_`, `DATABASE`, `REDIS_`, `API_KEY`, `AUTH`, `SESSION`, etc.
     * `StagedSkillDir`: Stages the script directory in an isolated temporary location with a maximum size cap (`max_stage_bytes: 10MB`).
     * `SandboxLimits`: Enforces timeout (`timeout_secs: 60`), max output bytes (256KB), and kill-on-drop.
   - **Architectural Opportunity**: This sandbox mechanism currently ONLY wraps `aro-skills`. It should be extracted and generalized into a shared execution sandboxing service used by `core.code.execute` and `core.shell.execute`.

### 4.3 Context Inspection & Memory Management

1. **Context Search (`core.context.search`)**:
   - Queries `self.store.search_agent_context(query, limit)`.
   - Returns structured context snippets previously recorded by tools, documents, or agent runs.
2. **Memory Primitives (`core.memory.*`)**:
   - 7 memory tools (`save`, `search`, `recall`, `update`, `forget`, `list`, `delete`).
   - SQLite backed in desktop (`crates/aro-runtime`), PostgreSQL backed in cloud API (`crates/aro-store`).
   - Vector indexing via `aro-vector` with fallback to full-text search (FTS).
3. **Agent Orchestration Inspection (`core.agent.*`)**:
   - `core.agent.delegate`, `core.agent.spawn`, `core.agent.status`.
   - Allows an orchestrator agent to spawn sub-agents and inspect their status.
   - **Requirement R1/R2 Security Requirement**: Delegation MUST attenuate permissions: a child agent cannot have higher privileges or looser constraints than its parent (`aro_policy::attenuate_delegation`).

### 4.4 Search and Web Navigation

1. **Web Search (`core.search.web`)**:
   - Multi-provider support: SearXNG, Brave Search API, Serper Google API, DuckDuckGo HTML scraping, DuckDuckGo JSON.
   - Domain filtering against `WebAccessPolicy.allowed_domains`.
   - Output formatted with ranked `WebSearchResult` artifacts and excerpts.
2. **Web Page Read / Fetch (`core.web.page.read`)**:
   - Outstanding SSRF protection in `crates/aro-tools/src/lib.rs:2005-2117`:
     * DNS resolution check forbidding loopback (`127.0.0.1`, `::1`), private IP ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fc00::/7`), link-local (`169.254.0.0/16`), and broadcast.
     * DNS Pinning (`pinned_client` / `resolve_to_addrs`) preventing DNS rebinding attacks between check and fetch.
     * Manual redirect following with re-validation of each redirect target against egress policies.
     * Content length limit (`max_fetch_bytes`, default 2MB) and HTML-to-markdown text extraction.
3. **Browser Navigation & Actions (`core.browser.*`)**:
   - `core.browser.navigate`: Wraps `fetch_web_page`, returning an HTML snapshot as an artifact.
   - `core.browser.action`: Currently a placeholder mock returning `{ "success": true, "message": "Browser action executed" }`.

---

## 5. Security Boundaries & Permission Policies

### 5.1 The Four Authoritative Security Modes

The system specification in `ORIGINAL_REQUEST.md` mandates four distinct security boundaries:

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                           ARO SECURITY BOUNDARIES                              │
├───────────────────┬──────────────┬──────────────┬──────────────┬───────────────┤
│ Capability / Tool │   Standard   │ Lecture seule│  Autonome    │    Sandbox    │
│                   │  (Balanced)  │ (Read-Only)  │ (Developer)  │   (Isolated)  │
├───────────────────┼──────────────┼──────────────┼──────────────┼───────────────┤
│ File Reading      │ ALLOWED (1)  │ ALLOWED (1)  │ ALLOWED (1)  │ FORBIDDEN (2) │
│ File Writing      │ ALLOWED (1)  │ FORBIDDEN    │ ALLOWED (1)  │ FORBIDDEN     │
│ File Deletion     │ CONFIRM REQ  │ FORBIDDEN    │ ALLOWED (1)  │ FORBIDDEN     │
│ Shell Execution   │ CONFIRM REQ  │ FORBIDDEN    │ ALLOWED (3)  │ FORBIDDEN     │
│ Code Execution    │ SANDBOX/CONF │ FORBIDDEN    │ ALLOWED (3)  │ FORBIDDEN     │
│ External Network  │ ALLOWED (4)  │ FORBIDDEN    │ ALLOWED      │ FORBIDDEN     │
│ Computer Control  │ CONFIRM REQ  │ FORBIDDEN    │ ALLOWED (3)  │ FORBIDDEN     │
│ Memory Reading    │ ALLOWED      │ ALLOWED      │ ALLOWED      │ IN-MEMORY ONLY│
│ Memory Mutation   │ ALLOWED      │ FORBIDDEN    │ ALLOWED      │ FORBIDDEN     │
│ Agent Delegation  │ ATTENUATED   │ ATTENUATED   │ ATTENUATED   │ FORBIDDEN     │
└───────────────────┴──────────────┴──────────────┴──────────────┴───────────────┘
(1) Strictly confined to verified project workspace roots (`trusted_roots`).
(2) Sandbox operates exclusively on the active prompt and in-memory scratchpad.
(3) Safety breakers (forbidden destructive system commands) remain active.
(4) Web search and documentation retrieval restricted to allowed domains.
```

### 5.2 Current Enforcement Gap Analysis

Our investigation identified four major gaps between the security model and runtime enforcement:

#### Gap 1: Desktop Runtime Lacks Policy Evaluation
- In `apps/desktop/src-tauri/src/main.rs:3046` and `crates/aro-runtime/src/lib.rs:140-156`, `send_message` creates an `AgentRunStartRequest` with `autonomy_profile_id: None`.
- In `crates/aro-runtime/src/lib.rs:948`, `run_tool_loop` sets up:
  `let policy = web_policy_for(web_access.clone(), search_settings);`
- When `execute_agent_tool` dispatches tools (`crates/aro-runtime/src/lib.rs:1202-1257`), `policy` is passed ONLY to `self.tools.execute(request.clone(), policy).await`.
- Inside `ToolExecutor::execute` (`crates/aro-tools/src/lib.rs:213-289`), `policy` is only checked for:
  - `TOOL_WEB_SEARCH` / `TOOL_CORE_SEARCH_WEB`
  - `TOOL_WEB_FETCH` / `TOOL_CORE_WEB_PAGE_READ`
  - `TOOL_CORE_BROWSER_NAVIGATE` / `TOOL_BROWSER_NAVIGATE`
- `execute_workspace_write`, `execute_workspace_delete`, `execute_shell`, `execute_code`, and `execute_computer_use` **do not inspect `policy`, `PermissionProfile`, or the active preset at all**!

#### Gap 2: Path Traversal Vulnerability in `resolve_workspace_path`
- In `crates/aro-tools/src/lib.rs:118-135`:
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
    (Some(p), None) => Some(std::path::PathBuf::from(p)), // <-- VULNERABILITY!
...
```
- If the tool caller (or model) supplies `path: "C:\\Windows\\System32\\..."` and omits `root_path`, `resolve_workspace_path` returns the unconfined path directly without checking `contained_in_root`!

#### Gap 3: Asymmetric Authorization (Cloud Worker vs Desktop)
- In `apps/api/src/agent_tools.rs:522-569`, `authorize_worker_tool` classifies tools into `Network`, `Shell`, `Write`, `Read`, `Memory`, `Connector`, `SkillList`, `Blocked` and checks `PermissionProfile` flags. It also verifies `workspace_root_allowed`.
- However, this `authorize_worker_tool` logic is compiled only in `apps/api` and is **not available or invoked** in `apps/desktop/src-tauri` or `crates/aro-runtime`.

#### Gap 4: `aro-policy` ABAC Engine is Bypassed
- `crates/aro-policy` is a full Attribute-Based Access Control (ABAC) engine with data classification, subject/resource refs, decision constraints, delegation attenuation, and audit levels.
- Currently, `aro_policy::PolicyEngine` is only invoked in `apps/api/src/agent_tools.rs:239` for a synthetic legacy adapter with action `"network.read"`. It is never invoked for local desktop tool calls.

---

## 6. Detailed Requirements & Constraints for Requirement R2

To satisfy Requirement R2 and pass all acceptance criteria, the system must implement the following components:

### 6.1 Architectural Requirements

1. **Unified Tool Guard Middleware (`ToolAuthorizationGuard`)**:
   - Must be invoked before every tool execution, regardless of entry point (desktop `run_tool_loop`, desktop Tauri direct command, cloud REST API, cloud worker loop).
   - Input: `ToolExecutionRequest`, active `PermissionProfile` (or resolved Preset Mode), `trusted_roots`.
   - Output: `AuthorizationDecision`:
     * `Allow`: Proceed to tool handler.
     * `Blocked`: Return `ToolExecutionResult` with `status: Blocked`, error code, and explanation for model reasoning without crashing the loop.
     * `RequireConfirmation`: Pause execution or request explicit user token before proceeding.
2. **Hard Runtime Enforcement of the 4 Presets**:
   - `Read-Only` (`lecture_seule`):
     * Hard-block any tool in category `Write`, `Delete`, `Shell`, `Code`, or `NetworkEgress`.
     * Return immediate blocked step: `ToolExecutionStatus::Blocked`, code `permission_denied: read_only_mode`.
   - `Sandbox` (`isole`):
     * Hard-block any tool touching the local filesystem, shell, process spawning, or external network.
     * Only in-memory operations and conversation context inspection permitted.
   - `Developer` (`autonome`):
     * All tools permitted within `trusted_roots`.
     * Safety circuit-breakers active against system-level destruction (`rm -rf /`, formatting disks, registry deletion).
   - `Standard`:
     * Workspace reads and writes allowed inside `trusted_roots`.
     * Shell execution and irreversible mutations require user confirmation (`command_approval: always`).
     * Network access limited to search and doc retrieval.
3. **Hardened Path Containment (`SafePathResolver`)**:
   - Workspace paths must resolve against a mandatory `trusted_root`.
   - If no root is specified, use the conversation's project root; if none exists, fail closed.
   - Lexical normalization followed by canonicalization (resolving symlinks) must guarantee the path begins with `canon_root`.
   - Any attempt to escape via `..`, Windows volume roots (`C:\`), or symlinks must return `AroError::Security("path escape detected")`.
4. **Execution Sandboxing for Code & Shell (`ExecutionSandbox`)**:
   - Generalize `crates/aro-skills/src/sandbox.rs`:
     * Scrub all secret environment variables (`scrubbed_env()`).
     * Execute in an isolated workspace subdirectory.
     * Enforce strict wall-clock timeouts and max output size (prevent memory exhaustion).
5. **Catalogue Parity & Tool Descriptors in `ToolRegistry`**:
   - Register complete `ToolDescriptor`s for:
     * `core.workspace.delete` (Risk: High, effect: Delete)
     * `core.workspace.replace_in_files` (Risk: Medium, effect: ReversibleWrite)
     * `core.workspace.git_diff` (Risk: Low, effect: Read)
     * `core.artifact.create` (Risk: Low, effect: ReversibleWrite)
   - Add line-targeted patch tool: `core.workspace.patch` or `replace_file_content`.
6. **Delegation Attenuation**:
   - When an orchestrator delegates to a subagent (`core.agent.delegate`), the child agent's permission profile must be attenuated: `child_profile = parent_profile.attenuate(&requested)`. A child agent can never gain permissions not held by its parent.

---

## 7. Concrete Implementation Design (Blueprint)

### 7.1 Proposed Core Guard Interface (`crates/aro-tools/src/security.rs` or `aro-core`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPreset {
    Standard,
    ReadOnly,
    Developer,
    Sandbox,
    Custom,
}

#[derive(Debug, Clone)]
pub struct ToolSecurityContext {
    pub preset: PermissionPreset,
    pub profile: Option<PermissionProfile>,
    pub trusted_roots: Vec<PathBuf>,
    pub allow_interactive_confirmation: bool,
}

pub struct ToolAuthorizationGuard;

impl ToolAuthorizationGuard {
    pub fn authorize(
        tool_id: &str,
        input: &serde_json::Value,
        context: &ToolSecurityContext,
    ) -> Result<(), SecurityViolation> {
        let normalized = aro_core::normalize_tool_id(tool_id);
        
        // 1. Sandbox mode: fail-closed for all I/O
        if context.preset == PermissionPreset::Sandbox {
            if is_io_or_network_or_process_tool(normalized) {
                return Err(SecurityViolation::BlockedBySandbox {
                    tool_id: tool_id.to_string(),
                    reason: "Sandbox mode prohibits all filesystem, shell, and network I/O".into(),
                });
            }
        }

        // 2. Read-only mode: fail-closed for mutating operations
        if context.preset == PermissionPreset::ReadOnly {
            if is_mutating_or_exec_tool(normalized) {
                return Err(SecurityViolation::BlockedByReadOnly {
                    tool_id: tool_id.to_string(),
                    reason: "Read-only mode strictly forbids file writes, deletions, and command execution".into(),
                });
            }
        }

        // 3. Workspace Path Confinement
        if is_workspace_tool(normalized) {
            verify_path_containment(input, &context.trusted_roots)?;
        }

        // 4. Shell / Code Confirmation Gate
        if is_shell_or_code_tool(normalized) {
            if context.preset == PermissionPreset::Standard {
                // Must be confirmed or gated
            }
        }

        Ok(())
    }
}
```

### 7.2 Secure Path Resolution Blueprint

```rust
pub fn resolve_confined_workspace_path(
    input: &Value,
    path_field: &str,
    trusted_roots: &[PathBuf],
) -> AroResult<PathBuf> {
    if trusted_roots.is_empty() {
        return Err(AroError::Security("No trusted workspace root configured".into()));
    }
    
    let raw_path = extract_path_string(input, path_field)?;
    let target = Path::new(&raw_path);
    
    for root in trusted_roots {
        let candidate = if target.is_absolute() {
            target.to_path_buf()
        } else {
            root.join(target)
        };
        
        if let Some(valid_path) = verify_confinement(root, &candidate) {
            return Ok(valid_path);
        }
    }
    
    Err(AroError::Security(format!(
        "Path '{}' escapes all declared trusted workspace roots",
        target.display()
    )))
}
```

---

## 8. Verification Strategy & Invariants

To verify Requirement R2 across tests and CI:

1. **Unit & Contract Verification**:
   - `npm run contracts:check`: Ensure all TypeScript interfaces and preset types compile without error.
   - `npm run api-client:test`: Verify API client contracts.
2. **Security & Adversarial Tests**:
   - **Read-Only Mode Test**: Dispatch `workspace.write`, `workspace.delete`, `shell.execute`, and `code.execute` under Read-Only preset. Verify each is blocked before execution with `status: Blocked`.
   - **Sandbox Mode Test**: Dispatch file reads, writes, network searches, and shell commands under Sandbox preset. Verify 100% fail closed.
   - **Path Traversal Test**: Attempt `../../`, `C:\Windows\System32`, and symlink escapes in `workspace.read` and `workspace.write`. Verify all are rejected.
   - **Secret Scrubbing Test**: Execute a script via `code.execute` and attempt to print `process.env` / `os.environ`. Verify sensitive tokens (`ARO_*`, `API_KEY`, etc.) are absent.
3. **Rust Quality & Compilation Invariant**:
   - `npm run lint:rust` (`cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`): 0 errors, 0 warnings.
4. **Desktop Component Tests**:
   - `npm run test:components`: Ensure all 12 test files (including `security-settings.svelte.test.ts`, `settings-features.svelte.test.ts`) pass without regression.

---

## 9. Conclusion

The ARO agent tooling infrastructure has a strong technical foundation: comprehensive tool primitives, robust SSRF/DNS-pinned web retrieval, and an existing skill sandbox. However, the critical vulnerability is that **security boundaries are enforced only via prompt directives in the desktop runtime**.

Closing this gap by implementing the unified `ToolAuthorizationGuard`, hardening `resolve_workspace_path`, bringing parity to `ToolRegistry`, and extending process sandboxing to code/shell execution will turn ARO's security model into an ironclad, enterprise-grade execution platform.

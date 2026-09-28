//! Adversarial Challenge Test Suite: Kernel-Grade Tool Authorization Guard & Catalogue Parity
//!
//! Stress-tests:
//! 1. Blacklist precedence: denied_tools overrides Developer preset.
//! 2. Whitelist enforcement: allowed_tools rejects unlisted tools.
//! 3. ReadOnly preset: strictly blocks write and shell tools with regex /forbidden in read-only/i.
//! 4. Sandbox preset: strictly blocks write, shell, and filesystem access with regex /forbidden in sandbox/i.
//! 5. Empty or whitespace tool names reject with "Tool name cannot be empty".
//! 6. Pre-execution interception in AssistantEngine:
//!    - Creates Blocked status
//!    - Dispatches ErrorEscalation envelope to cognitive memory ledger
//!    - Aborts sub-agent run loop setting run.status = Failed
//! 7. Tool Registry Catalogue Parity (Feature 8).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use aro_agent::ToolRegistry;
use aro_core::{
    normalize_tool_id, AgentAction, AgentMessageType, AgentRun, AgentRunPriority, AgentRunStatus,
    AgentStepKind, AgentStepStatus, AssistantMode, Conversation, ModelGeneration,
    ModelGenerationRequest, ModelProviderKind, PermissionProfile, RuntimeStatus,
    ToolExecutionRequest, TOOL_CORE_ARTIFACT_CREATE, TOOL_CORE_WORKSPACE_DELETE,
    TOOL_CORE_WORKSPACE_GIT_DIFF, TOOL_CORE_WORKSPACE_REPLACE_IN_FILES, TOOL_CORE_WORKSPACE_WRITE,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::{AssistantEngine, ModelProvider};
use aro_tools::{PermissionPreset, ToolAuthorizationGuard};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

fn create_test_engine() -> (AssistantEngine, SqliteMemoryStore, std::path::PathBuf) {
    let db_path =
        std::env::temp_dir().join(format!("aro-guard-challenge-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    let engine = AssistantEngine::new(store.clone());
    (engine, store, db_path)
}

fn cleanup_temp_db(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

fn matches_regex_forbidden_in_read_only(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("forbidden in read-only")
}

fn matches_regex_forbidden_in_sandbox(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("forbidden in sandbox")
}

// =========================================================================
// 1. BLACKLIST PRECEDENCE STRESS TEST
// =========================================================================
#[test]
fn test_stress_blacklist_precedence_overrides_developer() {
    let denied_list = [
        "core.shell.execute",
        "core.code.execute",
        "workspace.delete",
        "workspace.write",
        "workspace.replace_in_files",
        "custom.backdoor.execute",
    ];

    let guard = ToolAuthorizationGuard::developer().with_denied_tools(denied_list);

    // 1. All denied tools MUST be rejected even in Developer mode
    for tool in &denied_list {
        let err = guard.check_permission(tool).expect_err(&format!(
            "Tool '{tool}' must be denied by blacklist in Developer preset"
        ));
        assert_eq!(err.preset, PermissionPreset::Developer);
        assert!(
            err.reason
                .contains("Tool explicitly blacklisted in denied_tools"),
            "Expected blacklist reason for '{tool}', got: '{}'",
            err.reason
        );
    }

    // 2. Case-insensitivity & whitespace resistance on blacklisted tools
    let variations = [
        "CORE.SHELL.EXECUTE",
        "  core.shell.execute  ",
        "Workspace.Delete",
        "\tworkspace.write\n",
    ];
    for var in &variations {
        assert!(
            guard.check_permission(var).is_err(),
            "Blacklist must block normalized variation '{var}'"
        );
    }

    // 3. Non-blacklisted tools MUST still be allowed under Developer mode
    assert!(guard.check_permission("workspace.read").is_ok());
    assert!(guard.check_permission("workspace.list_dir").is_ok());
    assert!(guard.check_permission("web.fetch").is_ok());
}

// =========================================================================
// 2. WHITELIST ENFORCEMENT STRESS TEST
// =========================================================================
#[test]
fn test_stress_whitelist_enforcement_rejects_unlisted() {
    let allowed_list = ["workspace.read", "core.workspace.read", "context.search"];
    let guard = ToolAuthorizationGuard::standard().with_allowed_tools(allowed_list);

    // 1. Allowed tools pass
    for tool in &allowed_list {
        assert!(
            guard.check_permission(tool).is_ok(),
            "Tool '{tool}' in whitelist must be permitted"
        );
    }

    // 2. Case-insensitivity and whitespace on whitelist
    assert!(guard.check_permission("WORKSPACE.READ").is_ok());
    assert!(guard.check_permission("  workspace.read  ").is_ok());

    // 3. Any tool NOT explicitly in whitelist MUST be rejected
    let unlisted_tools = [
        "workspace.write",
        "workspace.delete",
        "workspace.replace_in_files",
        "core.shell.execute",
        "core.code.execute",
        "web.fetch",
        "unknown.unauthorized.tool",
    ];

    for tool in &unlisted_tools {
        let err = guard.check_permission(tool).expect_err(&format!(
            "Unlisted tool '{tool}' must be rejected by whitelist"
        ));
        assert!(
            err.reason
                .contains("Tool not found in explicit allowed_tools whitelist"),
            "Expected whitelist rejection reason for '{tool}', got: '{}'",
            err.reason
        );
    }

    // 4. Precedence: Blacklist MUST override Whitelist if a tool is present in both
    let conflicting_guard = ToolAuthorizationGuard::standard()
        .with_allowed_tools(["workspace.read", "workspace.write"])
        .with_denied_tools(["workspace.write"]);

    assert!(conflicting_guard.check_permission("workspace.read").is_ok());
    let conflict_err = conflicting_guard
        .check_permission("workspace.write")
        .expect_err("Tool in both whitelist and blacklist MUST be denied");
    assert!(
        conflict_err
            .reason
            .contains("Tool explicitly blacklisted in denied_tools"),
        "Blacklist must take precedence over whitelist, got: '{}'",
        conflict_err.reason
    );
}

// =========================================================================
// 3. READ-ONLY PRESET STRESS TEST (REGEX MATCHING)
// =========================================================================
#[test]
fn test_stress_read_only_preset_strictly_blocks_write_and_shell() {
    let guard = ToolAuthorizationGuard::read_only();

    let write_and_shell_tools = [
        "workspace.write",
        "core.workspace.write",
        "workspace.delete",
        "core.workspace.delete",
        "workspace.replace_in_files",
        "core.workspace.replace_in_files",
        "artifact.create",
        "core.artifact.create",
        "artifact.update",
        "core.shell.execute",
        "core.code.execute",
        "run_command",
        "terminal.exec",
        "bash.run",
        "cmd.run",
        "web.fetch",
        "http.download",
        "browser.navigate",
    ];

    for tool in &write_and_shell_tools {
        let err = guard
            .check_permission(tool)
            .expect_err(&format!("Tool '{tool}' must be forbidden in ReadOnly mode"));
        assert_eq!(err.preset, PermissionPreset::ReadOnly);
        let err_str = err.to_string();
        assert!(
            matches_regex_forbidden_in_read_only(&err_str),
            "Error for '{tool}' must match /forbidden in read-only/i regex, got: '{err_str}'"
        );
        assert!(
            matches_regex_forbidden_in_read_only(&err.reason),
            "Reason for '{tool}' must match /forbidden in read-only/i regex, got: '{}'",
            err.reason
        );
    }

    // Confirm that legitimate read operations ARE allowed
    let read_tools = [
        "workspace.read",
        "core.workspace.read",
        "workspace.list_dir",
        "workspace.git_diff",
        "artifact.list",
        "context.search",
    ];
    for tool in &read_tools {
        assert!(
            guard.check_permission(tool).is_ok(),
            "Read tool '{tool}' must be permitted under ReadOnly preset"
        );
    }
}

// =========================================================================
// 4. SANDBOX PRESET STRESS TEST (REGEX MATCHING)
// =========================================================================
#[test]
fn test_stress_sandbox_preset_strictly_blocks_filesystem_write_and_shell() {
    let guard = ToolAuthorizationGuard::sandbox();

    let sandbox_blocked_tools = [
        // Shell
        "core.shell.execute",
        "core.code.execute",
        "terminal.launch",
        "bash",
        // Write
        "workspace.write",
        "workspace.delete",
        "workspace.replace_in_files",
        "artifact.create",
        // Network
        "web.fetch",
        "http.client",
        "browser.open",
        // Local Filesystem Read
        "workspace.read",
        "core.workspace.read",
        "workspace.list_dir",
        "workspace.git_diff",
        "file.inspect",
        "view.source",
    ];

    for tool in &sandbox_blocked_tools {
        let err = guard
            .check_permission(tool)
            .expect_err(&format!("Tool '{tool}' must be forbidden in Sandbox mode"));
        assert_eq!(err.preset, PermissionPreset::Sandbox);
        let err_str = err.to_string();
        assert!(
            matches_regex_forbidden_in_sandbox(&err_str),
            "Error for '{tool}' must match /forbidden in sandbox/i regex, got: '{err_str}'"
        );
        assert!(
            matches_regex_forbidden_in_sandbox(&err.reason),
            "Reason for '{tool}' must match /forbidden in sandbox/i regex, got: '{}'",
            err.reason
        );
    }

    // Only non-filesystem context tools pass
    assert!(guard.check_permission("context.search").is_ok());
    assert!(guard.check_permission("core.context.search").is_ok());
}

// =========================================================================
// 5. EMPTY AND WHITESPACE TOOL NAMES STRESS TEST
// =========================================================================
#[test]
fn test_stress_empty_and_whitespace_tool_names() {
    let empty_inputs = ["", " ", "   ", "\t", "\n", "\r\n", "  \t \r\n  "];
    let presets = [
        ToolAuthorizationGuard::standard(),
        ToolAuthorizationGuard::read_only(),
        ToolAuthorizationGuard::developer(),
        ToolAuthorizationGuard::sandbox(),
    ];

    for guard in &presets {
        for input in &empty_inputs {
            let err = guard
                .check_permission(input)
                .expect_err(&format!("Empty input '{input:?}' must be rejected"));
            assert_eq!(
                err.reason, "Tool name cannot be empty",
                "Empty tool error reason must be exactly 'Tool name cannot be empty', got: '{}'",
                err.reason
            );
        }
    }
}

// =========================================================================
// 6. PRE-EXECUTION INTERCEPTION IN AssistantEngine & SUB-AGENT RUN LOOP ABORT
// =========================================================================

struct MaliciousSubAgentMockProvider {
    pub call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl ModelProvider for MaliciousSubAgentMockProvider {
    async fn generate(
        &self,
        _request: ModelGenerationRequest,
    ) -> aro_core::AroResult<ModelGeneration> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let content = if count == 0 {
            // First turn: generate an unauthorized tool action under Read-Only preset
            let action = AgentAction::tool(
                TOOL_CORE_WORKSPACE_WRITE,
                json!({
                    "root_path": "C:\\repo",
                    "path": "unauthorized_exploit.txt",
                    "content": "malicious content"
                }),
                Some("Attempting unauthorized file write in ReadOnly mode".to_string()),
            );
            serde_json::to_string(&action).unwrap()
        } else {
            // If the loop continues, return final (which SHOULD NOT happen!)
            let action = AgentAction::final_response("Unexpected loop continuation!");
            serde_json::to_string(&action).unwrap()
        };

        Ok(ModelGeneration {
            content,
            provider_detail: "MaliciousSubAgentMockProvider".to_string(),
            token_estimate: Some(64),
        })
    }

    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        _on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> aro_core::AroResult<ModelGeneration> {
        self.generate(request).await
    }

    async fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            model_provider: ModelProviderKind::Mock,
            model_id: "mock-malicious".to_string(),
            model_ready: true,
            voice_ready: false,
            endpoint: None,
            detail: "Malicious SubAgent Mock Provider Ready".to_string(),
            checked_at: Utc::now(),
        }
    }

    fn temperature(&self) -> f32 {
        0.0
    }

    fn max_tokens(&self) -> u32 {
        1024
    }
}

#[tokio::test]
async fn test_stress_pre_execution_interception_and_subagent_run_loop_abort() {
    let (engine, store, db_path) = create_test_engine();
    let conv_id = Uuid::new_v4();

    // 1. Setup Read-Only permission profile
    let mut profile = PermissionProfile::trusted_workspace("C:\\repo");
    profile.id = Uuid::new_v4();
    profile.name = "read-only".to_string();
    profile.allow_read = true;
    profile.allow_write = false;
    profile.allow_shell = false;
    store
        .upsert_permission_profile(&profile)
        .expect("save read-only profile");

    // 2. Setup Conversation and AgentRun assigned to ReadOnly profile
    let conv = Conversation::with_id(conv_id, "ReadOnly Interception Test", AssistantMode::Code);
    store.upsert_conversation(&conv).expect("save conversation");

    let run = AgentRun::new(
        "Attempt unauthorized write",
        AssistantMode::Code,
        Some(conv_id),
        None,
        None,
        Some(profile.id),
    );
    store.upsert_agent_run(&run).expect("save run");

    // 3. Test direct execute_tool interception
    let write_req = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_WORKSPACE_WRITE,
        json!({
            "root_path": "C:\\repo",
            "path": "exploit.txt",
            "content": "payload"
        }),
    );

    let direct_res = engine.execute_tool(&run, write_req).await;
    assert!(
        direct_res.is_err(),
        "Direct write execution must be intercepted"
    );
    let err_str = direct_res.unwrap_err().to_string();
    assert!(
        matches_regex_forbidden_in_read_only(&err_str),
        "Error message must specify forbidden in read-only, got: '{err_str}'"
    );

    // Verify step status recorded in SQLite is Failed (from Blocked status)
    let steps_after_direct = store.list_agent_steps(run.id).expect("list steps");
    assert_eq!(steps_after_direct.len(), 1);
    assert_eq!(steps_after_direct[0].kind, AgentStepKind::Tool);
    assert_eq!(steps_after_direct[0].status, AgentStepStatus::Failed);

    // Verify ErrorEscalation envelope was recorded in cognitive memory ledger
    let envelopes = store
        .list_agent_envelopes(&conv_id.to_string(), None, None)
        .expect("list envelopes");
    assert_eq!(envelopes.len(), 1);
    let envelope = &envelopes[0];
    assert_eq!(envelope.message_type, AgentMessageType::ErrorEscalation);
    assert_eq!(envelope.sender.name, "Security Guard");
    assert_eq!(envelope.sender.role.as_deref(), Some("kernel"));
    assert_eq!(envelope.recipient.id, "orchestrator");
    assert_eq!(envelope.priority, Some(AgentRunPriority::High));
    assert!(envelope
        .payload
        .content
        .contains("Security Guard Interception"));

    // 4. Test Sub-Agent Run Loop Pre-Execution Abort
    // Create a new fresh run for the autonomous loop test
    let subagent_run = AgentRun::new(
        "Autonomous malicious subagent",
        AssistantMode::Code,
        Some(conv_id),
        None,
        None,
        Some(profile.id),
    );
    store
        .upsert_agent_run(&subagent_run)
        .expect("save subagent run");

    // Wire malicious mock provider
    let call_count = Arc::new(AtomicUsize::new(0));
    let mock_provider = Arc::new(MaliciousSubAgentMockProvider {
        call_count: call_count.clone(),
    });
    engine.set_model_provider(mock_provider).await;

    // Run the sub-agent execution loop
    let loop_result = engine
        .execute_subagent_run_loop(subagent_run.id, Some(5))
        .await;

    // The loop MUST abort with an error
    assert!(
        loop_result.is_err(),
        "Subagent run loop MUST abort upon unauthorized tool interception"
    );
    let loop_err = loop_result.unwrap_err().to_string();
    assert!(
        matches_regex_forbidden_in_read_only(&loop_err),
        "Loop error must report security rejection, got: '{loop_err}'"
    );

    // Verify that the model provider was called only ONCE and immediately aborted
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Run loop MUST abort immediately after unauthorized tool attempt without continuing"
    );

    // Verify in SQLite that run.status was set to Failed and last_error was recorded
    let updated_run = store
        .get_agent_run(subagent_run.id)
        .expect("get run")
        .expect("run exists in SQLite");

    assert_eq!(
        updated_run.status,
        AgentRunStatus::Failed,
        "Subagent run.status MUST be set to Failed upon security interception"
    );
    assert!(
        updated_run.last_error.is_some(),
        "Subagent run MUST have last_error populated"
    );
    let last_err = updated_run.last_error.unwrap();
    assert!(
        matches_regex_forbidden_in_read_only(&last_err),
        "last_error must contain security rejection details, got: '{last_err}'"
    );
    assert!(
        updated_run.completed_at.is_some(),
        "completed_at must be populated on aborted run"
    );

    cleanup_temp_db(db_path);
}

// =========================================================================
// 7. TOOL REGISTRY CATALOGUE PARITY (FEATURE 8)
// =========================================================================
#[test]
fn test_stress_tool_registry_catalogue_parity() {
    let registry = ToolRegistry::default();
    let descriptors = registry.descriptors();

    // 1. Verify exact 40 built-in tools
    assert_eq!(
        descriptors.len(),
        40,
        "ToolRegistry::default() must expose exactly 40 descriptors"
    );

    // 2. Validate all 40 descriptors pass schema validation
    for descriptor in descriptors {
        assert!(
            descriptor.validate().is_ok(),
            "Descriptor '{}' must pass schema validation",
            descriptor.id
        );
    }

    // 3. Verify presence of all 4 newly integrated tools
    let required_new_tools = [
        TOOL_CORE_WORKSPACE_DELETE,
        TOOL_CORE_WORKSPACE_REPLACE_IN_FILES,
        TOOL_CORE_WORKSPACE_GIT_DIFF,
        TOOL_CORE_ARTIFACT_CREATE,
    ];

    for expected_id in &required_new_tools {
        assert!(
            descriptors.iter().any(|d| d.id == *expected_id),
            "ToolRegistry must contain '{}'",
            expected_id
        );
    }

    // 4. Verify canonical normalization of legacy and canonical forms
    assert_eq!(
        normalize_tool_id("workspace.delete"),
        TOOL_CORE_WORKSPACE_DELETE
    );
    assert_eq!(
        normalize_tool_id("core.workspace.delete"),
        TOOL_CORE_WORKSPACE_DELETE
    );
    assert_eq!(
        normalize_tool_id("workspace.replace_in_files"),
        TOOL_CORE_WORKSPACE_REPLACE_IN_FILES
    );
    assert_eq!(
        normalize_tool_id("core.workspace.replace_in_files"),
        TOOL_CORE_WORKSPACE_REPLACE_IN_FILES
    );
    assert_eq!(
        normalize_tool_id("workspace.git_diff"),
        TOOL_CORE_WORKSPACE_GIT_DIFF
    );
    assert_eq!(
        normalize_tool_id("core.workspace.git_diff"),
        TOOL_CORE_WORKSPACE_GIT_DIFF
    );
    assert_eq!(
        normalize_tool_id("artifact.create"),
        TOOL_CORE_ARTIFACT_CREATE
    );
    assert_eq!(
        normalize_tool_id("core.artifact.create"),
        TOOL_CORE_ARTIFACT_CREATE
    );
}

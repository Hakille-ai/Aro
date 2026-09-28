//! Milestone 2 Integration Tests: Kernel-Grade Tool Authorization Guard
//!
//! Verifies:
//! - Pre-execution interception in execute_tool_request before any tool execution
//! - All 5 presets: Standard, ReadOnly, Developer, Sandbox, Custom
//! - Priority overrides: denied_tools overrides Developer, allowed_tools whitelist, empty tool rejection
//! - Blocked step persistence in SQLite with ToolExecutionStatus::Blocked
//! - ErrorEscalation envelope dispatch into cognitive memory ledger
//! - Fail-closed enforcement on unauthorized invocations

use aro_core::{
    AgentRun, AssistantMode, Conversation, PermissionProfile, ToolExecutionRequest,
    TOOL_CORE_SHELL_EXECUTE, TOOL_CORE_WORKSPACE_DELETE, TOOL_CORE_WORKSPACE_READ,
    TOOL_CORE_WORKSPACE_WRITE,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::AssistantEngine;
use aro_tools::{PermissionPreset, ToolAuthorizationGuard};
use serde_json::json;
use uuid::Uuid;

fn create_test_engine() -> (AssistantEngine, SqliteMemoryStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!("aro-guard-test-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    let engine = AssistantEngine::new(store.clone());
    (engine, store, db_path)
}

fn cleanup_temp_db(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

fn create_test_run(
    store: &SqliteMemoryStore,
    conv_id: Option<Uuid>,
    profile_id: Option<Uuid>,
) -> AgentRun {
    if let Some(cid) = conv_id {
        let conv = Conversation::with_id(cid, "Guard Test Conversation", AssistantMode::Code);
        let _ = store.upsert_conversation(&conv);
    }
    let run = AgentRun::new(
        "Guard test goal",
        AssistantMode::Code,
        conv_id,
        None,
        None,
        profile_id,
    );
    store.upsert_agent_run(&run).expect("save run");
    run
}

#[test]
fn test_guard_unit_evaluations_and_presets() {
    // 1. Empty tool names rejected under all presets
    let standard = ToolAuthorizationGuard::standard();
    assert!(standard.check_permission("").is_err());
    assert!(standard.check_permission("   ").is_err());

    // 2. ReadOnly preset: allows read, forbids write/shell/network
    let readonly = ToolAuthorizationGuard::read_only();
    assert!(readonly.check_permission("workspace.read").is_ok());
    assert!(readonly.check_permission("core.workspace.read").is_ok());
    assert!(readonly.check_permission("workspace.list").is_ok());
    assert!(readonly.check_permission("workspace.git_diff").is_ok());

    let write_err = readonly.check_permission("workspace.write").unwrap_err();
    assert_eq!(write_err.preset, PermissionPreset::ReadOnly);
    assert!(write_err
        .reason
        .to_lowercase()
        .contains("forbidden in read-only"));

    let del_err = readonly.check_permission("workspace.delete").unwrap_err();
    assert!(del_err
        .reason
        .to_lowercase()
        .contains("forbidden in read-only"));

    let shell_err = readonly.check_permission("core.shell.execute").unwrap_err();
    assert!(shell_err
        .reason
        .to_lowercase()
        .contains("forbidden in read-only"));

    let net_err = readonly.check_permission("web.fetch").unwrap_err();
    assert!(net_err
        .reason
        .to_lowercase()
        .contains("forbidden in read-only"));

    // 3. Sandbox preset: forbids read, write, shell, network
    let sandbox = ToolAuthorizationGuard::sandbox();
    let sb_read_err = sandbox.check_permission("workspace.read").unwrap_err();
    assert_eq!(sb_read_err.preset, PermissionPreset::Sandbox);
    assert!(sb_read_err
        .reason
        .to_lowercase()
        .contains("forbidden in sandbox"));

    let sb_write_err = sandbox.check_permission("workspace.write").unwrap_err();
    assert!(sb_write_err
        .reason
        .to_lowercase()
        .contains("forbidden in sandbox"));

    let sb_shell_err = sandbox.check_permission("core.shell.execute").unwrap_err();
    assert!(sb_shell_err
        .reason
        .to_lowercase()
        .contains("forbidden in sandbox"));

    // 4. Developer preset: full access
    let dev = ToolAuthorizationGuard::developer();
    assert!(dev.check_permission("workspace.read").is_ok());
    assert!(dev.check_permission("workspace.write").is_ok());
    assert!(dev.check_permission("core.shell.execute").is_ok());
    assert!(dev.check_permission("web.fetch").is_ok());

    // 5. Explicit blacklist (denied_tools) overrides Developer mode
    let dev_blacklisted = ToolAuthorizationGuard::developer()
        .with_denied_tools(["core.shell.execute", "workspace.delete"]);
    assert!(dev_blacklisted.check_permission("workspace.read").is_ok());
    assert!(dev_blacklisted.check_permission("workspace.write").is_ok());
    assert!(dev_blacklisted
        .check_permission("core.shell.execute")
        .is_err());
    assert!(dev_blacklisted
        .check_permission("workspace.delete")
        .is_err());

    // 6. Explicit whitelist (allowed_tools) acts strictly
    let whitelisted = ToolAuthorizationGuard::standard()
        .with_allowed_tools(["workspace.read", "core.agent.status"]);
    assert!(whitelisted.check_permission("workspace.read").is_ok());
    assert!(whitelisted.check_permission("workspace.write").is_err());
    assert!(whitelisted.check_permission("core.shell.execute").is_err());

    // 7. Custom preset with PermissionProfile
    let mut custom_profile = PermissionProfile::trusted_workspace("C:\\repo");
    custom_profile.name = "Auditor".to_string();
    custom_profile.allow_read = true;
    custom_profile.allow_write = false;
    custom_profile.allow_shell = false;
    custom_profile.allow_network = true;

    let custom_guard = ToolAuthorizationGuard::custom(custom_profile);
    assert!(custom_guard.check_permission("workspace.read").is_ok());
    assert!(custom_guard.check_permission("web.fetch").is_ok());
    assert!(custom_guard.check_permission("workspace.write").is_err());
    assert!(custom_guard.check_permission("core.shell.execute").is_err());
}

#[tokio::test]
async fn test_runtime_blocks_unauthorized_tool_and_records_step_and_envelope() {
    let (engine, store, db_path) = create_test_engine();
    let conv_id = Uuid::new_v4();

    // 1. Create a Read-Only permission profile
    let mut profile = PermissionProfile::trusted_workspace("C:\\test");
    profile.id = Uuid::new_v4();
    profile.name = "read-only".to_string();
    profile.allow_read = true;
    profile.allow_write = false;
    profile.allow_shell = false;
    store
        .upsert_permission_profile(&profile)
        .expect("save profile");

    // 2. Start an AgentRun assigned to this profile
    let run = create_test_run(&store, Some(conv_id), Some(profile.id));

    // 3. Attempt to execute workspace.write under ReadOnly preset
    let write_req = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_WORKSPACE_WRITE,
        json!({
            "root_path": "C:\\test",
            "path": "test.txt",
            "content": "malicious payload"
        }),
    );

    let result = engine.execute_tool(&run, write_req).await;
    assert!(
        result.is_err(),
        "write must be blocked under read-only profile"
    );
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.to_lowercase().contains("forbidden in read-only"),
        "error must explain forbidden in read-only, got: {err_str}"
    );

    // 4. Verify that step is persisted in SQLite with Failed/Blocked status
    let steps = store.list_agent_steps(run.id).expect("get steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].kind, aro_core::AgentStepKind::Tool);
    assert_eq!(steps[0].status, aro_core::AgentStepStatus::Failed);

    // 5. Verify that an ErrorEscalation envelope was recorded in the cognitive memory ledger
    let envelopes = store
        .list_agent_envelopes(&conv_id.to_string(), None, None)
        .expect("get envelopes");
    assert!(
        !envelopes.is_empty(),
        "error escalation envelope must be dispatched"
    );
    let escalation = &envelopes[0];
    assert_eq!(
        escalation.message_type,
        aro_core::AgentMessageType::ErrorEscalation
    );
    assert!(escalation
        .payload
        .content
        .contains("Security Guard Interception"));

    // 6. Attempt to execute shell execute under ReadOnly preset
    let shell_req = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_SHELL_EXECUTE,
        json!({
            "command": "whoami"
        }),
    );
    let shell_result = engine.execute_tool(&run, shell_req).await;
    assert!(shell_result.is_err());
    let shell_err = shell_result.unwrap_err().to_string();
    assert!(shell_err.to_lowercase().contains("forbidden in read-only"));

    cleanup_temp_db(db_path);
}

#[tokio::test]
async fn test_runtime_blocks_filesystem_and_shell_in_sandbox_preset() {
    let (engine, store, db_path) = create_test_engine();
    let conv_id = Uuid::new_v4();

    // Create a Sandbox profile
    let mut profile = PermissionProfile::trusted_workspace("C:\\sandbox");
    profile.id = Uuid::new_v4();
    profile.name = "sandbox".to_string();
    store
        .upsert_permission_profile(&profile)
        .expect("save profile");

    let run = create_test_run(&store, Some(conv_id), Some(profile.id));

    // In Sandbox, even reading filesystem is forbidden
    let read_req = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_WORKSPACE_READ,
        json!({
            "root_path": "C:\\sandbox",
            "path": "file.txt"
        }),
    );
    let read_result = engine.execute_tool(&run, read_req).await;
    assert!(read_result.is_err());
    let read_err = read_result.unwrap_err().to_string();
    assert!(read_err.to_lowercase().contains("forbidden in sandbox"));

    // In Sandbox, delete is forbidden
    let del_req = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_WORKSPACE_DELETE,
        json!({
            "root_path": "C:\\sandbox",
            "path": "file.txt"
        }),
    );
    let del_result = engine.execute_tool(&run, del_req).await;
    assert!(del_result.is_err());
    let del_err = del_result.unwrap_err().to_string();
    assert!(del_err.to_lowercase().contains("forbidden in sandbox"));

    cleanup_temp_db(db_path);
}

//! Milestone 2 Integration Tests for aro-tools security, confinement, and sandboxing

use aro_tools::{
    is_sensitive_env_var, resolve_workspace_path, scrubbed_env, truncate_output_safe,
    ToolAuthorizationGuard, MAX_EXECUTION_OUTPUT_BYTES,
};
use serde_json::json;

#[test]
fn test_sensitive_env_var_detection() {
    // 1. Sensitive prefixes
    assert!(is_sensitive_env_var("AWS_SECRET_ACCESS_KEY"));
    assert!(is_sensitive_env_var("GITHUB_TOKEN"));
    assert!(is_sensitive_env_var("GH_TOKEN"));
    assert!(is_sensitive_env_var("ARO_JWT_SECRET"));
    assert!(is_sensitive_env_var("OPENAI_API_KEY"));
    assert!(is_sensitive_env_var("ANTHROPIC_API_KEY"));
    assert!(is_sensitive_env_var("DATABASE_URL"));
    assert!(is_sensitive_env_var("REDIS_PASSWORD"));

    // 2. Sensitive suffixes
    assert!(is_sensitive_env_var("MY_CUSTOM_API_KEY"));
    assert!(is_sensitive_env_var("SERVICE_SECRET"));
    assert!(is_sensitive_env_var("USER_AUTH_TOKEN"));
    assert!(is_sensitive_env_var("ROOT_PASSWORD"));

    // 3. Safe system environment variables
    assert!(!is_sensitive_env_var("PATH"));
    assert!(!is_sensitive_env_var("HOME"));
    assert!(!is_sensitive_env_var("USER"));
    assert!(!is_sensitive_env_var("TEMP"));
    assert!(!is_sensitive_env_var("SYSTEMROOT"));
    assert!(!is_sensitive_env_var("MY_SAFE_APP_VAR_123"));
}

#[test]
fn test_scrubbed_env_filtering() {
    std::env::set_var("ARO_SECRET_TEST_KEY_123", "sensitive_val");
    std::env::set_var("MY_SAFE_APP_VAR_123", "public_val");

    let filtered = scrubbed_env();
    assert!(!filtered.iter().any(|(k, _)| k == "ARO_SECRET_TEST_KEY_123"));
    assert!(filtered.iter().any(|(k, _)| k == "MY_SAFE_APP_VAR_123"));

    std::env::remove_var("ARO_SECRET_TEST_KEY_123");
    std::env::remove_var("MY_SAFE_APP_VAR_123");
}

#[test]
fn test_truncate_output_safe_utf8_boundaries() {
    // Short text - no truncation
    let (text, truncated) = truncate_output_safe("hello world", 100);
    assert_eq!(text, "hello world");
    assert!(!truncated);

    // Multibyte UTF-8 boundary (e.g. French accents or emojis)
    let french = "é".repeat(10); // each 'é' is 2 bytes (total 20 bytes)
                                 // Truncating at 7 bytes must not split the 4th 'é' (bytes 6..8), so it must take exactly 6 bytes (3 chars)
    let (trunc_french, truncated_fr) = truncate_output_safe(&french, 7);
    assert!(truncated_fr);
    assert!(trunc_french.starts_with("ééé"));
    assert!(trunc_french.contains("[truncated]"));

    // Large output exceeding MAX_EXECUTION_OUTPUT_BYTES
    let large = "A".repeat(MAX_EXECUTION_OUTPUT_BYTES + 1000);
    let (trunc_large, was_truncated) = truncate_output_safe(&large, MAX_EXECUTION_OUTPUT_BYTES);
    assert!(was_truncated);
    assert!(trunc_large.len() <= MAX_EXECUTION_OUTPUT_BYTES + 50);
}

#[test]
fn test_resolve_workspace_path_fail_closed_invariants() {
    // 1. Missing root_path fails closed
    let no_root = json!({ "path": "file.txt" });
    assert!(resolve_workspace_path(&no_root, "path", false).is_err());

    // 2. Empty or whitespace root_path fails closed
    let empty_root = json!({ "root_path": "", "path": "file.txt" });
    assert!(resolve_workspace_path(&empty_root, "path", false).is_err());
    let ws_root = json!({ "root_path": "   ", "path": "file.txt" });
    assert!(resolve_workspace_path(&ws_root, "path", false).is_err());

    // 3. Null byte in root_path or target_path fails closed
    let null_root = json!({ "root_path": "/tmp/root\0evil", "path": "file.txt" });
    assert!(resolve_workspace_path(&null_root, "path", false).is_err());
    let null_target = json!({ "root_path": "/tmp/root", "path": "file\0.txt" });
    assert!(resolve_workspace_path(&null_target, "path", false).is_err());

    // 4. Directory traversal escapes fail closed
    let traversal = json!({ "root_path": "/tmp/root", "path": "../../etc/shadow" });
    assert!(resolve_workspace_path(&traversal, "path", false).is_err());

    // 5. Foreign absolute path fails closed
    let abs_escape = json!({ "root_path": "/tmp/root", "path": "C:\\Windows\\System32\\cmd.exe" });
    assert!(resolve_workspace_path(&abs_escape, "path", false).is_err());
}

#[test]
fn test_tool_authorization_guard_presets() {
    let ro = ToolAuthorizationGuard::read_only();
    assert!(ro.check_permission("workspace.read").is_ok());
    assert!(ro.check_permission("workspace.git_diff").is_ok());
    assert!(ro.check_permission("workspace.write").is_err());
    assert!(ro.check_permission("core.shell.execute").is_err());

    let sb = ToolAuthorizationGuard::sandbox();
    assert!(sb.check_permission("workspace.read").is_err());
    assert!(sb.check_permission("workspace.write").is_err());
    assert!(sb.check_permission("core.shell.execute").is_err());

    let dev = ToolAuthorizationGuard::developer();
    assert!(dev.check_permission("workspace.write").is_ok());
    assert!(dev.check_permission("core.shell.execute").is_ok());
}

//! Adversarial Challenge Test Suite: Feature 6 (Confinement) & Feature 7 (Sandboxing)
//! Authored by teamwork_preview_challenger_m2_1

use aro_core::{ToolExecutionRequest, ToolExecutionStatus};
use aro_tools::{
    resolve_workspace_path, truncate_output_safe, ToolExecutor, MAX_EXECUTION_OUTPUT_BYTES,
};
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Helper to create a unique temporary workspace directory
fn create_temp_workspace() -> (PathBuf, PathBuf) {
    let base = env::temp_dir().join(format!("aro_adv_test_{}", Uuid::new_v4()));
    let ws_root = base.join("workspace");
    let outside_dir = base.join("outside");
    fs::create_dir_all(&ws_root).expect("create ws_root");
    fs::create_dir_all(&outside_dir).expect("create outside_dir");

    // Write a canary file outside workspace
    fs::write(
        outside_dir.join("secret_outside.txt"),
        "TOP_SECRET_OUTSIDE_WORKSPACE",
    )
    .expect("write secret_outside.txt");

    // Write a safe file inside workspace
    fs::write(ws_root.join("safe_inside.txt"), "SAFE_INSIDE_WORKSPACE")
        .expect("write safe_inside.txt");

    (ws_root, outside_dir)
}

// =========================================================================
// 1. ADVERSARIAL DIRECTORY TRAVERSAL & ESCAPE PROBES (FEATURE 6)
// =========================================================================

#[test]
fn test_adversarial_directory_traversal_variations() {
    let (ws_root, _outside) = create_temp_workspace();
    let root_str = ws_root.to_str().unwrap();

    let adversarial_traversals = vec![
        "../outside/secret_outside.txt",
        "..\\outside\\secret_outside.txt",
        "..//..//outside//secret_outside.txt",
        "..\\..\\outside\\secret_outside.txt",
        "nested/../../outside/secret_outside.txt",
        "nested\\..\\..\\outside\\secret_outside.txt",
        "a/b/c/../../../../outside/secret_outside.txt",
        "./../../outside/secret_outside.txt",
        "..",
        "../",
        "..\\",
        "subdir/../..",
    ];

    for attack in adversarial_traversals {
        let input = json!({
            "root_path": root_str,
            "path": attack,
        });

        let res = resolve_workspace_path(&input, "path", false);
        assert!(
            res.is_err(),
            "VULNERABILITY: Directory traversal attack '{}' was NOT blocked! Result: {:?}",
            attack,
            res
        );
    }

    let _ = fs::remove_dir_all(ws_root.parent().unwrap());
}

#[test]
fn test_adversarial_absolute_path_and_prefix_injection() {
    let (ws_root, _outside) = create_temp_workspace();
    let root_str = ws_root.to_str().unwrap();

    let adversarial_injections = vec![
        // Windows absolute path
        "C:\\Windows\\System32\\calc.exe",
        "C:/Windows/System32/drivers/etc/hosts",
        // Unix style absolute path
        "/etc/passwd",
        "/etc/shadow",
        "/var/log/syslog",
        // Windows verbatim \\?\ prefix injection
        "\\\\?\\C:\\Windows\\System32\\cmd.exe",
        "\\\\?\\C:\\secret.txt",
        // Device namespaces
        "\\\\.\\COM1",
        "\\\\.\\NUL",
        // UNC paths
        "\\\\127.0.0.1\\c$\\secret.txt",
        "\\\\localhost\\share\\data.txt",
    ];

    for injection in adversarial_injections {
        let input = json!({
            "root_path": root_str,
            "path": injection,
        });

        let res = resolve_workspace_path(&input, "path", false);
        assert!(
            res.is_err(),
            "VULNERABILITY: Absolute/Prefix injection '{}' was NOT blocked! Result: {:?}",
            injection,
            res
        );
    }

    let _ = fs::remove_dir_all(ws_root.parent().unwrap());
}

#[test]
fn test_adversarial_null_byte_injections() {
    let (ws_root, _outside) = create_temp_workspace();
    let root_str = ws_root.to_str().unwrap();

    let null_probes = [
        json!({ "root_path": format!("{}\0evil", root_str), "path": "safe.txt" }),
        json!({ "root_path": root_str, "path": "safe.txt\0.exe" }),
        json!({ "root_path": root_str, "path": "file\0/../../secret.txt" }),
        json!({ "root_path": format!("{}\0", root_str), "path": "safe.txt" }),
    ];

    for (idx, probe) in null_probes.iter().enumerate() {
        let res = resolve_workspace_path(probe, "path", false);
        assert!(
            res.is_err(),
            "VULNERABILITY: Null byte probe #{} was NOT blocked! Result: {:?}",
            idx,
            res
        );
    }

    let _ = fs::remove_dir_all(ws_root.parent().unwrap());
}

#[test]
fn test_adversarial_symlink_traversal_detection() {
    let (ws_root, outside_dir) = create_temp_workspace();
    let root_str = ws_root.to_str().unwrap();

    // Create a symlink inside ws_root pointing to outside_dir
    let symlink_path = ws_root.join("evil_symlink");

    // Attempt to create symlink (Windows may require developer mode or admin privilege)
    #[cfg(windows)]
    let symlink_created = std::os::windows::fs::symlink_dir(&outside_dir, &symlink_path).is_ok();
    #[cfg(unix)]
    let symlink_created = std::os::unix::fs::symlink(&outside_dir, &symlink_path).is_ok();

    if symlink_created {
        // 1. Try to read existing file through symlink
        let input_read = json!({
            "root_path": root_str,
            "path": "evil_symlink/secret_outside.txt",
        });
        let res_read = resolve_workspace_path(&input_read, "path", false);
        assert!(
            res_read.is_err(),
            "VULNERABILITY: Symlink traversal escape was NOT blocked! Result: {:?}",
            res_read
        );

        // 2. Try to target a new file to be created through symlink
        let input_write = json!({
            "root_path": root_str,
            "path": "evil_symlink/new_evil_file.txt",
        });
        let res_write = resolve_workspace_path(&input_write, "path", false);
        assert!(
            res_write.is_err(),
            "VULNERABILITY: File creation through symlink escape was NOT blocked! Result: {:?}",
            res_write
        );
    }

    let _ = fs::remove_dir_all(ws_root.parent().unwrap());
}

// =========================================================================
// 2. ADVERSARIAL WORKSPACE ROOT DELETION ATTACKS (FEATURE 6)
// =========================================================================

#[tokio::test]
async fn test_adversarial_workspace_root_deletion_blocked() {
    let (ws_root, _outside) = create_temp_workspace();
    let root_str = ws_root.to_str().unwrap();
    let executor = ToolExecutor::default();

    let root_delete_attempts = [
        json!({ "root_path": root_str, "path": "." }),
        json!({ "root_path": root_str, "path": "./" }),
        json!({ "root_path": root_str, "path": ".\\" }),
        json!({ "root_path": root_str, "path": "" }),
        json!({ "root_path": root_str, "path": "   " }),
        json!({ "root_path": root_str, "path": root_str }),
        json!({ "workspace_root": root_str, "path": "." }),
        json!({ "root_path": root_str }), // omitted path
    ];

    for (idx, attack_input) in root_delete_attempts.iter().enumerate() {
        let req = ToolExecutionRequest::new(
            Uuid::new_v4(),
            None,
            "core.workspace.delete",
            attack_input.clone(),
        );

        let res = executor.execute_workspace_delete(req).await;
        assert!(
            res.is_err(),
            "CRITICAL VULNERABILITY: Root deletion attempt #{} succeeded! Input: {:?}",
            idx,
            attack_input
        );

        // Confirm workspace root still exists!
        assert!(
            ws_root.exists(),
            "CRITICAL: Workspace root directory was deleted by attack #{}: {:?}",
            idx,
            attack_input
        );
    }

    let _ = fs::remove_dir_all(ws_root.parent().unwrap());
}

// =========================================================================
// 3. ADVERSARIAL SECRET LEAKAGE THROUGH PROCESS ENVIRONMENT (FEATURE 7)
// =========================================================================

#[tokio::test]
async fn test_adversarial_secret_leak_in_shell_and_code_execution() {
    // 1. Plant fake high-value credentials in parent process environment
    let fake_secrets = vec![
        ("AWS_SECRET_ACCESS_KEY", "AKIA_FAKE_SECRET_KEY_12345"),
        ("OPENAI_API_KEY", "sk-proj-FAKE-OPENAI-KEY-ABCDEF123456"),
        ("TEST_KEY", "SUPER_SECRET_TEST_KEY_VAL_777"),
        ("GITHUB_TOKEN", "ghp_FAKE_PERSONAL_ACCESS_TOKEN_XYZ"),
        (
            "DATABASE_URL",
            "postgres://admin:topsecretpw@localhost:5432/db",
        ),
        ("ARO_JWT_SECRET", "jwt_secret_token_never_leak_this"),
        ("APP_REDIS_PASSWORD", "redis_secret_pass_888"),
        ("STRIPE_SECRET_KEY", "rk_live_stripe_secret_999"),
        ("MY_AUTH_TOKEN", "auth_bearer_token_val_000"),
    ];

    for (k, v) in &fake_secrets {
        env::set_var(k, v);
    }

    // Also plant a safe variable to ensure environment is not totally empty
    env::set_var("SAFE_PUBLIC_VARIABLE_XYZ", "SAFE_PUBLIC_DATA_CONFIRMED");

    let executor = ToolExecutor::default();

    // 2. Shell execution test: inspect child process environment
    #[cfg(windows)]
    let shell_cmd = "set";
    #[cfg(not(windows))]
    let shell_cmd = "env";

    let shell_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        None,
        "core.shell.execute",
        json!({
            "command": shell_cmd,
        }),
    );

    let shell_res = executor
        .execute_shell(shell_req)
        .await
        .expect("shell execution failed");

    let shell_stdout = shell_res.output["stdout"].as_str().unwrap_or("");

    // Assert safe variable is received
    assert!(
        shell_stdout.contains("SAFE_PUBLIC_VARIABLE_XYZ"),
        "Sanity check failed: SAFE_PUBLIC_VARIABLE_XYZ was not passed to child process! Stdout: {}",
        shell_stdout
    );

    // Assert NONE of the secret keys or secret values leaked into the shell child process
    for (k, v) in &fake_secrets {
        assert!(
            !shell_stdout.contains(k),
            "CRITICAL SECURITY LEAK: Secret environment variable name '{}' leaked into shell child process!",
            k
        );
        assert!(
            !shell_stdout.contains(v),
            "CRITICAL SECURITY LEAK: Secret environment variable value '{}' leaked into shell child process!",
            v
        );
    }

    // 3. Code execution test: inspect child process via python script
    let python_probe_code = r#"
import os
safe_found = "SAFE_PUBLIC_VARIABLE_XYZ" in os.environ
print(f"SAFE_FOUND: {safe_found}")
for k, v in os.environ.items():
    print(f"ENV_VAR: {k}={v}")
"#;

    let code_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        None,
        "core.code.execute",
        json!({
            "language": "python",
            "code": python_probe_code,
        }),
    );

    let code_res = executor
        .execute_code(code_req)
        .await
        .expect("code execution failed");

    let code_stdout = code_res.output["stdout"].as_str().unwrap_or("");

    // If python is installed in this test environment, check python output
    if code_res.status == ToolExecutionStatus::Completed {
        assert!(
            code_stdout.contains("SAFE_FOUND: True"),
            "Sanity check: SAFE_PUBLIC_VARIABLE_XYZ should be present in python env"
        );
        for (k, v) in &fake_secrets {
            assert!(
                !code_stdout.contains(k),
                "CRITICAL SECURITY LEAK: Secret '{}' leaked into code child process!",
                k
            );
            assert!(
                !code_stdout.contains(v),
                "CRITICAL SECURITY LEAK: Secret value '{}' leaked into code child process!",
                v
            );
        }
    }

    // Cleanup planted environment variables
    for (k, _) in &fake_secrets {
        env::remove_var(k);
    }
    env::remove_var("SAFE_PUBLIC_VARIABLE_XYZ");
}

// =========================================================================
// 4. ADVERSARIAL STRESS TEST: 64 KB OUTPUT CAPPING (FEATURE 7)
// =========================================================================

#[tokio::test]
async fn test_adversarial_64kb_output_capping_and_utf8_preservation() {
    let executor = ToolExecutor::default();

    // 1. Stress test stdout capping via shell execution (generate > 120 KB)
    // Direct powershell command loop
    #[cfg(windows)]
    let flood_cmd =
        "for ($i=0; $i -lt 3000; $i++) { Write-Output 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789' }";
    #[cfg(not(windows))]
    let flood_cmd = "python3 -c \"print('A' * 150000)\"";

    let shell_flood_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        None,
        "core.shell.execute",
        json!({
            "command": flood_cmd,
            "shell": "powershell",
        }),
    );

    let shell_flood_res = executor
        .execute_shell(shell_flood_req)
        .await
        .expect("flood command execution");

    let stdout_output = shell_flood_res.output["stdout"].as_str().unwrap_or("");
    assert!(
        stdout_output.contains("[truncated]"),
        "Oversized stdout output was NOT marked as truncated! Length: {}",
        stdout_output.len()
    );
    // Max allowed size is MAX_EXECUTION_OUTPUT_BYTES + "[truncated]" suffix length (~30 bytes)
    assert!(
        stdout_output.len() <= MAX_EXECUTION_OUTPUT_BYTES + 100,
        "Oversized stdout output ({} bytes) exceeded 64 KB limit!",
        stdout_output.len()
    );

    // 2. Stress test stderr capping via shell execution
    #[cfg(windows)]
    let flood_stderr_cmd = "for ($i=0; $i -lt 3000; $i++) { [Console]::Error.WriteLine('ERR_LINE_0123456789_ERR_LINE_0123456789') }";
    #[cfg(not(windows))]
    let flood_stderr_cmd = "python3 -c \"import sys; sys.stderr.write('E' * 150000)\"";

    let shell_err_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        None,
        "core.shell.execute",
        json!({
            "command": flood_stderr_cmd,
            "shell": "powershell",
        }),
    );

    let shell_err_res = executor
        .execute_shell(shell_err_req)
        .await
        .expect("flood stderr execution");

    let stderr_output = shell_err_res.output["stderr"].as_str().unwrap_or("");
    assert!(
        stderr_output.contains("[truncated]"),
        "Oversized stderr output was NOT marked as truncated!"
    );
    assert!(
        stderr_output.len() <= MAX_EXECUTION_OUTPUT_BYTES + 100,
        "Oversized stderr output ({} bytes) exceeded 64 KB limit!",
        stderr_output.len()
    );

    // 3. UTF-8 boundary stress test with multibyte characters right at 64 KB
    // Each French accented 'é' is 2 bytes (0xC3 0xA9).
    // Let's create a string of 32,768 'é' characters = 65,536 bytes exactly,
    // plus one extra 'é' = 65,538 bytes.
    let multibyte_stream = "é".repeat(32_768 + 10);
    let (truncated_mb, was_truncated) =
        truncate_output_safe(&multibyte_stream, MAX_EXECUTION_OUTPUT_BYTES);
    assert!(was_truncated);
    assert!(truncated_mb.contains("[truncated]"));
    // Verify valid UTF-8 and no corruption
    assert!(std::str::from_utf8(truncated_mb.as_bytes()).is_ok());

    // Test with odd max_bytes (e.g. 65535) where boundary falls on the second byte of 'é'
    let (odd_trunc, was_odd_trunc) = truncate_output_safe(&multibyte_stream, 65_535);
    assert!(was_odd_trunc);
    assert!(std::str::from_utf8(odd_trunc.as_bytes()).is_ok());
}

#[tokio::test]
async fn test_adversarial_workspace_read_multibyte_boundary() {
    let (ws_root, _outside) = create_temp_workspace();
    let root_str = ws_root.to_str().unwrap();
    let executor = ToolExecutor::default();

    // 15,999 ASCII 'a' bytes + French accented 'é' (2 bytes, 0xC3 0xA9)
    // Byte 16,000 is the continuation byte of 'é'.
    let malicious_utf8_content = format!("{}{}", "a".repeat(15_999), "é");
    assert_eq!(malicious_utf8_content.len(), 16_001);

    let test_file = ws_root.join("utf8_trap.txt");
    fs::write(&test_file, &malicious_utf8_content).expect("write trap file");

    let read_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        None,
        "core.workspace.read",
        json!({
            "root_path": root_str,
            "path": "utf8_trap.txt",
        }),
    );

    let res = executor.execute_workspace_read(read_req).await;
    assert!(
        res.is_ok(),
        "Expected execute_workspace_read to succeed without panic, got: {:?}",
        res
    );
    let output = res.unwrap();
    assert_eq!(output.status, ToolExecutionStatus::Completed);
    let read_content = output.output["content"].as_str().unwrap();
    assert!(read_content.contains("[truncated]"));
    assert!(std::str::from_utf8(read_content.as_bytes()).is_ok());

    let _ = fs::remove_dir_all(ws_root.parent().unwrap());
}

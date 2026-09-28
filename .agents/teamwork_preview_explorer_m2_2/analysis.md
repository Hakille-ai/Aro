# Technical Investigation: Feature 6 (Strict Workspace Path Confinement) & Feature 7 (Code & Shell Execution Sandboxing)

**Author:** 	eamwork_preview_explorer_m2_2  
**Date:** 2026-09-24 / 2026-09-25  
**Working Directory:** c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m2_2  
**Target Crates:** crates/aro-tools, crates/aro-core, crates/aro-skills, pps/desktop/src-tauri  
**Reference:** Feature 6 & Feature 7 (Milestone 2) from PROJECT.md & ORIGINAL_REQUEST.md (R2)

---

## 1. Executive Summary

This investigation delivers the exact technical analysis, vulnerability breakdown, and implementation blueprint for:
1. **Feature 6: Strict Workspace Path Confinement** (esolve_workspace_path in crates/aro-tools).
2. **Feature 7: Code & Shell Execution Sandboxing** (scrubbed_env & resource bounds in crates/aro-tools).

### Key Vulnerabilities Discovered
- **Critical Fail-Open Escape in esolve_workspace_path (crates/aro-tools/src/lib.rs:129)**:
  When oot_path (or ootPath, workspace_root, workspaceRoot) is omitted or empty ("), esolve_workspace_path directly returns Some(PathBuf::from(p)). An agent or prompt injection can read, write, or recursively delete any arbitrary file or directory on the host machine (C:\Windows\System32, /etc/passwd, user home directories).
- **Silent Fail-Open in Directory Tools (search, list, grep, eplace_in_files, git_diff)**:
 When esolve_workspace_path fails, each of these tools falls back via .unwrap_or_else(|| PathBuf::from(.)), scanning and mutating the current working directory of the host application process.
- **Unchecked File Writing in execute_document_create (crates/aro-tools/src/lib.rs:1257-1289)**:
 execute_document_create bypasses esolve_workspace_path entirely, allowing arbitrary absolute paths or relative .. escapes.
- **Environment Leak in execute_shell and execute_code (crates/aro-tools/src/lib.rs:887-926, 1042-1130)**:
 Neither execute_shell nor execute_code calls cmd.env_clear(). All spawned processes inherit 100% of host environment variables, exposing LLM API keys (OPENAI_API_KEY, ANTHROPIC_API_KEY), database passwords (DATABASE_URL), cloud secrets (AWS_*, GITHUB_*), and Aro encryption keys.
- **Unbounded Memory Buffering in Shell Execution (crates/aro-tools/src/lib.rs:929-935)**:
 child.wait_with_output() buffers entire stdout and stderr into memory without output caps. Neither stdout nor stderr is truncated in the returned tool result, risking memory exhaustion or IPC serialization failure.

---

## 2. Feature 6: Strict Workspace Path Confinement

### 2.1 Deep-Dive on Existing Implementation (crates/aro-tools/src/lib.rs:88-176)

The current implementation is:
`ust
pub fn resolve_workspace_path(
 input: &Value,
 path_field: &str,
 default_to_root: bool,
) -> Option<std::path::PathBuf> {
 let raw_path = input
 .get(path_field)
 .or_else(|| {
 if path_field == path {
 input.get(file_path).or_else(|| input.get(filePath))
 } else {
 None
 }
 })
 .and_then(|v| v.as_str())
 .map(str::trim)
 .filter(|s| !s.is_empty());

 let root_str = input
 .get(root_path)
 .or_else(|| input.get(rootPath))
 .or_else(|| input.get(workspace_root))
 .or_else(|| input.get(workspaceRoot))
 .and_then(|v| v.as_str())
 .map(str::trim)
 .filter(|s| !s.is_empty());

 let root_buf = root_str.map(std::path::PathBuf::from);

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
 (Some(p), None) => Some(std::path::PathBuf::from(p)), // <-- FATAL FLAW
 (None, Some(root)) if default_to_root => Some(root),
 (None, None) if default_to_root => {
 Some(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(.)))
 }
 _ => None,
 }
}
`

### 2.2 Vulnerability Analysis

1. **Path Escape when oot_path is Missing**:
 If input is {path: C:\\Windows\\System32\\drivers\\etc\\hosts}, oot_buf is None.
 The match branch evaluates to (Some(p), None) => Some(std::path::PathBuf::from(p)).
 The function returns Some(PathBuf::from(C:\\Windows\\System32\\drivers\\etc\\hosts)) with zero confinement checks!
2. **Path Escape when oot_path is Empty ( or whitespace)**:
 Because of .filter(|s| !s.is_empty()) at line 113, passing  or   sets oot_str = None.
 This immediately matches (Some(p), None) and bypasses all confinement checks. An attacker can supply root_path:  to bypass containment.
3. **Host Process CWD Leaks when default_to_root is True**:
 At line 131, if both aw_path and oot_buf are None, it returns std::env::current_dir(). The host application's working directory is exposed as the workspace root.
4. **Symlink / Non-Existent Target Escape in contained_in_root (crates/aro-tools/src/lib.rs:167-174)**:
 In contained_in_root, canonicalization is only performed if both paths can be canonicalized:
 `ust
 if let (Ok(canon_root), Ok(canon_cand)) = (
 std::fs::canonicalize(&root_norm),
 std::fs::canonicalize(&cand_norm),
 ) {
 if !canon_cand.starts_with(&canon_root) {
 return None;
 }
 }
 Some(cand_norm)
 `
 When writing a *new* file (e.g., via execute_workspace_write), cand_norm does NOT exist yet on disk.
 std::fs::canonicalize(&cand_norm) returns Err(NotFound).
 The entire canonical check is skipped! If an existing subdirectory in the workspace is a symlink or Windows directory junction pointing outside the workspace (e.g. workspace/shared -> C:\Windows), writing workspace/shared/payload.txt passes lexical check starts_with(workspace) and writes to C:\Windows\payload.txt!
5. **Windows Verbatim Prefix Mismatches (\\?\)**:
 On Windows, std::fs::canonicalize prepends \\?\. Comparing a canonicalized path (\\?\C:\foo) with a non-canonicalized path (C:\foo) via starts_with fails even when legitimate, causing erratic failures or bypasses.

---

### 2.3 Inspection of All Workspace Tools in crates/aro-tools

| Tool ID | Method Name | Line Numbers | Current Confinement Behavior | Required Fix |
|---|---|---|---|---|
| core.workspace.read | execute_workspace_read | 388-439 | Calls esolve_workspace_path(..., false). Fails open if oot_path omitted. Returns misleading error path parameter is required on confinement error. | Return AroResult<ToolExecutionResult>. Propagate esolve_workspace_path(...)? directly. Fails closed with AroError::Security. |
| core.workspace.write | execute_workspace_write | 440-499 | Calls esolve_workspace_path(..., false). Overwrites any host file if oot_path omitted! | Propagate esolve_workspace_path(...)?. Enforce ancestor canonicalization for new files. |
| core.workspace.delete | execute_workspace_delete | 726-780 | Calls esolve_workspace_path(..., false). Recursively removes arbitrary directories on host! | Propagate esolve_workspace_path(...)?. Forbid deleting workspace root itself ( arget_path == canon_root). |
| core.workspace.list | execute_workspace_list | 500-609 | Uses .unwrap_or_else(|| PathBuf::from(.)). Scans host CWD if oot_path omitted! | Replace fallback with esolve_workspace_path(..., true)?. Fails closed if root missing. |
| core.workspace.grep | execute_workspace_grep | 610-725 | Uses .unwrap_or_else(|| PathBuf::from(.)). Searches host CWD if oot_path omitted! | Replace fallback with esolve_workspace_path(..., true)?. |
| core.workspace.search | execute_workspace_search | 292-387 | Uses .unwrap_or_else(|| PathBuf::from(.)). Scans host CWD if oot_path omitted! | Replace fallback with esolve_workspace_path(..., true)?. |
| core.workspace.replace_in_files | execute_workspace_replace_in_files | 1590-1690 | Uses .unwrap_or_else(|| PathBuf::from(.)). Mutates files across host CWD! | Replace fallback with esolve_workspace_path(..., true)?. |
| core.workspace.git_diff | execute_workspace_git_diff | 1691-1770 | Uses .unwrap_or_else(|| PathBuf::from(.)). Runs git in host CWD! | Replace fallback with esolve_workspace_path(..., true)?. |
| core.document.create | execute_document_create | 1257-1289 | **Does NOT call esolve_workspace_path at all!** Writes directly to any path_str. | Must use esolve_workspace_path for dest_path. |
| core.shell.execute | execute_shell | 877-879 | cwd uses .unwrap_or_else(|| current_dir()). | When oot_path is present, strictly confine cwd. |
| core.code.execute | execute_code | 1034-1036 | cwd uses .unwrap_or_else(|| current_dir()). | When oot_path is present, strictly confine cwd. |

---

### 2.4 Recommended Design for esolve_workspace_path

To guarantee fail-closed behavior, the signature should return AroResult<std::path::PathBuf>:

`ust
fn strip_verbatim_prefix(path: &std::path::Path) -> std::path::PathBuf {
 let s = path.to_string_lossy();
 if let Some(stripped) = s.strip_prefix(r\\?") {
        std::path::PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

pub fn resolve_workspace_path(
    input: &Value,
    path_field: &str,
    default_to_root: bool,
) -> AroResult<std::path::PathBuf> {
    // 1. Extract and enforce non-empty root_path
    let root_str = input
        .get(root_path)
        .or_else(|| input.get(rootPath))
        .or_else(|| input.get(workspace_root))
        .or_else(|| input.get(workspaceRoot))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AroError::Security(workspace root path is required but missing or empty.to_string())
        })?;

    if root_str.contains('\0') {
        return Err(AroError::Security(workspace root path contains null byte.to_string()));
    }

    let root_path = std::path::Path::new(root_str);
    let clean_root = strip_verbatim_prefix(root_path);
    let canon_root = std::fs::canonicalize(root_path)
        .map(|p| strip_verbatim_prefix(&p))
        .unwrap_or_else(|_| clean_root.clone());

    // 2. Extract path argument
    let raw_path = input
        .get(path_field)
        .or_else(|| {
            if path_field == path {
                input.get(file_path).or_else(|| input.get(filePath))
            } else {
                None
            }
        })
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let target_str = match raw_path {
        Some(p) => p,
        None if default_to_root => return Ok(canon_root),
        None => {
            return Err(AroError::Configuration(format!(
                '{path_field}' parameter is required
            )))
        }
    };

    if target_str.contains('\0') {
        return Err(AroError::Security(target path contains null byte.to_string()));
    }

    let cand_path = std::path::Path::new(target_str);
    let combined = if cand_path.is_absolute() {
        cand_path.to_path_buf()
    } else {
        canon_root.join(cand_path)
    };

    // 3. Normalize components against .. escapes
    let mut normalized = std::path::PathBuf::new();
    for comp in combined.components() {
        match comp {
            std::path::Component::Prefix(p) => normalized.push(p.as_os_str()),
            std::path::Component::RootDir => normalized.push(comp.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(AroError::Security(format!(
                        access denied: path traversal escapes root: '{target_str}'
                    )));
                }
            }
            std::path::Component::Normal(c) => normalized.push(c),
        }
    }

    let clean_normalized = strip_verbatim_prefix(&normalized);

    // 4. Target exists on disk: canonicalize and verify prefix
    if let Ok(canon) = std::fs::canonicalize(&clean_normalized) {
        let clean_canon = strip_verbatim_prefix(&canon);
        if !clean_canon.starts_with(&canon_root) && !clean_canon.starts_with(&clean_root) {
            return Err(AroError::Security(format!(
                access denied: resolved path '{}' escapes workspace root '{}',
                clean_canon.display(),
                canon_root.display()
            )));
        }
        return Ok(clean_canon);
    }

    // 5. Target does not exist yet (file creation): verify closest existing ancestor
    let mut cur = clean_normalized.as_path();
    let mut existing_ancestor = None;
    while let Some(parent) = cur.parent() {
        if parent.exists() {
            existing_ancestor = Some(parent);
            break;
        }
        cur = parent;
    }

    if let Some(ancestor) = existing_ancestor {
        if let Ok(canon_ancestor) = std::fs::canonicalize(ancestor) {
            let clean_ancestor = strip_verbatim_prefix(&canon_ancestor);
            if !clean_ancestor.starts_with(&canon_root) && !clean_ancestor.starts_with(&clean_root) {
                return Err(AroError::Security(format!(
                    access denied: parent directory '{}' escapes workspace root '{}',
                    clean_ancestor.display(),
                    canon_root.display()
                )));
            }
        } else {
            return Err(AroError::Security(format!(
                access denied: unable to resolve ancestor directory for '{target_str}'
            )));
        }
    }

    // 6. Lexical containment check
    if !clean_normalized.starts_with(&clean_root) && !clean_normalized.starts_with(&canon_root) {
        return Err(AroError::Security(format!(
            access denied: path '{}' escapes workspace root '{}',
            clean_normalized.display(),
            canon_root.display()
        )));
    }

    Ok(clean_normalized)
}
`

---

## 3. Feature 7: Code & Shell Execution Sandboxing

### 3.1 Implementation Locations
- **core.shell.execute**: crates/aro-tools/src/lib.rs:781-1007 (pub async fn execute_shell).
- **core.code.execute**: crates/aro-tools/src/lib.rs:1009-1223 (pub async fn execute_code).

### 3.2 Environment Scrubbing Architecture (scrubbed_env)

execute_shell and execute_code currently spawn processes using 	okio::process::Command::new(...) without clearing the inherited environment.

#### Required Filtering Rules
The environment filter must combine:
1. **Specific prefixes**:
   - AWS_ (e.g. AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_SESSION_TOKEN)
   - GITHUB_, GH_ (e.g. GITHUB_TOKEN, GITHUB_PAT, GH_TOKEN)
   - ARO_ (e.g. ARO_JWT_SECRET, ARO_DATABASE_URL, ARO_ADMIN_KEY)
   - OPENAI_, ANTHROPIC_, GEMINI_, GROQ_, MISTRAL_, DEEPSEEK_, COHERE_
   - STRIPE_, SLACK_, DISCORD_, TWILIO_, SENDGRID_, RESEND_
   - DATABASE_, DB_, POSTGRES_, MYSQL_, REDIS_
2. **Specific suffixes**:
   - _KEY (e.g. API_KEY, ACCESS_KEY, PRIVATE_KEY, SIGNING_KEY, SECRET_KEY)
   - _SECRET (e.g. APP_SECRET, CLIENT_SECRET, JWT_SECRET)
   - _TOKEN (e.g. AUTH_TOKEN, BEARER_TOKEN, ACCESS_TOKEN, REFRESH_TOKEN)
   - _PASSWORD, _PASSWD
   - _CREDENTIAL, _CREDENTIALS
   - _AUTH, _CERT
3. **Sensitive Substrings**:
   - API_KEY, APIKEY, SECRET, PASSWORD, PASSWD, TOKEN, DATABASE_URL, DATABASE, REDIS, BEARER, COOKIE, SESSION, CREDENTIAL, PRIVATE_KEY, SIGNING_KEY, KEYRING, ENCRYPTION, SMTP.
4. **Preserved Safe Operating Variables**:
   Standard system variables required for process launch and runtime execution must remain intact:
   - Windows: SystemRoot, SystemDrive, WINDIR, PATH, PATHEXT, TEMP, TMP, COMSPEC, USERPROFILE, APPDATA, LOCALAPPDATA.
   - Unix: PATH, HOME, SHELL, USER, TMPDIR, LANG, LC_*, PWD.
   - Language runtimes: PYTHONPATH, NODE_PATH (unless matching a sensitive keyword).

#### Scrubbing Implementation

`ust
pub fn is_sensitive_env_var(key: &str) -> bool {
    let upper = key.to_uppercase();

    // 1. Prefixes
    if upper.starts_with(AWS_)
        || upper.starts_with(GITHUB_)
        || upper.starts_with(GH_)
        || upper.starts_with(ARO_)
        || upper.starts_with(OPENAI_)
        || upper.starts_with(ANTHROPIC_)
        || upper.starts_with(GEMINI_)
        || upper.starts_with(GROQ_)
        || upper.starts_with(MISTRAL_)
        || upper.starts_with(DEEPSEEK_)
        || upper.starts_with(COHERE_)
        || upper.starts_with(STRIPE_)
        || upper.starts_with(SLACK_)
        || upper.starts_with(DISCORD_)
        || upper.starts_with(DATABASE_)
        || upper.starts_with(DB_)
        || upper.starts_with(REDIS_)
    {
        return true;
    }

    // 2. Suffixes
    if upper.ends_with(_KEY)
        || upper.ends_with(_SECRET)
        || upper.ends_with(_TOKEN)
        || upper.ends_with(_PASSWORD)
        || upper.ends_with(_PASSWD)
        || upper.ends_with(_CREDENTIAL)
        || upper.ends_with(_CREDENTIALS)
        || upper.ends_with(_AUTH)
        || upper.ends_with(_CERT)
    {
        return true;
    }

    // 3. Substrings
    const SENSITIVE_SUBSTRINGS: &[&str] = &[
        API_KEY,
        APIKEY,
        SECRET,
        PASSWORD,
        PASSWD,
        TOKEN,
        DATABASE_URL,
        DATABASE,
        REDIS_,
        REDIS_URL,
        PRIVATE_KEY,
        SIGNING_KEY,
        BEARER,
        COOKIE,
        SESSION,
        CREDENTIAL,
        KEYRING,
        ENCRYPTION,
        SMTP,
    ];

    SENSITIVE_SUBSTRINGS.iter().any(|needle| upper.contains(needle))
}

pub fn scrubbed_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(key, _)| !is_sensitive_env_var(key))
        .collect()
}
`

#### Application to Process Spawning
In both execute_shell and execute_code:
`ust
cmd.env_clear();
for (key, val) in scrubbed_env() {
    cmd.env(key, val);
}
`
cmd.env_clear() wipes the inherited parent environment, and cmd.env(key, val) selectively populates only non-sensitive variables.

---

### 3.3 Execution Resource Limits & Bounds

#### 1. Wall-Clock Timeout Limit
- **Default Budget**: 30 seconds.
- **Configurable Range**: 1 to 300 seconds (equest.input.get(timeout_secs).unwrap_or(30).clamp(1, 300)).
- **Enforcement**:
  `ust
  cmd.kill_on_drop(true);
  let mut child = cmd.spawn()?;
  let wait_result = tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;
  if wait_result.is_err() {
      let _ = child.kill().await;
      let _ = child.wait().await;
  }
  `
  Guarantees no orphaned zombie processes remain running.

#### 2. Per-Stream Output Caps (MAX_EXECUTION_OUTPUT_BYTES)
- **Budget**: 64 KB (65,536 bytes) per stream (stdout and stderr).
- **UTF-8 Char Boundary Truncation**:
  `ust
  pub const MAX_EXECUTION_OUTPUT_BYTES: usize = 64 * 1024;

  pub fn truncate_output_safe(text: &str, max_bytes: usize) -> (String, bool) {
      if text.len() <= max_bytes {
          return (text.to_string(), false);
      }
      let mut end = max_bytes;
      while end > 0 && !text.is_char_boundary(end) {
          end -= 1;
      }
      (format!({}...\n[truncated], &text[..end]), true)
  }
  `
- **Consistent Application**:
  Apply truncation to:
  1. output[stdout]
  2. output[stderr]
  3. context_sources[0].excerpt
  Prevents serialization blow-ups, memory bloat, and UI freezes.

#### 3. Temporary Script Cleanup Guard (execute_code)
In execute_code:
- Generate script in std::env::temp_dir().join(aro_code_exec).
- Clean up script using RAII or explicit removal in both normal and error branches.

---

## 4. Verification & Testing Strategy

### 4.1 Unit Test Invariants (crates/aro-tools/src/lib.rs)

1. **	est_workspace_path_fails_closed_missing_root**:
   Verify calling esolve_workspace_path with no oot_path returns Err(AroError::Security(_)).
2. **	est_workspace_path_fails_closed_empty_root**:
   Verify calling esolve_workspace_path with oot_path: " or     returns Err(AroError::Security(_)).
3. ** est_workspace_path_blocks_parent_directory_traversal**:
 Verify ../../secret.txt, sub/../../secret.txt, and /etc/shadow are rejected with Err(AroError::Security(_)).
4. ** est_workspace_path_blocks_null_byte**:
 Verify path: valid\0evil is rejected with Err(AroError::Security(_)).
5. ** est_workspace_path_permits_valid_subpaths**:
 Verify subpaths inside root resolve correctly to canonical paths.
6. ** est_scrubbed_env_filters_secrets**:
 Verify AWS_ACCESS_KEY_ID, OPENAI_API_KEY, GITHUB_TOKEN, DATABASE_URL, ARO_JWT_SECRET, and MY_CUSTOM_SECRET_KEY return rue for is_sensitive_env_var.
 Verify PATH, SystemRoot, TEMP, HOME, LANG return alse.
7. ** est_execute_code_scrubs_environment**:
 Set TEST_ARO_SECRET_TOKEN=supersecret123 in parent process.
 Execute Python script: import os; print(os.environ.get('TEST_ARO_SECRET_TOKEN', 'NOT_FOUND')).
 Verify output contains NOT_FOUND and does NOT contain supersecret123.
8. ** est_execute_code_caps_large_output**:
 Execute Python script printing 200 KB of text.
 Verify output length is <= 65,536 + length of truncation notice.
9. ** est_execute_shell_caps_large_output**:
 Execute shell command printing large text.
 Verify stdout and stderr are truncated to MAX_EXECUTION_OUTPUT_BYTES.

---

## 5. Implementation Roadmap for Milestone 2

1. **Phase 1: Security Primitives in crates/aro-tools**:
 - Add is_sensitive_env_var and scrubbed_env.
 - Add runcate_output_safe and MAX_EXECUTION_OUTPUT_BYTES.
 - Rewrite esolve_workspace_path with fail-closed semantics and ancestor canonicalization.
2. **Phase 2: Tool Callers Update**:
 - Update execute_workspace_read, write, delete, list, grep, search, eplace_in_files, git_diff to use esolve_workspace_path(...)?.
 - Update execute_document_create to use esolve_workspace_path.
 - Update execute_shell and execute_code to apply cmd.env_clear(), scrubbed_env(), output caps, and working dir validation.
3. **Phase 3: Verification**:
 - Run cargo test -p aro-tools.
 - Run cargo test -p aro-runtime.
 - Run cargo test -p aro-desktop-app --test adversarial_containment.
 - Verify cargo clippy --workspace --all-targets -- -D warnings (0 warnings).

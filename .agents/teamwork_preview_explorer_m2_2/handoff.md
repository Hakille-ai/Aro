# Milestone 2 Handoff: Feature 6 (Path Confinement) & Feature 7 (Sandboxing)

**Agent:** 	eamwork_preview_explorer_m2_2  
**Parent Orchestrator ID:** 279e94fd-d099-4039-9ebd-159c33f6194c  
**Date:** 2026-09-24 / 2026-09-25  
**Type:** Hard Handoff (Investigation Complete)  
**Detailed Analysis Reference:** c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_explorer_m2_2\analysis.md

---

## 1. Observation

1. **esolve_workspace_path Implementation (crates/aro-tools/src/lib.rs:88-136)**:
   - Lines 106-115 extract oot_str from oot_path, ootPath, workspace_root, workspaceRoot and filter empty strings.
   - Lines 129-133:
     `ust
     (Some(p), None) => Some(std::path::PathBuf::from(p)),
     (None, Some(root)) if default_to_root => Some(root),
     (None, None) if default_to_root => {
         Some(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(.)))
     }
     `
     When oot_str is None (omitted or empty "), line 129 returns Some(PathBuf::from(p)) without any confinement check.
 - Lines 167-174 in contained_in_root:
 `ust
 if let (Ok(canon_root), Ok(canon_cand)) = (
 std::fs::canonicalize(&root_norm),
 std::fs::canonicalize(&cand_norm),
 ) {
 if !canon_cand.starts_with(&canon_root) {
 return None;
 }
 }
 `
 For new files (e.g. execute_workspace_write), cand_norm does not exist on disk, causing std::fs::canonicalize to fail with Err(NotFound). The canonical prefix check is bypassed completely.

2. **Callers in crates/aro-tools/src/lib.rs**:
 - execute_workspace_read (line 393): esolve_workspace_path(&request.input, path, false).ok_or_else(|| AroError::Configuration(path parameter is required.to_string()))?
 - execute_workspace_write (line 445): same pattern, writes anywhere if oot_path omitted.
 - execute_workspace_delete (line 731): same pattern, recursively deletes directories anywhere if oot_path omitted.
 - execute_workspace_search (line 304), execute_workspace_list (line 505), execute_workspace_grep (line 620), execute_workspace_replace_in_files (line 1605), execute_workspace_git_diff (line 1696): each uses .unwrap_or_else(|| std::path::PathBuf::from(.)), falling back to host process working directory when path resolution fails.
 - execute_document_create (lines 1257-1289): completely bypasses esolve_workspace_path and writes directly to arbitrary paths.

3. **Shell and Code Execution (crates/aro-tools/src/lib.rs:781-1223)**:
 - execute_shell (lines 887-926) and execute_code (lines 1042-1130) instantiate okio::process::Command without calling .env_clear().
 - Child processes inherit the entire parent process environment, including all API keys, database credentials, and cloud tokens.
 - In execute_shell, stdout and stderr are un-capped; child.wait_with_output() buffers entire streams in memory.
 - In execute_code, output is truncated to 64,000 characters in output[stdout], but stderr and context_sources[0].excerpt remain un-truncated.

4. **Reference Implementation in pps/desktop/src-tauri (main.rs:2319-2419 & ests/adversarial_containment.rs:15-112)**:
 - Implements esolve_safe_workspace_path with:
 * strip_verbatim_prefix to handle Windows \\?\ prefix mismatches.
 * Closest existing ancestor directory canonicalization for non-existent target files.
 * Component-by-component traversal protection against ...

5. **Reference Implementation in crates/aro-skills/src/sandbox.rs:54-95**:
 - Implements SECRET_ENV_SUBSTRINGS and scrubbed_env().
 - Implements runcate_output ensuring char boundary safety.

---

## 2. Logic Chain

1. **Vulnerability in Path Confinement**:
 - From Observation 1: Line 129 returns Some(PathBuf::from(p)) whenever oot_buf is None.
 - From Observation 2: Tool callers pass &request.input directly to esolve_workspace_path. If an agent run does not have a workspace root configured in conversation state or if an adversary omits oot_path, oot_buf is None.
 - Therefore, execute_workspace_read, execute_workspace_write, and execute_workspace_delete can operate on arbitrary files (C:\Windows, /etc/passwd) outside the workspace.
 - Furthermore, search, list, grep, eplace_in_files, and git_diff fall back to ., operating on the desktop or cloud server's runtime directory.

2. **Remediation for Path Confinement**:
 - Changing esolve_workspace_path to return AroResult<std::path::PathBuf> and requiring oot_path to be present and non-empty ensures fail-closed security.
 - Stripping \\?\ prefixes guarantees consistent string and path prefix comparisons on Windows.
 - Canonicalizing the closest existing ancestor for non-existent files closes the symlink junction bypass for file creation.
 - Removing .unwrap_or_else(|| PathBuf::from(.)) across all workspace tools prevents silent fallbacks to host process CWD.

3. **Vulnerability in Execution Sandboxing**:
 - From Observation 3: execute_shell and execute_code never call cmd.env_clear().
 - Standard OS process spawning inherits all environment variables of the parent process.
 - Host process holds secrets: OPENAI_API_KEY, ANTHROPIC_API_KEY, DATABASE_URL, AWS_*, GITHUB_*, ARO_*.
 - Therefore, any executed code or shell command can dump process.env / os.environ and exfiltrate secrets.

4. **Remediation for Execution Sandboxing**:
 - Introducing is_sensitive_env_var with prefix, suffix, and substring denylists strips all sensitive secrets while preserving standard runtime variables (PATH, SystemRoot, TEMP, USER, HOME).
 - Calling cmd.env_clear() followed by or (k, v) in scrubbed_env() { cmd.env(k, v); } enforces clean isolation before process launch.
 - Adding MAX_EXECUTION_OUTPUT_BYTES = 64 * 1024 with UTF-8 boundary truncation ( runcate_output_safe) caps both stdout and stderr in tool outputs and context sources.

---

## 3. Caveats

- **OS-Level Namespacing**: The proposed sandboxing is process-level environment scrubbing and timeout/output bounding; it does not constitute an OS-level hypervisor or container sandbox (such as Docker, Windows AppContainer, or Linux seccomp). Full OS isolation is delegated to future containerized worker backends.
- **Windows Verbatim Prefix**: Windows UNC/verbatim paths (\\?\) must always be stripped using strip_verbatim_prefix before comparing starts_with, or comparison will produce false rejections.
- **Conversation Root Propagation**: In crates/aro-runtime/src/lib.rs:1493-1499, oot_path is injected into ction.input only if self.store.resolve_effective_root_path(conv_id) returns a root. If a conversation has no folder/project root assigned, workspace tools will fail closed with a clear security error requiring a workspace root to be set.

---

## 4. Conclusion

1. **Feature 6 Plan**:
 - Refactor esolve_workspace_path(input: &Value, path_field: &str, default_to_root: bool) -> AroResult<PathBuf> in crates/aro-tools/src/lib.rs.
 - Require non-empty oot_path (fail closed with AroError::Security).
 - Implement strip_verbatim_prefix, component traversal checks, and ancestor canonicalization for new files.
 - Update all 10 callers in crates/aro-tools/src/lib.rs to propagate esolve_workspace_path(...)? and eliminate fail-open unwrap_or_else(|| PathBuf::from(.)).
 - Update execute_document_create to route destination paths through esolve_workspace_path.
2. **Feature 7 Plan**:
 - Implement is_sensitive_env_var and scrubbed_env() in crates/aro-tools/src/lib.rs.
 - Strip prefixes (AWS_, GITHUB_, GH_, ARO_, DATABASE_, AI provider keys), suffixes (_KEY, _SECRET, _TOKEN, _PASSWORD), and sensitive substrings.
 - Call cmd.env_clear() and inject scrubbed_env() in execute_shell and execute_code.
 - Enforce MAX_EXECUTION_OUTPUT_BYTES = 64 * 1024 on stdout and stderr across output JSON and context sources.
 - Enforce kill_on_drop(true) and drain child processes on timeout.

---

## 5. Verification Method

To independently verify the implementation:
1. **Unit Tests in crates/aro-tools**:
 - Run cargo test -p aro-tools -- --nocapture.
 - Verify est_workspace_path_fails_closed_missing_root passes (returns Err(AroError::Security(_))).
 - Verify est_workspace_path_fails_closed_empty_root passes.
 - Verify est_workspace_path_blocks_parent_directory_traversal passes (../../secret.txt, /etc/passwd, C:\Windows).
 - Verify est_workspace_path_blocks_null_byte passes.
 - Verify est_scrubbed_env_filters_secrets passes.
 - Verify est_execute_code_scrubs_environment executes a Python script verifying TEST_SECRET_KEY is not present in os.environ.
 - Verify est_execute_code_caps_large_output and est_execute_shell_caps_large_output verify truncation at 64 KB.
2. **Adversarial Containment Suite**:
 - Run cargo test -p aro-desktop-app --test adversarial_containment.
3. **Workspace Lints**:
 - Run cargo clippy --workspace --all-targets -- -D warnings.

# Forensic Audit Report: Milestone 2 — Kernel-Grade Tool Authorization Guard & Sandboxing

**Auditor**: `teamwork_preview_auditor_m2_final`  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\.agents\teamwork_preview_auditor_m2_final`  
**Report To**: Parent Orchestrator (`279e94fd-d099-4039-9ebd-159c33f6194c`)  
**Date**: 2026-09-25T08:58:30Z  
**Profile**: General Project  
**Integrity Mode**: Development (from `ORIGINAL_REQUEST.md`)  
**Verdict**: **INTEGRITY VIOLATION**

---

## 1. Observation

### 1.1 Source Code Forensic Analysis
1. **Tool Authorization Guard (`crates/aro-tools/src/security.rs`)**:
   - `classify_tool(tool_name: &str)` (lines 61–122): Properly identifies tool categories (`Shell`, `Network`, `Write`, `Read`, `Other`) based on tool name keywords and canonical namespaces.
   - `ToolAuthorizationGuard` (lines 186–414): Evaluates access with the correct priority:
     * Empty/whitespace tool names are rejected (`Err("Tool name cannot be empty")`).
     * `denied_tools` takes absolute priority (overrides all presets including `Developer`).
     * `allowed_tools` acts as a strict whitelist when populated.
     * Presets (`Standard`, `ReadOnly`, `Developer`, `Sandbox`, `Custom`) enforce expected boundary invariants.
   - No mock facades or hardcoded bypasses were detected in `crates/aro-tools/src/security.rs`.

2. **Workspace Path Confinement (`crates/aro-tools/src/lib.rs:214–324`)**:
   - `resolve_workspace_path`:
     * Fails closed if `root_path` is missing, empty, or whitespace.
     * Rejects null byte injection in root or target paths (`\0`).
     * Normalizes path components and canonicalizes existing paths.
     * Verifies that existing targets and non-existent targets' existing ancestors are contained within the canonical or clean workspace root.
     * Fails closed against parent traversal escapes (`..`).
   - `execute_workspace_delete` (`crates/aro-tools/src/lib.rs:869–925`):
     * Enforces `strip_root != strip_target` to prevent deleting the workspace root itself.

3. **Execution Sandboxing & Environment Scrubbing (`crates/aro-tools/src/lib.rs:116–193, 1086–1090, 1305–1309`)**:
   - `is_sensitive_env_var` filters sensitive prefixes (`AWS_`, `GITHUB_`, `ARO_`, `OPENAI_`, `ANTHROPIC_`, `DATABASE_`, etc.), suffixes (`_KEY`, `_SECRET`, `_TOKEN`, `_PASSWORD`, etc.), and substrings.
   - `scrubbed_env()` collects only non-sensitive variables.
   - Both shell execution (`crates/aro-tools/src/lib.rs:1086`) and code execution (`crates/aro-tools/src/lib.rs:1305`) explicitly invoke `cmd.env_clear()` before loading `scrubbed_env()`.
   - Both shell and code processes set `.kill_on_drop(true)`.

4. **Tool Registry Catalogue Parity (`crates/aro-agent/src/lib.rs:537–605, 2116–2380`)**:
   - Exposes exactly 40 built-in tool descriptors in `ToolRegistry::default()`.
   - Includes full descriptors for `workspace.delete`, `workspace.replace_in_files`, `workspace.git_diff`, and `artifact.create`.
   - Schema validation passes for all 40 descriptors.
   - TypeScript contract parity exists in `packages/contracts/src/tools.ts`.

5. **UTF-8 Slicing Defect in `execute_workspace_read` (`crates/aro-tools/src/lib.rs:549–553`)**:
   - In `execute_workspace_read`, output is truncated using raw byte slicing:
     ```rust
     let truncated = if content.len() > 16_000 {
         format!("{}...\n[truncated]", &content[..16_000])
     } else {
         content
     };
     ```
   - When a multibyte character (such as French accented `é` at bytes 15,999..16,001) spans across byte index 16,000, Rust panics with an unhandled runtime error.

---

### 1.2 Verification Commands & Verbatim Results

#### Test Suite 1: `cargo test -p aro-tools` — **FAILED** (Exit code 1)
```
running 20 tests
test tests::accepts_representative_global_unicast_addresses ... ok
test tests::extracts_urls_from_natural_text ... ok
test tests::executor_construction_rejects_an_unsafe_search_endpoint ... ok
test tests::fallible_executor_construction_rejects_an_invalid_http_client ... ok
test tests::parses_duckduckgo_result_links ... ok
test tests::app_launch_rejects_empty_target ... ok
test tests::html_to_clean_markdown_strips_scripts_and_tags ... ok
test tests::canonical_and_legacy_ids_dispatch_to_the_same_guarded_executors ... ok
test tests::policy_matches_exact_wildcard_and_subdomains ... ok
test tests::rejects_private_shared_and_non_global_ip_ranges ... ok
test tests::requested_domains_are_strictly_intersected_with_policy ... ok
test tests::search_redirects_reapply_endpoint_security_rules ... ok
test tests::shell_blocks_obfuscated_destructive_commands ... ok
test tests::search_endpoint_rejects_non_https_and_local_destinations_without_network ... ok
test tests::web_request_heuristic_detects_french_search_intent ... ok
test tests::workspace_paths_cannot_escape_their_root ... ok
test tests::notification_and_email_tools_execute_successfully ... ok
test tests::app_launch_reports_missing_app_truthfully ... ok
test tests::test_workspace_tools_with_root_path_and_context_sources ... ok
test tests::computer_use_tools_execute_successfully ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.90s

     Running tests\adversarial_confinement_tests.rs (target\debug\deps\adversarial_confinement_tests-3f4d80f14901cb17.exe)

running 8 tests
test test_adversarial_symlink_traversal_detection ... ok
test test_adversarial_directory_traversal_variations ... ok
test test_adversarial_null_byte_injections ... ok
test test_adversarial_workspace_root_deletion_blocked ... ok
test test_adversarial_workspace_read_multibyte_boundary ... FAILED
test test_adversarial_secret_leak_in_shell_and_code_execution ... ok
test test_adversarial_64kb_output_capping_and_utf8_preservation ... ok
test test_adversarial_absolute_path_and_prefix_injection ... ok

failures:

---- test_adversarial_workspace_read_multibyte_boundary stdout ----

thread 'test_adversarial_workspace_read_multibyte_boundary' (16908) panicked at crates\aro-tools\src\lib.rs:550:51:
end byte index 16000 is not a char boundary; it is inside 'é' (bytes 15999..16001) of `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`[...]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    test_adversarial_workspace_read_multibyte_boundary

test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.33s

error: test failed, to rerun pass `-p aro-tools --test adversarial_confinement_tests`
```

#### Test Suite 2: `cargo test -p aro-runtime` — **PASSED** (Exit code 0)
`test result: ok. 26 passed (unit); 47 passed (integration); total 73 passed; 0 failed`

#### Test Suite 3: `cargo test -p aro-agent` — **PASSED** (Exit code 0)
`test result: ok. 24 passed (unit); 6 passed (stress); total 30 passed; 0 failed`

#### Test Suite 4: `cargo test -p aro-memory` — **PASSED** (Exit code 0)
`test result: ok. 18 passed (unit); 28 passed (stress); total 46 passed; 0 failed`

#### Test Suite 5: `cargo test -p aro-core` — **PASSED** (Exit code 0)
`test result: ok. 42 passed (unit); 35 passed (stress/contract/fuzz); total 77 passed; 0 failed`

#### Test Suite 6: `npm run lint:rust` — **FAILED** (Exit code 1)
```
> aro@0.1.0 lint:rust
> cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings

Diff in \\?\C:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\crates\aro-runtime\tests\adversarial_authorization_challenge_tests.rs:72
Diff in \\?\C:\Users\Stagiaire\Documents\Amadou PGC\Prs\Aro\crates\aro-tools\tests\adversarial_confinement_tests.rs:20
```

And when checking `cargo clippy --workspace --all-targets -- -D warnings`:
```
error: unused imports: `TOOL_CORE_SHELL_EXECUTE` and `TOOL_CORE_WORKSPACE_READ`
  --> crates\aro-runtime\tests\adversarial_authorization_challenge_tests.rs:23:54
   |
   = note: `-D unused-imports` implied by `-D warnings`

error: useless use of `vec!`
   --> crates\aro-tools\tests\adversarial_confinement_tests.rs:122:23
    |
    = note: `-D clippy::useless-vec` implied by `-D warnings`

error: useless use of `vec!`
   --> crates\aro-tools\tests\adversarial_confinement_tests.rs:195:32
    |
    = note: `-D clippy::useless-vec` implied by `-D warnings`
```

---

## 2. Logic Chain

1. **Test Suite Execution Requirement**: The system invariants and dispatch instructions mandate that `cargo test -p aro-tools` and `npm run lint:rust` must pass completely without failure.
2. **Empirical Verification of `cargo test -p aro-tools`**: Running `cargo test -p aro-tools` produces Exit Code 1. Specifically, `adversarial_confinement_tests::test_adversarial_workspace_read_multibyte_boundary` panics in `crates\aro-tools\src\lib.rs:550:51` because `&content[..16_000]` violates Rust UTF-8 character boundary constraints.
3. **Empirical Verification of Quality Invariants**: `ORIGINAL_REQUEST.md` (lines 38) explicitly mandates: `Le code Rust compile et passe l'audit de qualité sans aucun warning (npm run lint:rust)`. Running `npm run lint:rust` produces Exit Code 1 due to rustfmt formatting differences and clippy compilation errors with `-D warnings`.
4. **Discrepancy with Worker Handoff**: `teamwork_preview_worker_m2_1/handoff.md` claimed 0 failures and 0 warnings for these suites. Empirical verification directly invalidates that claim.
5. **Auditor Constraint**: As a Forensic Auditor, rule is strict: "Block on failure: If ANY check fails, the verdict is INTEGRITY VIOLATION and the work product must be rejected." "Report any failures as findings — do NOT fix them yourself."

---

## 3. Caveats

No caveats. All commands were run directly in the project root on the target operating system (Windows) with live outputs recorded above.

---

## 4. Conclusion

**Verdict: INTEGRITY VIOLATION**

Milestone 2 implementation exhibits two critical blocking integrity defects:
1. **Failing Test in `aro-tools`**: `cargo test -p aro-tools` exits with code 1 due to an unhandled UTF-8 character boundary panic in `execute_workspace_read` (`crates/aro-tools/src/lib.rs:550`).
2. **Failing Rust Linting**: `npm run lint:rust` fails with code 1 due to unformatted files, unused imports, and clippy warnings treated as errors.

The work product must be rejected until these two defects are resolved by a worker agent.

---

## 5. Verification Method

To independently reproduce this finding:
```powershell
# 1. Run aro-tools test suite to observe UTF-8 boundary panic in test_adversarial_workspace_read_multibyte_boundary:
cargo test -p aro-tools

# 2. Run rust lint check to observe formatting and clippy rejections:
npm run lint:rust
cargo clippy --workspace --all-targets -- -D warnings
```

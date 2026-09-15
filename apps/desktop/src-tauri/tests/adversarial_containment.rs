// Adversarial Stress Harness for Path Containment Logic
// Direct empirical challenge of resolve_safe_workspace_path and apply_unified_patch

use std::path::{Path, PathBuf};

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn resolve_safe_workspace_path(root: &Path, relative_or_absolute: &str) -> Result<PathBuf, String> {
    let raw_path = relative_or_absolute.trim();
    if raw_path.is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    let cand_path = Path::new(raw_path);
    let combined = if cand_path.is_absolute() {
        cand_path.to_path_buf()
    } else {
        root.join(cand_path)
    };

    // Normalize path components to prevent .. traversal escaping root
    let mut normalized = PathBuf::new();
    for comp in combined.components() {
        match comp {
            std::path::Component::Prefix(_p) => normalized.push(comp.as_os_str()),
            std::path::Component::RootDir => normalized.push(comp.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "Access denied: Path traversal detected outside workspace root: {}",
                        raw_path
                    ));
                }
            }
            std::path::Component::Normal(c) => normalized.push(c),
        }
    }

    let clean_root = strip_verbatim_prefix(root);
    let clean_normalized = strip_verbatim_prefix(&normalized);

    // Resolve canonical root for comparing canonicalized paths (handles Windows 8.3 short names and casing)
    let canon_root = std::fs::canonicalize(root)
        .map(|p| strip_verbatim_prefix(&p))
        .unwrap_or_else(|_| clean_root.clone());

    // Check 1: Target file exists on disk
    if let Ok(canon) = std::fs::canonicalize(&clean_normalized) {
        let clean_canon = strip_verbatim_prefix(&canon);
        if !clean_canon.starts_with(&canon_root) && !clean_canon.starts_with(&clean_root) {
            return Err(format!(
                "Access denied: File path outside workspace root: {}",
                raw_path
            ));
        }
        return Ok(clean_canon);
    }

    // Check 2: Target does not exist yet.
    // Iteratively walk up the directory tree to find the closest existing ancestor.
    // Canonicalizing the existing ancestor resolves any junctions or symlinks in the path prefix.
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
            if !clean_ancestor.starts_with(&canon_root) && !clean_ancestor.starts_with(&clean_root)
            {
                return Err(format!(
                    "Access denied: Parent directory outside workspace root: {}",
                    raw_path
                ));
            }
        } else {
            return Err(format!(
                "Access denied: Unable to resolve ancestor directory: {}",
                raw_path
            ));
        }
    } else {
        return Err(format!(
            "Access denied: Workspace root or parent directory outside workspace root: {}",
            raw_path
        ));
    }

    // Check 3: Lexical containment fallback
    if !clean_normalized.starts_with(&clean_root) && !clean_normalized.starts_with(&canon_root) {
        return Err(format!(
            "Access denied: File path outside workspace root: {}",
            raw_path
        ));
    }

    Ok(clean_normalized)
}

fn main() {
    println!("=== RUNNING ADVERSARIAL PATH CONTAINMENT HARNESS ===");

    let base_temp = std::env::temp_dir().join(format!("aro_adv_{}", std::process::id()));
    let ws_root = base_temp.join("workspace");
    let outside_dir = base_temp.join("outside_target");
    let sibling_dir = base_temp.join("workspace_sibling");

    std::fs::create_dir_all(&ws_root).expect("failed to create ws_root");
    std::fs::create_dir_all(&outside_dir).expect("failed to create outside_dir");
    std::fs::create_dir_all(&sibling_dir).expect("failed to create sibling_dir");

    // Canonicalize ws_root as resolve_effective_root_path does
    let canon_ws = strip_verbatim_prefix(&std::fs::canonicalize(&ws_root).unwrap());
    println!("Canonical workspace root: {:?}", canon_ws);

    let mut passed = 0;
    let mut failed = 0;

    macro_rules! assert_rejected {
        ($input:expr, $msg:expr) => {
            match resolve_safe_workspace_path(&canon_ws, $input) {
                Err(err) => {
                    println!("[PASS: REJECTED] {} => {}", $input, err);
                    passed += 1;
                }
                Ok(path) => {
                    println!("[FAIL: ACCEPTED ESCAPE!] {} => {:?}", $input, path);
                    failed += 1;
                }
            }
        };
    }

    macro_rules! assert_accepted {
        ($input:expr, $msg:expr) => {
            match resolve_safe_workspace_path(&canon_ws, $input) {
                Ok(path) => {
                    println!("[PASS: ACCEPTED] {} => {:?}", $input, path);
                    passed += 1;
                }
                Err(err) => {
                    println!("[FAIL: REJECTED VALID!] {} => {}", $input, err);
                    failed += 1;
                }
            }
        };
    }

    // --- Category 1: Classic & Tricky Traversal ---
    println!("\n--- 1. Relative Traversal Attacks ---");
    assert_rejected!("../secret.txt", "1 level parent traversal");
    assert_rejected!("..\\secret.txt", "1 level backslash parent traversal");
    assert_rejected!("../../secret.txt", "2 level parent traversal");
    assert_rejected!("..\\..\\secret.txt", "2 level backslash parent traversal");
    assert_rejected!(
        "../../../../../../../../Windows/System32/cmd.exe",
        "deep root traversal"
    );
    assert_rejected!("sub/../../secret.txt", "sub then 2 levels up");
    assert_rejected!("sub\\..\\..\\secret.txt", "sub then 2 levels up backslash");
    assert_rejected!("sub/nested/../../../secret.txt", "2 sub then 3 up");
    assert_rejected!("sub/./../../secret.txt", "sub with curdir then 2 up");
    assert_rejected!("   ", "whitespace only");
    assert_rejected!("", "empty string");

    // --- Category 2: Absolute Path Escapes ---
    println!("\n--- 2. Absolute Path Escapes ---");
    assert_rejected!(
        "C:\\Windows\\System32\\cmd.exe",
        "Windows absolute system path"
    );
    assert_rejected!(
        "c:\\windows\\system32\\cmd.exe",
        "lowercase drive letter absolute path"
    );
    assert_rejected!("C:/Windows/System32/cmd.exe", "forward slash absolute path");
    assert_rejected!("D:\\external\\file.txt", "different drive letter");
    assert_rejected!("\\Windows\\System32", "root-relative path");
    assert_rejected!("/Windows/System32", "forward slash root-relative");

    // --- Category 3: Sibling Directory Prefix Attack ---
    println!("\n--- 3. Sibling Directory Prefix Attack ---");
    // e.g. root is /workspace, sibling is /workspace_sibling
    let sibling_file = sibling_dir.join("secret.txt");
    std::fs::write(&sibling_file, "secret data").unwrap();
    let sibling_str = sibling_file.to_string_lossy();
    assert_rejected!(
        &sibling_str,
        "sibling directory with prefix matching root name"
    );

    // --- Category 4: Valid Safe In-Workspace Paths ---
    println!("\n--- 4. Valid Workspace Paths ---");
    // Existing file
    let existing_file = ws_root.join("hello.txt");
    std::fs::write(&existing_file, "hello").unwrap();
    assert_accepted!("hello.txt", "existing file in root");
    assert_accepted!("sub/new_file.txt", "new file in non-existent sub directory");
    assert_accepted!("./hello.txt", "dot slash existing file");
    assert_accepted!(".\\hello.txt", "dot backslash existing file");
    assert_accepted!("sub/inner/deep/file.rs", "deeply nested new file");

    // Test lowercase root path sensitivity
    let lower_ws_str = canon_ws.to_string_lossy().to_string();
    if lower_ws_str.chars().nth(1) == Some(':') {
        let mut chars: Vec<char> = lower_ws_str.chars().collect();
        chars[0] = chars[0].to_ascii_lowercase();
        let lower_root = PathBuf::from(chars.into_iter().collect::<String>());
        println!("[Testing lowercase root: {:?}]", lower_root);
        match resolve_safe_workspace_path(&lower_root, "hello.txt") {
            Ok(p) => println!("[PASS: Lowercase root works] {:?}", p),
            Err(e) => {
                println!("[FAIL: Lowercase root rejected existing file!] {}", e);
                failed += 1;
            }
        }
    }

    // --- Category 5: Symlink / Junction Traversal ---
    println!("\n--- 5. Symlink / Junction Traversal Attacks ---");
    // Create junction or directory symlink inside ws_root pointing to outside_dir
    let link_path = ws_root.join("ext_junction");
    #[cfg(target_os = "windows")]
    {
        // Try creating directory junction via cmd /c mklink /J
        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link_path.to_str().unwrap(),
                outside_dir.to_str().unwrap(),
            ])
            .output();

        if let Ok(out) = status {
            if out.status.success() {
                println!("Created directory junction at {:?}", link_path);

                // Subcase 5a: Existing file through junction
                let target_secret = outside_dir.join("outside_secret.txt");
                std::fs::write(&target_secret, "outside content").unwrap();

                // Path: ext_junction/outside_secret.txt
                println!("[Test 5a: Existing file through junction]");
                assert_rejected!(
                    "ext_junction/outside_secret.txt",
                    "existing file through junction"
                );

                // Subcase 5b: New file directly in junction
                println!("[Test 5b: New file in junction root]");
                assert_rejected!(
                    "ext_junction/new_outside_file.txt",
                    "new file in junction root"
                );

                // Subcase 5c: New file in NON-EXISTENT subfolder inside junction
                println!("[Test 5c: New file in non-existent subfolder inside junction]");
                match resolve_safe_workspace_path(
                    &canon_ws,
                    "ext_junction/non_existent_folder/file.txt",
                ) {
                    Ok(leaked_path) => {
                        println!("[VULNERABILITY CONFIRMED: PATH ACCEPTED] {:?}", leaked_path);
                        // Simulate workspace_file_write
                        if let Some(p) = leaked_path.parent() {
                            let _ = std::fs::create_dir_all(p);
                        }
                        let _ = std::fs::write(&leaked_path, "EXPLOIT CONTENT");

                        // Check if file was written into outside_dir
                        let written_in_outside =
                            outside_dir.join("non_existent_folder").join("file.txt");
                        if written_in_outside.exists() {
                            println!("[CRITICAL EXPLOIT PROVED] Arbitrary file write outside workspace root! File exists at: {:?}", written_in_outside);
                        }
                        failed += 1;
                    }
                    Err(err) => {
                        println!("[PASS: REJECTED] {}", err);
                        passed += 1;
                    }
                }
            } else {
                println!(
                    "Note: Could not create junction (permissions). Skipping junction subtests."
                );
            }
        }
    }

    // --- Category 6: Unified Patch Application Logic Stress ---
    println!("\n--- 6. Unified Patch Multi-Hunk Stress ---");
    let original_multi =
        "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10";
    let multi_hunk_patch = "--- a/test.txt\n+++ b/test.txt\n@@ -2,1 +2,3 @@\n-line 2\n+line 2a\n+line 2b\n+line 2c\n@@ -9,1 +11,1 @@\n-line 9\n+modified line 9\n";

    match apply_unified_patch(original_multi, multi_hunk_patch) {
        Ok(patched) => {
            println!("[Patched Result]:\n{}", patched);
            if patched.contains("modified line 9")
                && !patched.contains("\nline 9\n")
                && patched.contains("line 7")
            {
                println!("[PASS: Multi-hunk patch applied correctly]");
                passed += 1;
            } else {
                println!("[FAIL: Multi-hunk corrupted file lines!]");
                if !patched.contains("modified line 9") {
                    println!("  Missing 'modified line 9'");
                }
                if patched.contains("\nline 9\n") {
                    println!("  Original 'line 9' was not replaced!");
                }
                if !patched.contains("line 7") {
                    println!("  Corrupted 'line 7' was erroneously removed instead of line 9!");
                }
                failed += 1;
            }
        }
        Err(e) => {
            println!("[FAIL: apply_unified_patch error]: {}", e);
            failed += 1;
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&base_temp);

    println!("\n==========================================");
    println!("TOTAL: Passed: {}, Failed: {}", passed, failed);
    println!("==========================================");

    if failed > 0 {
        std::process::exit(1);
    }
}

fn apply_unified_patch(original: &str, patch: &str) -> Result<String, String> {
    // Guard against empty diff patches: a no-op diff preserves original content
    if patch.trim().is_empty() {
        return Ok(original.to_string());
    }

    // Direct replacement if patch has no unified diff markers
    if !patch.contains("@@") {
        if patch.starts_with("diff --git") || patch.starts_with("--- ") || patch.starts_with("+++ ")
        {
            return Ok(original.to_string());
        }
        return Ok(patch.to_string());
    }

    let has_crlf = original.contains("\r\n");
    let line_ending = if has_crlf { "\r\n" } else { "\n" };

    let mut current_lines: Vec<String> = if original.is_empty() {
        Vec::new()
    } else {
        original.lines().map(|s| s.to_string()).collect()
    };

    let patch_lines: Vec<&str> = patch.lines().collect();
    let mut i = 0;
    let mut hunk_applied = false;
    let mut accumulated_offset: isize = 0;

    while i < patch_lines.len() {
        let line = patch_lines[i];
        if line.starts_with("@@ ") {
            hunk_applied = true;
            let parts: Vec<&str> = line.split("@@").collect();
            if parts.len() < 3 {
                i += 1;
                continue;
            }
            let header = parts[1].trim();
            let ranges: Vec<&str> = header.split_whitespace().collect();
            if ranges.is_empty() {
                i += 1;
                continue;
            }
            let old_spec = ranges[0].trim_start_matches('-');
            let old_start: usize = old_spec
                .split(',')
                .next()
                .unwrap_or("1")
                .parse()
                .unwrap_or(1);
            let base_start = if old_start > 0 {
                old_start as isize - 1
            } else {
                0
            };
            let adjusted_start = base_start + accumulated_offset;
            let mut line_cursor = if adjusted_start < 0 {
                0
            } else {
                (adjusted_start as usize).min(current_lines.len())
            };

            let len_before = current_lines.len();

            i += 1;
            while i < patch_lines.len() {
                let hline = patch_lines[i];
                if hline.starts_with("@@ ") || hline.starts_with("diff --git") {
                    break;
                }
                if let Some(content) = hline.strip_prefix('+') {
                    if line_cursor <= current_lines.len() {
                        current_lines.insert(line_cursor, content.to_string());
                    } else {
                        current_lines.push(content.to_string());
                    }
                    line_cursor += 1;
                } else if hline.starts_with('-') {
                    if line_cursor < current_lines.len() {
                        current_lines.remove(line_cursor);
                    }
                } else if hline.starts_with(' ') || hline.is_empty() {
                    line_cursor += 1;
                }
                i += 1;
            }

            let len_after = current_lines.len();
            accumulated_offset += (len_after as isize) - (len_before as isize);
        } else {
            i += 1;
        }
    }

    if hunk_applied {
        let mut res = current_lines.join(line_ending);
        if has_crlf {
            if original.ends_with("\r\n") && !res.ends_with("\r\n") && !res.is_empty() {
                res.push_str("\r\n");
            }
        } else if original.ends_with('\n') && !res.ends_with('\n') && !res.is_empty() {
            res.push('\n');
        }
        Ok(res)
    } else if patch.starts_with("diff --git")
        || patch.starts_with("--- ")
        || patch.starts_with("+++ ")
    {
        Ok(original.to_string())
    } else {
        Ok(patch.to_string())
    }
}

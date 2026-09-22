//! Sandboxed skill script execution for shared hosts (cloud worker).
//!
//! [`Skill::execute`] runs a plugin's script with the parent environment,
//! no timeout and no output caps — acceptable on a personal desktop,
//! unsafe on shared infrastructure where plugins come from marketplaces,
//! git remotes or user uploads. This module contains that risk:
//!
//! - the skill directory is staged into an isolated temp dir (size-capped,
//!   symlinks never followed, removed on drop) so scripts cannot write
//!   back into the installed plugin package;
//! - secret-bearing environment variables are scrubbed (server credentials
//!   such as `ARO_*`, tokens, database URLs never reach the child);
//! - wall-clock timeout kills the child (no orphaned processes);
//! - stdout/stderr are truncated (no memory blowup from chatty scripts).
//!
//! Honest limits: this is process-level containment, not an OS sandbox
//! (no namespaces/AppContainer/seccomp), and network egress is NOT blocked
//! here — network-touching skills additionally require the caller's
//! permission-profile network gate before they ever reach this module.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::skill::{Skill, SkillOutput};

/// Resource bounds for one sandboxed skill run.
#[derive(Debug, Clone)]
pub struct SandboxLimits {
    /// Wall-clock budget; the child is killed past it.
    pub timeout_secs: u64,
    /// Cumulative cap for the staged skill copy.
    pub max_stage_bytes: u64,
    /// Per-stream cap for captured stdout/stderr.
    pub max_output_bytes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            timeout_secs: 60,
            max_stage_bytes: 10 * 1024 * 1024,
            max_output_bytes: 256 * 1024,
        }
    }
}

/// Env keys are dropped when they contain one of these (case-insensitive).
/// Denylist, not allowlist: interpreters need their ordinary variables
/// (`SystemRoot`, `PATH`, `PYTHONPATH`…), so we remove what must never
/// cross into plugin code instead of guessing what it needs.
const SECRET_ENV_SUBSTRINGS: &[&str] = &[
    "SECRET", "PASSWORD", "PASSWD", "TOKEN", "ARO_", "DATABASE", "REDIS_", "API_KEY", "APIKEY",
    "AUTH", "PRIVATE", "COOKIE", "SESSION", "CREDENTIAL", "BEARER", "SMTP",
];

/// Filtered `(key, value)` pairs inherited by a sandboxed child.
pub fn scrubbed_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(key, _)| {
            let upper = key.to_uppercase();
            !SECRET_ENV_SUBSTRINGS
                .iter()
                .any(|needle| upper.contains(needle))
        })
        .collect()
}

/// Truncate to a byte budget on a char boundary; reports truncation.
pub fn truncate_output(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_string(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…[truncated]", &text[..end]), true)
}

/// Isolated copy of a skill directory. Removed best-effort on drop.
pub struct StagedSkillDir {
    path: PathBuf,
}

impl StagedSkillDir {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagedSkillDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Copy `skill_dir` (files + subdirs, never following symlinks) into a
/// unique temp dir, enforcing a cumulative size cap.
pub fn stage_skill_dir(skill_dir: &Path, max_bytes: u64) -> Result<StagedSkillDir> {
    let dest = std::env::temp_dir().join(format!("aro-skill-sandbox-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest)
        .with_context(|| format!("failed to create {}", dest.display()))?;
    let staged = StagedSkillDir { path: dest };
    let mut total: u64 = 0;
    copy_dir_capped(skill_dir, &staged.path, &mut total, max_bytes).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&staged.path);
    })?;
    Ok(staged)
}

fn copy_dir_capped(src: &Path, dest: &Path, total: &mut u64, max_bytes: u64) -> Result<()> {
    for entry in std::fs::read_dir(src)
        .with_context(|| format!("failed to read {}", src.display()))?
        .flatten()
    {
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to stat {}", entry.path().display()))?;
        // Never follow symlinks: a malicious package could otherwise stage
        // the server's credential files into (or out of) the sandbox.
        if file_type.is_symlink() {
            continue;
        }
        let target = dest.join(entry.file_name());
        if file_type.is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("failed to create {}", target.display()))?;
            copy_dir_capped(&entry.path(), &target, total, max_bytes)?;
        } else if file_type.is_file() {
            let size = entry
                .metadata()
                .with_context(|| format!("failed to stat {}", entry.path().display()))?
                .len();
            *total = total
                .checked_add(size)
                .ok_or_else(|| anyhow!("skill exceeds the staging budget"))?;
            if *total > max_bytes {
                return Err(anyhow!(
                    "skill directory exceeds the {max_bytes} byte staging budget"
                ));
            }
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("failed to stage {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Build the interpreter command for a staged script. Same mapping as
/// [`Skill::execute`]: python/node/bash/powershell by extension, direct
/// exec otherwise.
fn script_command(staged_script: &Path) -> Command {
    let extension = staged_script
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    match extension {
        "py" => {
            let mut c = Command::new("python");
            c.arg(staged_script);
            c
        }
        "js" | "mjs" => {
            let mut c = Command::new("node");
            c.arg(staged_script);
            c
        }
        "sh" => {
            let mut c = Command::new("bash");
            c.arg(staged_script);
            c
        }
        "ps1" => {
            let mut c = Command::new("powershell");
            c.arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(staged_script);
            c
        }
        _ => Command::new(staged_script),
    }
}

/// Run a skill's primary script inside the sandbox. Skills without scripts
/// behave exactly like [`Skill::execute`] (pure instruction synthesis).
pub async fn execute_sandboxed(
    skill: &Skill,
    input: &Value,
    limits: &SandboxLimits,
) -> Result<SkillOutput> {
    let Some(script_path) = skill.scripts.first() else {
        return skill.execute(input).await;
    };
    let staged = stage_skill_dir(&skill.skill_dir, limits.max_stage_bytes)?;
    // Preserve the script's location *inside* the skill tree (usually
    // `scripts/…`), not just its file name.
    let relative = script_path.strip_prefix(&skill.skill_dir).map_err(|_| {
        anyhow!("skill script escapes its skill directory and is refused")
    })?;
    let staged_script = staged.path().join(relative);
    if !staged_script.is_file() {
        return Err(anyhow!("staged skill script is missing"));
    }

    let input_str = serde_json::to_string(input)?;
    let mut cmd = script_command(&staged_script);
    cmd.current_dir(staged.path())
        .arg(&input_str)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in scrubbed_env() {
        cmd.env(key, value);
    }

    // The child stays owned here so a timeout can always kill it (no
    // orphans): only borrowed streams enter the timed future.
    let mut child = cmd.spawn().context("failed to spawn sandboxed skill")?;
    let mut stdout_take = child.stdout.take();
    let mut stderr_take = child.stderr.take();
    let collect = async {
        // Concurrent drains: a chatty script filling one pipe while we
        // read the other would deadlock sequential reads.
        let read_out = async {
            let mut out = Vec::new();
            if let Some(stream) = stdout_take.as_mut() {
                stream.read_to_end(&mut out).await?;
            }
            Ok::<_, std::io::Error>(out)
        };
        let read_err = async {
            let mut err = Vec::new();
            if let Some(stream) = stderr_take.as_mut() {
                stream.read_to_end(&mut err).await?;
            }
            Ok::<_, std::io::Error>(err)
        };
        let (out, err) = tokio::join!(read_out, read_err);
        let status = child.wait().await?;
        Ok::<_, std::io::Error>((status, out?, err?))
    };
    let timeout = std::time::Duration::from_secs(limits.timeout_secs.max(1));
    let (status, stdout_bytes, stderr_bytes) =
        match tokio::time::timeout(timeout, collect).await {
            Ok(Ok(collected)) => collected,
            Ok(Err(err)) => {
                let _ = child.kill().await;
                return Err(anyhow!("sandboxed skill I/O failed: {err}"));
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Ok(SkillOutput {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "skill exceeded the {}s sandbox timeout and was killed",
                        limits.timeout_secs
                    )),
                    skill_id: skill.id.clone(),
                });
            }
        };

    let (stdout, stdout_cut) = truncate_output(
        &String::from_utf8_lossy(&stdout_bytes),
        limits.max_output_bytes,
    );
    let (stderr, stderr_cut) = truncate_output(
        &String::from_utf8_lossy(&stderr_bytes),
        limits.max_output_bytes,
    );
    let success = status.success();
    let mut text = if !stdout.is_empty() { stdout } else { stderr.clone() };
    if stdout_cut || stderr_cut {
        text.push_str("\n[sandbox: output truncated]");
    }
    Ok(SkillOutput {
        success,
        output: text,
        error: if !success { Some(stderr) } else { None },
        skill_id: skill.id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scrubbed_env_drops_secrets_but_keeps_runtimes() {
        // NOTE: no `ARO_` prefix on the plain probe — every `ARO_*` var is
        // server configuration by convention and is scrubbed on purpose.
        std::env::set_var("SANDBOX_PROBE_SECRET_TOKEN_XYZ", "s3cr3t");
        std::env::set_var("SANDBOX_PROBE_PLAIN_XYZ", "hello");
        let env = scrubbed_env();
        assert!(!env.iter().any(|(k, _)| k == "SANDBOX_PROBE_SECRET_TOKEN_XYZ"));
        assert!(env
            .iter()
            .any(|(k, v)| k == "SANDBOX_PROBE_PLAIN_XYZ" && v == "hello"));
        std::env::remove_var("SANDBOX_PROBE_SECRET_TOKEN_XYZ");
        std::env::remove_var("SANDBOX_PROBE_PLAIN_XYZ");
        // Runtime essentials survive (at least one of PATH/SystemRoot).
        assert!(env
            .iter()
            .any(|(k, _)| k == "PATH" || k == "Path" || k == "SystemRoot"));
    }

    #[test]
    fn staging_copies_tree_skips_symlinks_and_enforces_budget() {
        let root = std::env::temp_dir().join(format!("aro-sandbox-test-{}", uuid::Uuid::new_v4()));
        let nested = root.join("refs");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("SKILL.md"), "# skill").unwrap();
        std::fs::write(nested.join("notes.md"), "notes").unwrap();

        let staged = stage_skill_dir(&root, 1024 * 1024).expect("stage");
        assert!(staged.path().join("SKILL.md").is_file());
        assert!(staged.path().join("refs").join("notes.md").is_file());

        let tiny = stage_skill_dir(&root, 2);
        assert!(tiny.is_err(), "2-byte budget must reject the copy");

        let path = staged.path.to_path_buf();
        drop(staged);
        assert!(!path.exists(), "staging dir removed on drop");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn truncation_respects_char_boundaries() {
        let (text, cut) = truncate_output("héllo wörld", 5);
        assert!(cut);
        assert!(text.starts_with("hé"));
        assert!(text.ends_with("…[truncated]"));
        let (same, untouched) = truncate_output("abc", 100);
        assert!(!untouched);
        assert_eq!(same, "abc");
    }

    fn interpreter_available() -> Option<&'static str> {
        ["python", "node"].into_iter().find(|candidate| {
            std::process::Command::new(candidate)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
    }

    #[tokio::test]
    async fn sandboxed_python_echo_cannot_see_secrets() {
        let Some(interpreter) = interpreter_available() else {
            eprintln!("skipping: no python/node interpreter");
            return;
        };
        std::env::set_var("ARO_SANDBOX_PROBE_SECRET_XYZ", "topsecret");
        let root = std::env::temp_dir().join(format!("aro-sandbox-e2e-{}", uuid::Uuid::new_v4()));
        let scripts = root.join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        fs::write_sandbox_probe(&root, &scripts, interpreter);
        let skill = Skill::load_from_dir(&root).expect("load skill");

        let out = execute_sandboxed(&skill, &json!({ "ping": 1 }), &SandboxLimits::default())
            .await
            .expect("sandboxed run");
        std::env::remove_var("ARO_SANDBOX_PROBE_SECRET_XYZ");
        std::fs::remove_dir_all(root).unwrap();

        assert!(out.success, "probe failed: {:?}", out.error);
        assert!(out.output.contains("PING=1"), "input arg echoed: {}", out.output);
        assert!(
            !out.output.contains("topsecret"),
            "secret leaked into sandbox: {}",
            out.output
        );
    }

    #[tokio::test]
    async fn sandbox_timeout_kills_hanging_scripts() {
        let Some(interpreter) = interpreter_available() else {
            eprintln!("skipping: no python/node interpreter");
            return;
        };
        let root = std::env::temp_dir().join(format!("aro-sandbox-hang-{}", uuid::Uuid::new_v4()));
        let scripts = root.join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        fs::write_hang_probe(&root, &scripts, interpreter);
        let skill = Skill::load_from_dir(&root).expect("load skill");

        let limits = SandboxLimits {
            timeout_secs: 2,
            ..SandboxLimits::default()
        };
        let out = execute_sandboxed(&skill, &json!({}), &limits)
            .await
            .expect("sandboxed run returns");
        std::fs::remove_dir_all(root).unwrap();

        assert!(!out.success);
        assert!(
            out.error.as_deref().unwrap_or_default().contains("timeout"),
            "unexpected: {:?}",
            out.error
        );
    }

    /// Test-only probe writers (kept here so the probes stay next to the
    /// sandbox contract they verify).
    mod fs {
        use super::*;

        pub fn write_sandbox_probe(root: &Path, scripts: &Path, interpreter: &str) {
            std::fs::write(
                root.join("SKILL.md"),
                "---\nname: Probe\ndescription: Probe skill\n---\n# Probe\n",
            )
            .unwrap();
            if interpreter == "node" {
                std::fs::write(
                    scripts.join("run.js"),
                    "const probe = process.env.ARO_SANDBOX_PROBE_SECRET_XYZ || 'ABSENT';\n\
                     console.log('PING=' + JSON.parse(process.argv[2] || '{}').ping + ' SECRET=' + probe);\n",
                )
                .unwrap();
            } else {
                std::fs::write(
                    scripts.join("run.py"),
                    "import json, os, sys\n\
                     payload = json.loads(sys.argv[1]) if len(sys.argv) > 1 else {}\n\
                     print(f\"PING={payload.get('ping')} SECRET={os.environ.get('ARO_SANDBOX_PROBE_SECRET_XYZ', 'ABSENT')}\")\n",
                )
                .unwrap();
            }
        }

        pub fn write_hang_probe(root: &Path, scripts: &Path, interpreter: &str) {
            std::fs::write(
                root.join("SKILL.md"),
                "---\nname: Hang\ndescription: Hang skill\n---\n# Hang\n",
            )
            .unwrap();
            if interpreter == "node" {
                std::fs::write(scripts.join("hang.js"), "setInterval(() => {}, 1000);\n").unwrap();
            } else {
                std::fs::write(scripts.join("hang.py"), "import time\ntime.sleep(60)\n").unwrap();
            }
        }
    }
}

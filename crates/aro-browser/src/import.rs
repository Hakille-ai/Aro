//! Import from system browsers (Chrome / Edge).
//!
//! What is imported today (no crypto involved):
//! - bookmarks (`Bookmarks` JSON — readable by design);
//! - history (`History` SQLite, copied first because the running browser
//!   locks it; visit counts + titles only, no page contents).
//!
//! System-browser PASSWORDS are deliberately NOT imported here: Chrome/Edge
//! encrypt them (DPAPI on Windows, Keychain on macOS, NSS on Linux) and each
//! OS needs its own unlock flow with explicit user consent. Requesting that
//! returns [`ImportError::PasswordsUnsupported`] with an actionable message
//! instead of a fake success. (Phase 5 item.)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("no system browser profile found (looked for Chrome/Edge)")]
    NoProfile,
    #[error("import failed: {0}")]
    Failed(String),
    #[error(
        "system-browser password import is not supported yet: save credentials in the ARO vault instead (Settings → Browser → Vault)"
    )]
    PasswordsUnsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedBookmark {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedHistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: i64,
}

/// Candidate profile roots, system channel first.
fn candidate_profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            for sub in [
                r"Google\Chrome\User Data\Default",
                r"Microsoft\Edge\User Data\Default",
            ] {
                dirs.push(PathBuf::from(format!(r"{local}\{sub}")));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            for sub in [
                "Library/Application Support/Google/Chrome/Default",
                "Library/Application Support/Microsoft Edge/Default",
            ] {
                dirs.push(PathBuf::from(format!("{home}/{sub}")));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            for sub in [
                ".config/google-chrome/Default",
                ".config/microsoft-edge/Default",
                ".config/chromium/Default",
            ] {
                dirs.push(PathBuf::from(format!("{home}/{sub}")));
            }
        }
    }
    dirs.into_iter().filter(|d| d.is_dir()).collect()
}

fn first_profile_dir() -> Result<PathBuf, ImportError> {
    candidate_profile_dirs().into_iter().next().ok_or(ImportError::NoProfile)
}

#[derive(Debug, Deserialize)]
struct BookmarkNode {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    node_type: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    children: Vec<BookmarkNode>,
}

fn collect_bookmarks(node: &BookmarkNode, out: &mut Vec<ImportedBookmark>) {
    if node.node_type == "url" {
        if let Some(url) = &node.url {
            if url.starts_with("http") {
                out.push(ImportedBookmark { name: node.name.clone(), url: url.clone() });
            }
        }
    }
    for child in &node.children {
        collect_bookmarks(child, out);
    }
}

/// Import bookmarks from the first system-browser profile found.
pub fn import_bookmarks() -> Result<Vec<ImportedBookmark>, ImportError> {
    let dir = first_profile_dir()?;
    let raw = std::fs::read_to_string(dir.join("Bookmarks")).map_err(|err| ImportError::Failed(err.to_string()))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|err| ImportError::Failed(err.to_string()))?;
    let mut out = Vec::new();
    if let Some(roots) = value.get("roots").and_then(|r| r.as_object()) {
        for (_, root) in roots {
            if let Ok(node) = serde_json::from_value::<BookmarkNode>(root.clone()) {
                collect_bookmarks(&node, &mut out);
            }
        }
    }
    Ok(out)
}

/// Import recent history (url, title, visit count) from the first profile.
/// The SQLite file is copied to temp first — the running browser locks it.
pub fn import_history(limit: usize) -> Result<Vec<ImportedHistoryEntry>, ImportError> {
    let dir = first_profile_dir()?;
    let source = dir.join("History");
    if !source.is_file() {
        return Err(ImportError::NoProfile);
    }
    let copy = std::env::temp_dir().join(format!("aro-history-import-{}.db", uuid::Uuid::new_v4()));
    std::fs::copy(&source, &copy).map_err(|err| ImportError::Failed(err.to_string()))?;
    let result = (|| {
        let conn = rusqlite::Connection::open(&copy).map_err(|err| ImportError::Failed(err.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT url, title, visit_count FROM urls ORDER BY last_visit_time DESC LIMIT ?1")
            .map_err(|err| ImportError::Failed(err.to_string()))?;
        let rows = stmt
            .query_map([limit.min(500) as i64], |row| {
                Ok(ImportedHistoryEntry {
                    url: row.get(0)?,
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    visit_count: row.get(2)?,
                })
            })
            .map_err(|err| ImportError::Failed(err.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| ImportError::Failed(err.to_string()))?);
        }
        Ok(out)
    })();
    let _ = std::fs::remove_file(&copy);
    result
}

/// Password import entry point — honest refusal (see module docs).
pub fn import_passwords() -> Result<Vec<ImportedBookmark>, ImportError> {
    Err(ImportError::PasswordsUnsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_collector_keeps_http_urls_only() {
        let root = BookmarkNode {
            name: "root".to_string(),
            node_type: "folder".to_string(),
            url: None,
            children: vec![
                BookmarkNode {
                    name: "A".to_string(),
                    node_type: "url".to_string(),
                    url: Some("https://example.com".to_string()),
                    children: vec![],
                },
                BookmarkNode {
                    name: "B".to_string(),
                    node_type: "url".to_string(),
                    url: Some("chrome://settings".to_string()),
                    children: vec![],
                },
            ],
        };
        let mut out = Vec::new();
        collect_bookmarks(&root, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "https://example.com");
    }

    #[test]
    fn passwords_import_refuses_honestly() {
        let err = import_passwords().expect_err("must not fake password import");
        assert!(matches!(err, ImportError::PasswordsUnsupported));
    }
}

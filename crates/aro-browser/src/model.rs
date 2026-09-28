//! Shared browser model: tabs, snapshots, element refs, actions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque tab handle (one CDP target).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tab metadata mirrored to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTabInfo {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub ai_controlled: bool,
    pub ai_status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_snapshot_at: Option<DateTime<Utc>>,
}

/// One interactable element, referenced by a stable-per-snapshot id.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementRef {
    /// `ref` id valid for the snapshot it came from (e.g. `"r12"`).
    pub id: String,
    pub role: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default)]
    pub in_viewport: bool,
}

/// Full observation of a tab: screenshot + referenced elements + text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSnapshot {
    pub tab_id: TabId,
    pub url: String,
    pub title: String,
    /// PNG bytes encoded as base64 (downscaled, JPEG-quality equivalent).
    pub screenshot_base64: String,
    pub screenshot_width: u32,
    pub screenshot_height: u32,
    pub elements: Vec<ElementRef>,
    /// Readable text of the page (truncated server-side).
    pub text_excerpt: String,
    pub captured_at: DateTime<Utc>,
}

/// Actions the agent (or the user relay) can perform on a tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BrowserAction {
    Click { target_ref: String },
    Type { target_ref: String, text: String },
    Press { key: String },
    Scroll { direction: String, amount: u32 },
    Navigate { url: String },
    Back,
    Forward,
    Reload,
    Wait { text_contains: Option<String>, timeout_ms: u64 },
    Select { target_ref: String, value: String },
}

/// Autofill material. Values are redacted from every log and are never
/// returned to the model — only a success/failure outcome is reported.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutofillRequest {
    pub tab_id: TabId,
    pub username_ref: String,
    pub password_ref: String,
    pub username: crate::vault::SecretString,
    pub password: crate::vault::SecretString,
}

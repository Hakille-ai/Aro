//! ARO Browser — real Chromium automation core (CDP).
//!
//! One `BrowserService` owns a single Chromium process and a pool of tabs
//! (one CDP target each). Every observation (`snapshot`) returns a real
//! screenshot plus a referenced element tree; every action is verified by a
//! post-action snapshot. Nothing here ever reports a fake success.

pub mod act;
pub mod config;
pub mod import;
pub mod input;
pub mod model;
pub mod observe;
pub mod service;
pub mod vault;

pub use act::{execute_action, ActionOutcome, BrowserError};
pub use config::{BrowserLaunchConfig, discover_executable, default_profile_dir};
pub use import::{import_bookmarks, import_history, import_passwords, ImportError, ImportedBookmark, ImportedHistoryEntry};
pub use model::{
    AutofillRequest, BrowserAction, BrowserTabInfo, ElementRef, PageSnapshot, TabId,
};
pub use service::BrowserService;
pub use vault::{delete_credential, load_credential, save_credential, SecretString};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_types_stay_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<BrowserService>();
        assert_send_sync_static::<BrowserTabInfo>();
        assert_send_sync_static::<PageSnapshot>();
        assert_send_sync_static::<BrowserAction>();
        assert_send_sync_static::<ActionOutcome>();
        // Regression guard: aro-runtime spawns the sub-agent loop, so every
        // type reachable from tool futures must be Send + Sync + 'static.
        // (A generic `impl AsyncFn` callback once broke this with a
        // higher-ranked Send proof failure — never reintroduce one.)
    }

    #[test]
    fn secret_string_never_prints() {
        let secret = SecretString::new("hunter2".to_string());
        assert_eq!(format!("{secret:?}"), "SecretString([redacted])");
        let json = serde_json::to_string(&secret).expect("serialize");
        assert!(!json.contains("hunter2"), "{json}");
    }
}

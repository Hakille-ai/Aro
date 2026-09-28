//! Phase-0 spike proof: drive a REAL Chromium, navigate, snapshot.
//!
//! Ignored by default (needs a system Chrome/Edge + network).
//! Run with: `ARO_BROWSER_SMOKE=1 cargo test -p aro-browser -- --ignored`

use aro_browser::{BrowserAction, BrowserLaunchConfig, BrowserService};

fn smoke_enabled() -> bool {
    std::env::var("ARO_BROWSER_SMOKE").as_deref() == Ok("1")
}

#[tokio::test]
#[ignore]
async fn spike_real_chromium_navigate_snapshot_act() {
    if !smoke_enabled() {
        eprintln!("skipped: set ARO_BROWSER_SMOKE=1 to run the real-browser spike");
        return;
    }
    let profile = std::env::temp_dir().join(format!("aro-spike-{}", uuid::Uuid::new_v4()));
    let config = BrowserLaunchConfig::new(profile);
    let service = BrowserService::launch(&config)
        .await
        .expect("launch real chromium");

    let tab = service
        .open_tab(Some("https://example.com"), true)
        .await
        .expect("open tab");
    assert!(tab.url.contains("example.com"), "url={}", tab.url);

    let snapshot = service.snapshot(tab.id).await.expect("snapshot");
    assert!(!snapshot.screenshot_base64.is_empty(), "screenshot must be real bytes");
    assert!(!snapshot.title.is_empty(), "title must be observed");
    assert!(snapshot.text_excerpt.contains("Example"), "text={}", snapshot.text_excerpt);

    let outcome = service
        .run_action(tab.id, &BrowserAction::Reload)
        .await
        .expect("reload");
    assert!(outcome.success, "reload must verify: {}", outcome.detail);

    let tabs = service.list_tabs().await;
    assert_eq!(tabs.len(), 1);
    service.close_tab(tab.id).await.expect("close tab");
    assert!(service.list_tabs().await.is_empty());
}

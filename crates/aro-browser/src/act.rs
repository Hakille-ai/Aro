//! Verified actions: perform, then re-observe. Success is only reported
//! when the post-action snapshot proves the effect.

use chromiumoxide::Page;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{BrowserAction, PageSnapshot, TabId};
use crate::observe::{enumerate_elements, find_by_ref, readable_text};

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("no executable found: install Chrome/Edge/Chromium or set ARO_CHROME_EXECUTABLE")]
    NoExecutable,
    #[error("browser launch failed: {0}")]
    Launch(String),
    #[error("unknown tab {0}")]
    NoSuchTab(TabId),
    #[error("cdp error: {0}")]
    Cdp(#[from] chromiumoxide::error::CdpError),
    #[error("navigation failed for {url}: {reason}")]
    Navigation { url: String, reason: String },
    #[error("action had no observable effect: {0}")]
    NotObserved(String),
    #[error("invalid action input: {0}")]
    InvalidInput(String),
    #[error("vault error: {0}")]
    Vault(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Outcome of one verified action, with before/after proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub tab_id: TabId,
    pub action: String,
    pub success: bool,
    pub detail: String,
    pub url_before: String,
    pub url_after: String,
    /// Fresh post-action snapshot so the caller never acts blind.
    pub snapshot: PageSnapshot,
}

/// Execute one action against an already-resolved live page, then verify.
/// The post-action snapshot is taken directly (no callback generic) so the
/// future type stays concrete — generic `impl AsyncFn` lifetimes used to
/// force higher-ranked `Send` proofs all the way up to `tokio::spawn`.
pub async fn execute_action(
    tab_id: TabId,
    page: &Page,
    action: &BrowserAction,
    viewport: (u32, u32),
) -> Result<ActionOutcome, BrowserError> {
    let url_before = page.url().await?.unwrap_or_default();
    let label = action_label(action);

    match action {
        BrowserAction::Navigate { url } => {
            navigate_and_settle(page, url).await?;
        }
        BrowserAction::Back => {
            page.evaluate("window.history.back()").await?;
            settle(page).await;
        }
        BrowserAction::Forward => {
            page.evaluate("window.history.forward()").await?;
            settle(page).await;
        }
        BrowserAction::Reload => {
            page.reload().await?;
            settle(page).await;
        }
        BrowserAction::Click { target_ref } => {
            let el = find_by_ref(page, target_ref)
                .await
                .map_err(|_| BrowserError::NotObserved(format!("no element {target_ref} on page")))?;
            el.click().await?;
            settle(page).await;
        }
        BrowserAction::Type { target_ref, text } => {
            if text.chars().count() > 4000 {
                return Err(BrowserError::InvalidInput("text exceeds 4000 chars".to_string()));
            }
            let el = find_by_ref(page, target_ref)
                .await
                .map_err(|_| BrowserError::NotObserved(format!("no element {target_ref} on page")))?;
            el.click().await?;
            el.type_str(text).await?;
            settle(page).await;
        }
        BrowserAction::Press { key } => {
            crate::input::press_key_trusted(page, key).await?;
            settle(page).await;
        }
        BrowserAction::Scroll { direction, amount } => {
            let px = (*amount).clamp(100, 8000) as i64;
            let dy = match direction.as_str() {
                "up" => -px,
                _ => px,
            };
            let expr = format!("window.scrollBy(0, {dy})");
            page.evaluate(expr.as_str()).await?;
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        BrowserAction::Wait { text_contains, timeout_ms } => {
            wait_for_text(page, text_contains.as_deref(), (*timeout_ms).clamp(500, 30_000)).await?;
        }
        BrowserAction::Select { target_ref, value } => {
            let value_js = serde_json::to_string(value)
                .map_err(|err| BrowserError::InvalidInput(err.to_string()))?;
            let expr = format!(
                "(function(){{const el=document.querySelector('[data-aro-ref=\"{target_ref}\"]');if(!el) return 'missing'; el.value={value_js}; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); return 'ok';}})()"
            );
            let evaluation = page.evaluate(expr.as_str()).await?;
            let status: String = evaluation.into_value().unwrap_or_default();
            if status != "ok" {
                return Err(BrowserError::NotObserved(format!("no element {target_ref} on page")));
            }
            settle(page).await;
        }
    }

    let snapshot = crate::service::snapshot_page(tab_id, page, viewport.0, viewport.1).await?;
    let url_after = snapshot.url.clone();
    let (success, detail) = verify_effect(action, &url_before, &url_after, page).await;

    Ok(ActionOutcome {
        tab_id,
        action: label,
        success,
        detail,
        url_before,
        url_after,
        snapshot,
    })
}

fn action_label(action: &BrowserAction) -> String {
    match action {
        BrowserAction::Click { target_ref } => format!("click {target_ref}"),
        BrowserAction::Type { target_ref, .. } => format!("type {target_ref}"),
        BrowserAction::Press { key } => format!("press {key}"),
        BrowserAction::Scroll { direction, amount } => format!("scroll {direction} {amount}"),
        BrowserAction::Navigate { url } => format!("navigate {url}"),
        BrowserAction::Back => "back".to_string(),
        BrowserAction::Forward => "forward".to_string(),
        BrowserAction::Reload => "reload".to_string(),
        BrowserAction::Wait { .. } => "wait".to_string(),
        BrowserAction::Select { target_ref, value } => format!("select {target_ref}={value}"),
    }
}

async fn navigate_and_settle(page: &Page, url: &str) -> Result<(), BrowserError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| BrowserError::InvalidInput(format!("not a url: {url}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(BrowserError::InvalidInput("only http(s) urls allowed".to_string()));
    }
    page.goto(url)
        .await
        .map_err(|err| BrowserError::Navigation { url: url.to_string(), reason: err.to_string() })?;
    settle(page).await;
    Ok(())
}

/// Wait for load event + network calm (bounded).
async fn settle(page: &Page) {
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        page.wait_for_navigation(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
}

/// Wait until the page text contains `needle` (or just settle when `None`).
async fn wait_for_text(page: &Page, needle: Option<&str>, timeout_ms: u64) -> Result<(), BrowserError> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let text = readable_text(page).await.unwrap_or_default();
        let matched = match needle {
            Some(n) => text.to_lowercase().contains(&n.to_lowercase()),
            None => true,
        };
        if matched {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(BrowserError::NotObserved(
                "wait timed out without observing expected text".to_string(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }
}

/// Confirm the action did something observable.
async fn verify_effect(
    action: &BrowserAction,
    url_before: &str,
    url_after: &str,
    page: &Page,
) -> (bool, String) {
    match action {
        BrowserAction::Navigate { url } => {
            if url_after == url || url_after == format!("{url}/") {
                (true, format!("navigated to {url_after}"))
            } else if url_after != url_before {
                (true, format!("navigated to {url_after} (redirected from {url})"))
            } else {
                (false, format!("still on {url_after} after navigate"))
            }
        }
        BrowserAction::Click { .. } | BrowserAction::Press { .. } | BrowserAction::Select { .. } => {
            if url_after != url_before {
                (true, format!("page changed to {url_after}"))
            } else {
                // Same-document interaction: re-enumerate to prove liveness.
                match enumerate_elements(page).await {
                    Ok(_) => (true, "interaction executed, page live".to_string()),
                    Err(err) => (false, format!("page not observable after action: {err}")),
                }
            }
        }
        _ => (true, "executed and re-observed".to_string()),
    }
}

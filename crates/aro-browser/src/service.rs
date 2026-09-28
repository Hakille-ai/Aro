//! `BrowserService`: owns one Chromium process and a pool of tabs.
//!
//! All browser I/O is serialized through one mutex so CDP sessions never
//! interleave. Tabs are real CDP targets sharing the dedicated ARO profile.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures_util::StreamExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::act::{execute_action, ActionOutcome, BrowserError};
use crate::config::{discover_executable, BrowserLaunchConfig};
use crate::model::{AutofillRequest, BrowserAction, BrowserTabInfo, PageSnapshot, TabId};
use crate::observe::{enumerate_elements, readable_text};
use crate::vault::fill_login_form;

struct TabState {
    info: BrowserTabInfo,
    page: Page,
}

struct Inner {
    browser: Browser,
    tabs: HashMap<TabId, TabState>,
    action_budget: usize,
}

#[derive(Clone)]
pub struct BrowserService {
    inner: Arc<Mutex<Inner>>,
    viewport_width: u32,
    viewport_height: u32,
}

impl BrowserService {
    /// Launch Chromium (system channel or `ARO_CHROME_EXECUTABLE`) and take
    /// ownership of the process. The handler pump is spawned internally.
    pub async fn launch(config: &BrowserLaunchConfig) -> Result<Self, BrowserError> {
        let executable = match &config.executable {
            Some(path) => path.clone(),
            None => discover_executable().ok_or(BrowserError::NoExecutable)?,
        };
        info!(executable = %executable.display(), "launching ARO browser");

        std::fs::create_dir_all(&config.profile_dir)?;
        let mut builder = BrowserConfig::builder()
            .chrome_executable(executable)
            .user_data_dir(config.profile_dir.clone())
            .window_size(config.window_width, config.window_height)
            .args(vec![
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-networking",
                "--disable-sync",
                "--disable-translate",
                "--mute-audio",
                "--hide-scrollbars",
            ]);
        if !config.headless {
            builder = builder.with_head();
        }
        if let Some(agent) = &config.user_agent {
            builder = builder.arg(format!("--user-agent={agent}"));
        }
        for arg in &config.extra_args {
            builder = builder.arg(arg.clone());
        }
        let launch_config = builder
            .build()
            .map_err(|err| BrowserError::Launch(err.to_string()))?;

        let (browser, mut handler) = Browser::launch(launch_config)
            .await
            .map_err(|err| BrowserError::Launch(err.to_string()))?;

        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    warn!("browser handler event error");
                    break;
                }
            }
        });

        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                browser,
                tabs: HashMap::new(),
                action_budget: 200,
            })),
            viewport_width: config.window_width,
            viewport_height: config.window_height,
        })
    }

    /// Open a new tab (real CDP target), optionally navigating to `url`.
    pub async fn open_tab(&self, url: Option<&str>, ai_controlled: bool) -> Result<BrowserTabInfo, BrowserError> {
        let mut inner = self.inner.lock().await;
        let page = inner.browser.new_page("about:blank").await?;
        let id = TabId::new();
        let mut info = BrowserTabInfo {
            id,
            url: "about:blank".to_string(),
            title: "New tab".to_string(),
            loading: false,
            ai_controlled,
            ai_status: None,
            created_at: Utc::now(),
            last_snapshot_at: None,
        };
        if let Some(target) = url {
            info.loading = true;
            inner.tabs.insert(id, TabState { info: info.clone(), page: page.clone() });
            drop(inner);
            let outcome = self.navigate(id, target).await?;
            let mut inner = self.inner.lock().await;
            if let Some(state) = inner.tabs.get_mut(&id) {
                state.info.url = outcome.url_after.clone();
                state.info.title = outcome.snapshot.title.clone();
                state.info.loading = false;
                return Ok(state.info.clone());
            }
            return Err(BrowserError::NoSuchTab(id));
        }
        inner.tabs.insert(id, TabState { info: info.clone(), page });
        Ok(info)
    }

    pub async fn close_tab(&self, id: TabId) -> Result<(), BrowserError> {
        let mut inner = self.inner.lock().await;
        let state = inner.tabs.remove(&id).ok_or(BrowserError::NoSuchTab(id))?;
        state.page.close().await?;
        Ok(())
    }

    pub async fn list_tabs(&self) -> Vec<BrowserTabInfo> {
        let inner = self.inner.lock().await;
        inner.tabs.values().map(|s| s.info.clone()).collect()
    }

    pub async fn set_ai_control(&self, id: TabId, controlled: bool, status: Option<String>) -> Result<(), BrowserError> {
        let mut inner = self.inner.lock().await;
        let state = inner.tabs.get_mut(&id).ok_or(BrowserError::NoSuchTab(id))?;
        state.info.ai_controlled = controlled;
        state.info.ai_status = status;
        Ok(())
    }

    /// Navigate a tab and wait for settle. Returns the verified outcome.
    pub async fn navigate(&self, id: TabId, url: &str) -> Result<ActionOutcome, BrowserError> {
        self.run_action(id, &BrowserAction::Navigate { url: url.to_string() }).await
    }

    /// Full observation: live URL + title + screenshot + refs + text.
    pub async fn snapshot(&self, id: TabId) -> Result<PageSnapshot, BrowserError> {
        let inner = self.inner.lock().await;
        let state = inner.tabs.get(&id).ok_or(BrowserError::NoSuchTab(id))?;
        let snapshot = snapshot_page(id, &state.page, self.viewport_width, self.viewport_height).await?;
        drop(inner);
        let mut inner = self.inner.lock().await;
        if let Some(state) = inner.tabs.get_mut(&id) {
            state.info.url = snapshot.url.clone();
            state.info.title = snapshot.title.clone();
            state.info.last_snapshot_at = Some(snapshot.captured_at);
        }
        Ok(snapshot)
    }

    /// Lightweight frame for the user-facing screencast: JPEG q70 at
    /// viewport size (adaptive fps handled by the caller). Agent snapshots
    /// keep full PNG fidelity for model vision.
    pub async fn frame_jpeg(&self, id: TabId) -> Result<Vec<u8>, BrowserError> {
        let inner = self.inner.lock().await;
        let state = inner.tabs.get(&id).ok_or(BrowserError::NoSuchTab(id))?;
        screenshot_jpeg(&state.page, 70).await
    }

    /// Back-compat PNG frame (tests, thumbnails).
    pub async fn frame_png(&self, id: TabId) -> Result<Vec<u8>, BrowserError> {
        let inner = self.inner.lock().await;
        let state = inner.tabs.get(&id).ok_or(BrowserError::NoSuchTab(id))?;
        screenshot_png(&state.page).await
    }

    /// Verified action with pre/post observation (budget-guarded).
    pub async fn run_action(&self, id: TabId, action: &BrowserAction) -> Result<ActionOutcome, BrowserError> {
        {
            let mut inner = self.inner.lock().await;
            if inner.action_budget == 0 {
                return Err(BrowserError::NotObserved("browser action budget exhausted".to_string()));
            }
            inner.action_budget -= 1;
        }
        let inner = self.inner.lock().await;
        let state = inner.tabs.get(&id).ok_or(BrowserError::NoSuchTab(id))?;
        let page = state.page.clone();
        let (width, height) = (self.viewport_width, self.viewport_height);
        drop(inner);
        let outcome = execute_action(id, &page, action, (width, height)).await?;
        let mut inner = self.inner.lock().await;
        if let Some(state) = inner.tabs.get_mut(&id) {
            state.info.url = outcome.url_after.clone();
            state.info.title = outcome.snapshot.title.clone();
            state.info.last_snapshot_at = Some(outcome.snapshot.captured_at);
        }
        Ok(outcome)
    }

    /// Server-side readable text (no JS rendering limits beyond Chromium).
    pub async fn extract_text(&self, id: TabId) -> Result<String, BrowserError> {
        let inner = self.inner.lock().await;
        let state = inner.tabs.get(&id).ok_or(BrowserError::NoSuchTab(id))?;
        Ok(readable_text(&state.page).await?)
    }

    /// User relay: click at viewport CSS coordinates, then re-observe.
    pub async fn click_point(&self, id: TabId, x: f64, y: f64) -> Result<PageSnapshot, BrowserError> {
        let page = self.page_for(id).await?;
        crate::input::click_point(&page, x, y).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        self.snapshot(id).await
    }

    /// User relay: type into the focused element, then re-observe.
    pub async fn type_focused(&self, id: TabId, text: &str) -> Result<PageSnapshot, BrowserError> {
        let page = self.page_for(id).await?;
        crate::input::type_into_focused(&page, text).await?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        self.snapshot(id).await
    }

    /// User relay: trusted key press, then re-observe.
    pub async fn press(&self, id: TabId, key: &str) -> Result<PageSnapshot, BrowserError> {
        let page = self.page_for(id).await?;
        crate::input::press_key_trusted(&page, key).await?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        self.snapshot(id).await
    }

    /// User relay: wheel scroll, then re-observe.
    pub async fn wheel(&self, id: TabId, delta_x: f64, delta_y: f64) -> Result<PageSnapshot, BrowserError> {
        let page = self.page_for(id).await?;
        crate::input::wheel(&page, delta_x, delta_y).await?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        self.snapshot(id).await
    }

    /// User relay: click a snapshot ref (same refs the agent sees).
    pub async fn click_ref(&self, id: TabId, target_ref: &str) -> Result<PageSnapshot, BrowserError> {
        self.run_action(id, &BrowserAction::Click { target_ref: target_ref.to_string() })
            .await
            .map(|outcome| outcome.snapshot)
    }

    async fn page_for(&self, id: TabId) -> Result<Page, BrowserError> {
        let inner = self.inner.lock().await;
        inner
            .tabs
            .get(&id)
            .map(|state| state.page.clone())
            .ok_or(BrowserError::NoSuchTab(id))
    }

    /// Fill a login form. Credentials stay inside this call: only the
    /// outcome (never the values) leaves the function.
    pub async fn autofill(&self, request: &AutofillRequest) -> Result<ActionOutcome, BrowserError> {
        let inner = self.inner.lock().await;
        let state = inner.tabs.get(&request.tab_id).ok_or(BrowserError::NoSuchTab(request.tab_id))?;
        let page = state.page.clone();
        let tab_id = request.tab_id;
        let (width, height) = (self.viewport_width, self.viewport_height);
        drop(inner);
        fill_login_form(
            &page,
            &request.username_ref,
            &request.password_ref,
            request.username.as_str(),
            request.password.as_str(),
        )
        .await?;
        let snapshot = snapshot_page(tab_id, &page, width, height).await?;
        Ok(ActionOutcome {
            tab_id,
            action: "autofill".to_string(),
            success: true,
            detail: format!("credentials filled on {}", snapshot.url),
            url_before: snapshot.url.clone(),
            url_after: snapshot.url.clone(),
            snapshot,
        })
    }

    /// Remaining verified-action budget (resets on relaunch; Phase 2 adds
    /// per-run budgets in aro-runtime).
    pub async fn action_budget(&self) -> usize {
        self.inner.lock().await.action_budget
    }
}

/// Capture + enumerate + read one live page.
pub async fn snapshot_page(
    tab_id: TabId,
    page: &Page,
    width: u32,
    height: u32,
) -> Result<PageSnapshot, BrowserError> {
    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();
    let png = screenshot_png(page).await?;
    let elements = enumerate_elements(page).await.unwrap_or_default();
    let text = readable_text(page).await.unwrap_or_default();
    Ok(PageSnapshot {
        tab_id,
        url,
        title,
        screenshot_base64: base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &png,
        ),
        screenshot_width: width,
        screenshot_height: height,
        elements,
        text_excerpt: text.chars().take(6000).collect(),
        captured_at: Utc::now(),
    })
}

async fn screenshot_png(page: &Page) -> Result<Vec<u8>, BrowserError> {
    use chromiumoxide::page::ScreenshotParams;
    let params = ScreenshotParams::builder().full_page(false).build();
    page.screenshot(params).await.map_err(BrowserError::from)
}

async fn screenshot_jpeg(page: &Page, quality: i64) -> Result<Vec<u8>, BrowserError> {
    use chromiumoxide::page::ScreenshotParams;
    use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
    let params = ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Jpeg)
        .quality(quality)
        .full_page(false)
        .build();
    page.screenshot(params).await.map_err(BrowserError::from)
}

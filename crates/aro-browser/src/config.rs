//! Launch configuration + executable discovery (system channels first,
//! sidecar later in Phase 5) + persistent profile directory.

use std::path::PathBuf;

/// How the browser process should be started.
#[derive(Debug, Clone)]
pub struct BrowserLaunchConfig {
    /// Explicit executable path. `None` = auto-discovery.
    pub executable: Option<PathBuf>,
    /// Persistent profile dir (cookies, storage). Dedicated ARO profile,
    /// never the user's own Chrome profile.
    pub profile_dir: PathBuf,
    pub headless: bool,
    pub window_width: u32,
    pub window_height: u32,
    pub user_agent: Option<String>,
    pub locale: String,
    /// Extra Chromium flags (proxy, etc.).
    pub extra_args: Vec<String>,
}

impl BrowserLaunchConfig {
    pub fn new(profile_dir: PathBuf) -> Self {
        Self {
            executable: None,
            profile_dir,
            headless: true,
            window_width: 1280,
            window_height: 800,
            user_agent: None,
            locale: "fr-FR".to_string(),
            extra_args: Vec::new(),
        }
    }
}

/// Default persistent profile directory for the ARO browser.
pub fn default_profile_dir() -> PathBuf {
    directories::ProjectDirs::from("local", "aro", "ARO")
        .map(|dirs| dirs.data_local_dir().join("browser-profile"))
        .unwrap_or_else(|| std::env::temp_dir().join("aro-browser-profile"))
}

/// Locate a usable Chromium-family executable.
///
/// Order: `ARO_CHROME_EXECUTABLE` env override, then well-known install
/// paths per OS (Chrome, Edge, Chromium). Returns `None` with no side
/// effects when nothing is found — the caller turns this into an
/// actionable error (Phase 5 adds sidecar download here).
pub fn discover_executable() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("ARO_CHROME_EXECUTABLE") {
        let path = PathBuf::from(custom.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    // Tauri sidecar (Phase 5 packaging): `aro-chromium[.exe]` next to the
    // application binary. `current_exe` in dev points at target/debug, in
    // prod at the installed app dir — both are checked.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(target_os = "windows")]
            let sidecar = dir.join("aro-chromium.exe");
            #[cfg(not(target_os = "windows"))]
            let sidecar = dir.join("aro-chromium");
            if sidecar.is_file() {
                return Some(sidecar);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        for candidate in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            for candidate in [
                format!(r"{local_app_data}\Google\Chrome\Application\chrome.exe"),
                format!(r"{local_app_data}\Microsoft\Edge\Application\msedge.exe"),
                format!(r"{local_app_data}\Chromium\Application\chrome.exe"),
            ] {
                let path = PathBuf::from(candidate);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for candidate in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for bin in ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge"] {
            if let Ok(path) = which::which(bin) {
                return Some(path);
            }
        }
    }
    None
}

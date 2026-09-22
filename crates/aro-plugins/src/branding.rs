//! Custom plugin branding (logos).
//!
//! The official `plugin.json` schema (agent-plugins.org §5) is closed and
//! carries NO logo/icon field. ARO stores custom branding in the portable,
//! spec-compliant client extension namespace:
//!
//! ```json
//! "extensions": {
//!   "com.aro.client": { "logo": "🚀", "brandColor": "#0A84FF" }
//! }
//! ```
//!
//! - `logo`: a short emoji string (inline, portable).
//! - `logoFile`: a file confined to the plugin root (`logo.png|jpg|jpeg|webp|svg`).
//! - `brandColor`: a `#rgb` / `#rrggbb` CSS color.
//!
//! Image uploads travel as `data:image/...;base64,...` URLs inside
//! `CreateCustomPluginRequest` so Tauri IPC and the JSON REST API share one
//! transport. Files are sniffed by magic bytes (never by extension alone),
//! capped in size, and confined to the plugin root on read.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Reverse-domain namespace owning ARO branding inside `extensions`.
pub const ARO_BRANDING_EXTENSION: &str = "com.aro.client";

/// Fixed on-disk file name for uploaded logos (no user-controlled path).
pub const LOGO_FILE_STEM: &str = "logo";

/// Hard cap for uploaded / served logos (2 MiB: large enough for HD
/// artwork, small enough to keep plugin dirs and IPC payloads sane).
pub const MAX_LOGO_BYTES: usize = 2 * 1024 * 1024;

/// How a plugin logo is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoKind {
    Emoji,
    File,
}

#[derive(Debug, Clone)]
pub struct ResolvedBranding {
    /// Emoji glyph, or `file:<name>` reference for uploaded images.
    pub logo: Option<String>,
    pub logo_kind: Option<LogoKind>,
    pub brand_color: Option<String>,
}

impl ResolvedBranding {
    pub fn empty() -> Self {
        Self {
            logo: None,
            logo_kind: None,
            brand_color: None,
        }
    }
}

/// Read branding from a manifest `extensions` map (never fails: invalid
/// values are ignored so a bad extension can never break plugin loading).
pub fn branding_from_extensions(
    extensions: &HashMap<String, serde_json::Value>,
) -> ResolvedBranding {
    let mut out = ResolvedBranding::empty();
    let obj = extensions
        .get(ARO_BRANDING_EXTENSION)
        .and_then(|v| v.as_object());
    let Some(obj) = obj else {
        return out;
    };
    if let Some(color) = obj.get("brandColor").and_then(|v| v.as_str()) {
        if is_valid_brand_color(color) {
            out.brand_color = Some(color.to_string());
        }
    }
    if let Some(file) = obj.get("logoFile").and_then(|v| v.as_str()) {
        if is_allowed_logo_file_name(file) {
            out.logo = Some(format!("file:{file}"));
            out.logo_kind = Some(LogoKind::File);
            return out;
        }
    }
    if let Some(emoji) = obj.get("logo").and_then(|v| v.as_str()) {
        if is_valid_logo_emoji(emoji) {
            out.logo = Some(emoji.to_string());
            out.logo_kind = Some(LogoKind::Emoji);
        }
    }
    out
}

/// Build the `com.aro.client` extension value for a new custom plugin.
pub fn build_branding_extension(
    logo_emoji: Option<&str>,
    logo_file: Option<&str>,
    brand_color: Option<&str>,
) -> Result<Option<serde_json::Value>> {
    let mut map = serde_json::Map::new();
    if let Some(emoji) = logo_emoji.map(str::trim).filter(|s| !s.is_empty()) {
        if !is_valid_logo_emoji(emoji) {
            return Err(anyhow!("Invalid logo emoji"));
        }
        map.insert("logo".to_string(), serde_json::Value::String(emoji.to_string()));
    }
    if let Some(file) = logo_file {
        if !is_allowed_logo_file_name(file) {
            return Err(anyhow!("Invalid logo file name"));
        }
        map.insert(
            "logoFile".to_string(),
            serde_json::Value::String(file.to_string()),
        );
    }
    if let Some(color) = brand_color.map(str::trim).filter(|s| !s.is_empty()) {
        if !is_valid_brand_color(color) {
            return Err(anyhow!("Invalid brand color (expected #rgb or #rrggbb)"));
        }
        map.insert(
            "brandColor".to_string(),
            serde_json::Value::String(color.to_string()),
        );
    }
    if map.is_empty() {
        Ok(None)
    } else {
        Ok(Some(serde_json::Value::Object(map)))
    }
}

/// An emoji logo: 1-2 graphemes, no control characters, bounded length.
pub fn is_valid_logo_emoji(s: &str) -> bool {
    if s.is_empty() || s.len() > 16 {
        return false;
    }
    if s.chars().count() > 4 {
        return false;
    }
    !s.chars().any(|c| c.is_control())
}

pub fn is_valid_brand_color(s: &str) -> bool {
    let hex = s.strip_prefix('#').unwrap_or("");
    (hex.len() == 3 || hex.len() == 6) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Only the fixed `logo.<ext>` names are servable (no traversal possible).
pub fn is_allowed_logo_file_name(name: &str) -> bool {
    matches!(
        name,
        "logo.png" | "logo.jpg" | "logo.jpeg" | "logo.webp" | "logo.svg"
    )
}

#[derive(Debug, Clone)]
pub struct DecodedLogo {
    pub mime: String,
    pub extension: String,
    pub bytes: Vec<u8>,
}

/// Validate + decode a `data:image/...;base64,...` upload.
/// Sniffs magic bytes (PNG/JPEG/WebP) or structural SVG checks; the file
/// extension declared in the data URL is never trusted.
pub fn parse_logo_data_url(data_url: &str) -> Result<DecodedLogo> {
    let (header, b64) = data_url
        .split_once(',')
        .ok_or_else(|| anyhow!("Malformed logo data URL"))?;
    let header = header.trim();
    let mime = header
        .strip_prefix("data:")
        .and_then(|h| h.split(';').next())
        .ok_or_else(|| anyhow!("Malformed logo data URL"))?
        .to_lowercase();
    if !header.contains(";base64") {
        return Err(anyhow!("Logo must be base64-encoded"));
    }
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
        .map_err(|_| anyhow!("Logo is not valid base64"))?;
    if bytes.is_empty() || bytes.len() > MAX_LOGO_BYTES {
        return Err(anyhow!("Logo must be 1 byte..2 MiB"));
    }
    match mime.as_str() {
        "image/png" => {
            if !is_png(&bytes) {
                return Err(anyhow!("Upload is not a real PNG file"));
            }
            Ok(DecodedLogo {
                mime,
                extension: "png".to_string(),
                bytes,
            })
        }
        "image/jpeg" => {
            if !is_jpeg(&bytes) {
                return Err(anyhow!("Upload is not a real JPEG file"));
            }
            Ok(DecodedLogo {
                mime,
                extension: "jpg".to_string(),
                bytes,
            })
        }
        "image/webp" => {
            if !is_webp(&bytes) {
                return Err(anyhow!("Upload is not a real WebP file"));
            }
            Ok(DecodedLogo {
                mime,
                extension: "webp".to_string(),
                bytes,
            })
        }
        "image/svg+xml" => {
            validate_svg_logo(&bytes)?;
            Ok(DecodedLogo {
                mime,
                extension: "svg".to_string(),
                bytes,
            })
        }
        _ => Err(anyhow!(
            "Unsupported logo format (PNG, JPEG, WebP or SVG only)"
        )),
    }
}

fn is_png(b: &[u8]) -> bool {
    b.len() > 8 && b[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
}

fn is_jpeg(b: &[u8]) -> bool {
    b.len() > 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF
}

fn is_webp(b: &[u8]) -> bool {
    b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

/// Structural SVG gate: must look like an SVG document and must not carry
/// active content (scripts, event handlers, external references). Served
/// with `Content-Security-Policy: sandbox` semantics in mind; defense in
/// depth for a local-first app.
fn validate_svg_logo(bytes: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| anyhow!("SVG logo must be UTF-8 text"))?;
    if text.len() > MAX_LOGO_BYTES {
        return Err(anyhow!("Logo must be 1 byte..2 MiB"));
    }
    let lower = text.to_lowercase();
    if !lower.contains("<svg") {
        return Err(anyhow!("Upload is not an SVG document"));
    }
    for forbidden in [
        "<script",
        "javascript:",
        "data:text/html",
        "<!entity",
        "<!doctype",
    ] {
        if lower.contains(forbidden) {
            return Err(anyhow!("SVG logo contains forbidden content"));
        }
    }
    // Event-handler attributes (`onload=`, `onerror=`, ...) and XLink
    // script hooks are rejected with a small scanner (no regex dep).
    let chars: Vec<char> = lower.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == 'o' && chars.get(i + 1) == Some(&'n') {
            let mut j = i + 2;
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '-') {
                j += 1;
            }
            let mut k = j;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            if k < chars.len() && chars[k] == '=' {
                // `on*=` outside of a longer identifier (e.g. `icon=`) is an
                // event handler; `icon` itself is safe and must not match.
                let prev = if i > 0 { chars[i - 1] } else { '<' };
                if !prev.is_ascii_alphanumeric() && prev != '-' && prev != '_' {
                    return Err(anyhow!("SVG logo must not contain event handlers"));
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Confine a logo file name to the plugin root and return its full path.
/// The name must be one of the fixed `logo.<ext>` values, so traversal is
/// structurally impossible; the `starts_with` check is defense in depth
/// (mirrors `contained_in_root` in aro-tools).
pub fn confine_logo_path(plugin_root: &Path, file_name: &str) -> Result<PathBuf> {
    if !is_allowed_logo_file_name(file_name) {
        return Err(anyhow!("Invalid logo file name"));
    }
    let candidate = plugin_root.join(file_name);
    let root = normalize(plugin_root).ok_or_else(|| anyhow!("Invalid plugin root"))?;
    let cand = normalize(&candidate).ok_or_else(|| anyhow!("Invalid logo path"))?;
    if !cand.starts_with(&root) {
        return Err(anyhow!("Logo path escapes the plugin directory"));
    }
    Ok(candidate)
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            c => out.push(c.as_os_str()),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_validation_bounds_glyphs() {
        assert!(is_valid_logo_emoji("🚀"));
        assert!(is_valid_logo_emoji("📑"));
        assert!(!is_valid_logo_emoji(""));
        assert!(!is_valid_logo_emoji("hello world, this is long"));
        assert!(!is_valid_logo_emoji("ab\0cd"));
    }

    #[test]
    fn brand_color_accepts_hex_only() {
        assert!(is_valid_brand_color("#0A84FF"));
        assert!(is_valid_brand_color("#fff"));
        assert!(!is_valid_brand_color("red"));
        assert!(!is_valid_brand_color("#12345"));
        assert!(!is_valid_brand_color("javascript:alert(1)"));
    }

    #[test]
    fn rejects_unknown_logo_formats() {
        let err = parse_logo_data_url("data:image/gif;base64,R0lGODdhAQABAIAAAP8AAAAA").unwrap_err();
        assert!(err.to_string().contains("Unsupported"));
    }

    #[test]
    fn rejects_mismatched_magic_bytes() {
        // Declared PNG but plain-text payload.
        let fake = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            b"hello, not a png",
        );
        let err = parse_logo_data_url(&format!("data:image/png;base64,{fake}")).unwrap_err();
        assert!(err.to_string().contains("not a real PNG"));
    }

    #[test]
    fn accepts_minimal_png_and_svg() {
        // 1x1 transparent PNG.
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let decoded =
            parse_logo_data_url(&format!("data:image/png;base64,{png_b64}")).unwrap();
        assert_eq!(decoded.extension, "png");
        assert_eq!(decoded.mime, "image/png");

        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#0A84FF"/></svg>"##;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, svg);
        let decoded =
            parse_logo_data_url(&format!("data:image/svg+xml;base64,{b64}")).unwrap();
        assert_eq!(decoded.extension, "svg");
    }

    #[test]
    fn rejects_active_svg_content() {
        for payload in [
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><circle r="5"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><a href="javascript:alert(1)">x</a></svg>"#,
        ] {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, payload);
            let err =
                parse_logo_data_url(&format!("data:image/svg+xml;base64,{b64}")).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("forbidden") || msg.contains("event handlers"),
                "unexpected: {msg}"
            );
        }
    }

    #[test]
    fn confines_logo_paths() {
        let root = Path::new("/tmp/plugins/my-plugin");
        assert!(confine_logo_path(root, "logo.png").is_ok());
        assert!(confine_logo_path(root, "../../evil.png").is_err());
        assert!(confine_logo_path(root, "other.png").is_err());
    }

    #[test]
    fn branding_extension_roundtrip() {
        let ext = build_branding_extension(Some("🚀"), None, Some("#0A84FF")).unwrap().unwrap();
        let mut map = HashMap::new();
        map.insert(ARO_BRANDING_EXTENSION.to_string(), ext);
        let resolved = branding_from_extensions(&map);
        assert_eq!(resolved.logo.as_deref(), Some("🚀"));
        assert_eq!(resolved.logo_kind, Some(LogoKind::Emoji));
        assert_eq!(resolved.brand_color.as_deref(), Some("#0A84FF"));
    }

    #[test]
    fn invalid_branding_values_are_ignored_not_fatal() {
        let mut inner = serde_json::Map::new();
        inner.insert("logo".to_string(), serde_json::Value::String("x".repeat(100)));
        inner.insert(
            "brandColor".to_string(),
            serde_json::Value::String("not-a-color".to_string()),
        );
        let mut map = HashMap::new();
        map.insert(
            ARO_BRANDING_EXTENSION.to_string(),
            serde_json::Value::Object(inner),
        );
        let resolved = branding_from_extensions(&map);
        assert_eq!(resolved.logo, None);
        assert_eq!(resolved.brand_color, None);
    }
}

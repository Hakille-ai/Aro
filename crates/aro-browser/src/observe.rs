//! Observation primitives: referenced element tree, readable text,
//! screenshots — all read from the live CDP page, never invented.

use chromiumoxide::Page;
use serde::Deserialize;

use crate::model::ElementRef;

#[derive(Debug, Deserialize)]
struct RawElement {
    id: String,
    role: String,
    name: String,
    #[serde(default)]
    input_type: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    in_viewport: bool,
}

/// JavaScript that enumerates visible interactive elements and stamps them
/// with a stable-per-snapshot `data-aro-ref` id (`r1`, `r2`, …).
pub const REF_ENUMERATION_JS: &str = r#"(function () {
  const out = [];
  let counter = 0;
  const selector = 'a[href], button, input, select, textarea, [role="button"], [role="link"], [role="textbox"], [role="checkbox"], [role="radio"], [role="switch"], [role="tab"], [onclick]';
  const nodes = document.querySelectorAll(selector);
  for (const el of nodes) {
    if (counter >= 120) break;
    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    if (rect.width === 0 && rect.height === 0) continue;
    if (style.visibility === 'hidden' || style.display === 'none') continue;
    counter += 1;
    const refId = 'r' + counter;
    el.setAttribute('data-aro-ref', refId);
    const tag = el.tagName.toLowerCase();
    const role = el.getAttribute('role') || tag;
    const name = (el.getAttribute('aria-label') || el.innerText || el.value || el.getAttribute('placeholder') || el.getAttribute('title') || el.getAttribute('href') || '').trim().slice(0, 120).replace(/\s+/g, ' ');
    const inViewport = rect.top >= 0 && rect.left >= 0 && rect.bottom <= window.innerHeight && rect.right <= window.innerWidth;
    out.push({
      id: refId,
      role,
      name,
      input_type: tag === 'input' ? (el.getAttribute('type') || 'text') : null,
      value: (tag === 'input' || tag === 'textarea' || tag === 'select') ? String(el.value || '').slice(0, 200) : null,
      in_viewport: inViewport,
    });
  }
  return out;
})()"#;

/// Readable text of the page (article/main preferred, scripts removed).
pub const READABLE_TEXT_JS: &str = r#"(function () {
  const root = document.querySelector('main, article, [role="main"]') || document.body;
  if (!root) return '';
  const clone = root.cloneNode(true);
  clone.querySelectorAll('script, style, noscript, svg, nav, footer').forEach((n) => n.remove());
  return (clone.innerText || '').replace(/[ \t]+/g, ' ').replace(/\n{3,}/g, '\n\n').trim().slice(0, 24000);
})()"#;

/// Enumerate referenced interactive elements of the live page.
pub async fn enumerate_elements(page: &Page) -> Result<Vec<ElementRef>, chromiumoxide::error::CdpError> {
    let evaluation = page.evaluate(REF_ENUMERATION_JS).await?;
    let raw: Vec<RawElement> = evaluation.into_value()?;
    Ok(raw
        .into_iter()
        .map(|r| ElementRef {
            id: r.id,
            role: r.role,
            name: r.name,
            input_type: r.input_type,
            value: r.value,
            in_viewport: r.in_viewport,
        })
        .collect())
}

/// Extract readable text of the live page.
pub async fn readable_text(page: &Page) -> Result<String, chromiumoxide::error::CdpError> {
    let evaluation = page.evaluate(READABLE_TEXT_JS).await?;
    Ok(evaluation.into_value::<String>().unwrap_or_default())
}

/// Resolve a `ref` id to a live element handle (re-query by attribute so
/// the handle is always fresh, even if the DOM shifted).
pub async fn find_by_ref(page: &Page, target_ref: &str) -> Result<chromiumoxide::Element, chromiumoxide::error::CdpError> {
    page.find_element(format!("[data-aro-ref=\"{target_ref}\"]")).await
}

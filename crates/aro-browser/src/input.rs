//! User-driven input relay: clicks at screencast coordinates, typing into
//! the focused element, trusted key presses, wheel scrolls.
//!
//! The frontend renders live frames and forwards pointer/keyboard events
//! here with viewport-relative CSS coordinates. All effects are real CDP
//! input — the same channel the agent uses, so both pilots share one
//! source of truth.

use chromiumoxide::Page;

use crate::act::BrowserError;

/// Click at viewport CSS coordinates (left button: press + release).
pub async fn click_point(page: &Page, x: f64, y: f64) -> Result<(), BrowserError> {
    use chromiumoxide::cdp::browser_protocol::input::{
        DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
    };
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 || x > 8000.0 || y > 8000.0 {
        return Err(BrowserError::InvalidInput(format!("bad click point ({x}, {y})")));
    }
    let moved = DispatchMouseEventParams {
        r#type: DispatchMouseEventType::MouseMoved,
        x,
        y,
        modifiers: None,
        timestamp: None,
        button: Some(MouseButton::None),
        buttons: Some(0),
        click_count: Some(0),
        force: None,
        tangential_pressure: None,
        tilt_x: None,
        tilt_y: None,
        twist: None,
        delta_x: None,
        delta_y: None,
        pointer_type: None,
    };
    page.execute(moved).await?;
    let mut pressed = DispatchMouseEventParams {
        r#type: DispatchMouseEventType::MousePressed,
        x,
        y,
        modifiers: None,
        timestamp: None,
        button: Some(MouseButton::Left),
        buttons: Some(1),
        click_count: Some(1),
        force: None,
        tangential_pressure: None,
        tilt_x: None,
        tilt_y: None,
        twist: None,
        delta_x: None,
        delta_y: None,
        pointer_type: None,
    };
    page.execute(pressed.clone()).await?;
    pressed.r#type = DispatchMouseEventType::MouseReleased;
    pressed.buttons = Some(0);
    page.execute(pressed).await?;
    Ok(())
}

/// Wheel scroll at the current pointer position.
pub async fn wheel(page: &Page, delta_x: f64, delta_y: f64) -> Result<(), BrowserError> {
    use chromiumoxide::cdp::browser_protocol::input::{DispatchMouseEventParams, DispatchMouseEventType};
    if !delta_x.is_finite() || !delta_y.is_finite() {
        return Err(BrowserError::InvalidInput("bad wheel delta".to_string()));
    }
    let event = DispatchMouseEventParams {
        r#type: DispatchMouseEventType::MouseWheel,
        x: 0.0,
        y: 0.0,
        modifiers: None,
        timestamp: None,
        button: None,
        buttons: None,
        click_count: None,
        force: None,
        tangential_pressure: None,
        tilt_x: None,
        tilt_y: None,
        twist: None,
        delta_x: Some(delta_x.clamp(-4000.0, 4000.0)),
        delta_y: Some(delta_y.clamp(-4000.0, 4000.0)),
        pointer_type: None,
    };
    page.execute(event).await?;
    Ok(())
}

/// Type into whatever element currently has focus.
pub async fn type_into_focused(page: &Page, text: &str) -> Result<(), BrowserError> {
    if text.chars().count() > 4000 {
        return Err(BrowserError::InvalidInput("text exceeds 4000 chars".to_string()));
    }
    if text.is_empty() {
        return Ok(());
    }
    let focused = page
        .find_element("*:focus")
        .await
        .map_err(|_| BrowserError::NotObserved("nothing is focused; click a field first".to_string()))?;
    focused.type_str(text).await?;
    Ok(())
}

/// Trusted key press (shared by agent `press` and user keyboard relay).
pub async fn press_key_trusted(page: &Page, key: &str) -> Result<(), BrowserError> {
    use chromiumoxide::cdp::browser_protocol::input::{DispatchKeyEventParams, DispatchKeyEventType};

    let normalized = key.trim();
    if normalized.is_empty() || normalized.chars().count() > 1 && !is_named_key(normalized) {
        return Err(BrowserError::InvalidInput(format!("unsupported key: {key}")));
    }
    let (vk, text, code) = key_descriptor(normalized);
    let dispatch = |event_type: DispatchKeyEventType, with_text: bool| DispatchKeyEventParams {
        r#type: event_type,
        modifiers: None,
        timestamp: None,
        text: if with_text { text.map(str::to_string) } else { None },
        unmodified_text: if with_text { text.map(str::to_string) } else { None },
        key_identifier: None,
        code: Some(code.to_string()),
        key: Some(normalized.to_string()),
        windows_virtual_key_code: Some(vk),
        native_virtual_key_code: Some(vk),
        auto_repeat: None,
        is_keypad: None,
        is_system_key: None,
        location: None,
        commands: None,
    };
    page.execute(dispatch(DispatchKeyEventType::RawKeyDown, text.is_some()))
        .await?;
    if text.is_some() {
        page.execute(dispatch(DispatchKeyEventType::Char, true)).await?;
    }
    page.execute(dispatch(DispatchKeyEventType::KeyUp, false)).await?;
    Ok(())
}

fn is_named_key(key: &str) -> bool {
    matches!(
        key,
        "Enter" | "Tab" | "Escape" | "Backspace" | "Delete" | "ArrowLeft" | "ArrowUp"
            | "ArrowRight" | "ArrowDown" | "Home" | "End" | "PageUp" | "PageDown" | " "
    )
}

/// (windows virtual key code, text, DOM code) for supported keys.
fn key_descriptor(key: &str) -> (i64, Option<&str>, &str) {
    match key {
        "Enter" => (13, Some("\r"), "Enter"),
        "Tab" => (9, None, "Tab"),
        "Escape" => (27, None, "Escape"),
        "Backspace" => (8, None, "Backspace"),
        "Delete" => (46, None, "Delete"),
        "ArrowLeft" => (37, None, "ArrowLeft"),
        "ArrowUp" => (38, None, "ArrowUp"),
        "ArrowRight" => (39, None, "ArrowRight"),
        "ArrowDown" => (40, None, "ArrowDown"),
        "Home" => (36, None, "Home"),
        "End" => (35, None, "End"),
        "PageUp" => (33, None, "PageUp"),
        "PageDown" => (34, None, "PageDown"),
        " " => (32, Some(" "), "Space"),
        single => {
            let upper = single.to_uppercase();
            let vk = upper.chars().next().map(|c| c as i64).unwrap_or(0);
            (vk, Some(single), "KeyA")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_have_key_descriptors() {
        for key in ["Enter", "Tab", "Escape", "Backspace", "ArrowLeft", " "] {
            assert!(is_named_key(key), "{key}");
            let (vk, _, _) = key_descriptor(key);
            assert!(vk > 0, "{key}");
        }
        assert!(!is_named_key("F13"));
    }
}

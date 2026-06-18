use std::sync::{Mutex, OnceLock};

use super::Hwnd;

static BOTTOM_WEBVIEW_HWND: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static OVERLAY_WEBVIEW_HWND: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

pub fn register_bottom_webview(hwnd: Hwnd) -> Result<(), String> {
    // Stores the background webview HWND so docked windows can be layered above it.
    register_layer_hwnd(bottom_webview_state(), hwnd, "bottom webview")
}

pub fn register_overlay_webview(hwnd: Hwnd) -> Result<(), String> {
    // Stores the active overlay webview HWND so it can be raised above docked windows.
    register_layer_hwnd(overlay_webview_state(), hwnd, "overlay webview")
}

pub fn bottom_webview_hwnd() -> Option<Hwnd> {
    // Returns the registered background webview HWND when it is still available.
    layer_hwnd(bottom_webview_state())
}

pub fn overlay_hwnd() -> Option<Hwnd> {
    // Returns the registered overlay HWND when it is still available.
    layer_hwnd(overlay_webview_state())
}

fn register_layer_hwnd(
    state: &'static Mutex<Option<isize>>,
    hwnd: Hwnd,
    label: &str,
) -> Result<(), String> {
    // Writes one valid HWND into a global layer slot.
    if !hwnd.is_window() {
        return Err(format!("Invalid {label} HWND: {}", hwnd.0));
    }

    let mut state = state
        .lock()
        .map_err(|_| format!("{label} HWND state lock is poisoned."))?;
    *state = Some(hwnd.0);

    Ok(())
}

fn layer_hwnd(state: &'static Mutex<Option<isize>>) -> Option<Hwnd> {
    // Reads one layer HWND and verifies it still points to a live window.
    state
        .lock()
        .ok()
        .and_then(|state| state.map(Hwnd))
        .filter(|hwnd| hwnd.is_window())
}

fn bottom_webview_state() -> &'static Mutex<Option<isize>> {
    // Lazily creates the global bottom webview HWND state holder.
    BOTTOM_WEBVIEW_HWND.get_or_init(|| Mutex::new(None))
}

fn overlay_webview_state() -> &'static Mutex<Option<isize>> {
    // Lazily creates the global overlay webview HWND state holder.
    OVERLAY_WEBVIEW_HWND.get_or_init(|| Mutex::new(None))
}

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, State};

use crate::models::{DockPanelBounds, OverlayConfig};
use crate::state::OverlayStateStore;

#[tauri::command]
pub fn get_overlay(
    state: State<'_, OverlayStateStore>,
    id: String,
) -> Result<OverlayConfig, String> {
    // Returns one overlay config snapshot from Rust's local overlay cache.
    state.overlay(&id)
}

#[tauri::command]
pub fn move_overlay(
    app: AppHandle,
    state: State<'_, OverlayStateStore>,
    id: String,
    bounds: DockPanelBounds,
    is_minimized: Option<bool>,
) -> Result<OverlayConfig, String> {
    // Moves and resizes an overlay webview, then persists its clamped bounds to overlay.json.
    let overlay = state.move_overlay(&id, bounds, is_minimized.unwrap_or(false))?;
    if let Some(webview) = app.get_webview(&id) {
        webview
            .set_position(LogicalPosition::new(overlay.bounds.x, overlay.bounds.y))
            .map_err(|error| format!("Failed to move overlay {id}: {error}"))?;
        webview
            .set_size(LogicalSize::new(
                overlay.bounds.width as u32,
                overlay.bounds.height as u32,
            ))
            .map_err(|error| format!("Failed to resize overlay {id}: {error}"))?;
    }

    Ok(overlay)
}

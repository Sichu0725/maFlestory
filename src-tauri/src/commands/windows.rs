use tauri::{State, Window};

use crate::models::{DockPanelBounds, DockStateSnapshot, DockedWindowInfo, WindowInfo, WindowRect};
use crate::state::DockStateStore;
use crate::win32::{
    self, apply_child_style, apply_no_activate, get_parent, get_window_ex_style, get_window_rect,
    get_window_scale_factor, get_window_style, restore_parent, restore_window_rect,
    restore_window_style, set_window_position, Hwnd,
};

#[tauri::command]
pub fn list_windows(window: Window) -> Result<Vec<WindowInfo>, String> {
    // Returns visible top-level windows, excluding the current Tauri container.
    let container = tauri_hwnd(&window)?;
    let mut windows = win32::enum_windows()?;
    windows.retain(|window| window.hwnd != container.0);

    Ok(windows)
}

#[tauri::command]
pub fn list_docked_windows(
    state: State<'_, DockStateStore>,
) -> Result<Vec<DockedWindowInfo>, String> {
    // Returns window metadata for HWND values currently tracked in the Rust dock state.
    let entries = lock_entries(&state)?;
    Ok(entries
        .values()
        .filter_map(|snapshot| {
            let hwnd = Hwnd(snapshot.hwnd);
            hwnd.is_window()
                .then(|| docked_window_info(hwnd, snapshot.virtual_bounds))
        })
        .collect())
}

#[tauri::command]
pub fn dock_window(
    window: Window,
    state: State<'_, DockStateStore>,
    hwnd: isize,
    camera_bounds: DockPanelBounds,
) -> Result<(), String> {
    // Reparents the target HWND into the no-activate Tauri container at its current size.
    let container = tauri_hwnd(&window)?;
    let target = Hwnd::new(hwnd)?;

    if target.0 == container.0 {
        return Err("Cannot dock the Tauri container window into itself.".to_string());
    }

    ensure_valid_hwnd(target)?;

    let already_docked = {
        let entries = lock_entries(&state)?;
        entries.contains_key(&target.0)
    };

    if already_docked {
        return Ok(());
    }

    let original_rect = get_window_rect(target)?;
    let virtual_bounds = centered_child_virtual_bounds(camera_bounds, target, original_rect)?;
    let snapshot = capture_snapshot(target, original_rect, virtual_bounds)?;
    let target_bounds = viewport_child_bounds(virtual_bounds, camera_bounds);

    apply_no_activate(container)?;

    if let Err(error) = apply_child_style(target).and_then(|_| apply_no_activate(target)) {
        let _ = restore_window_style(target, snapshot.original_style, snapshot.original_ex_style);
        return Err(error);
    }

    if let Err(error) = win32::set_parent(target, container) {
        let _ = restore_window_style(target, snapshot.original_style, snapshot.original_ex_style);
        return Err(error);
    }

    if let Err(error) = win32::install_mouse_no_activate_handler(target) {
        eprintln!("failed to install no-activate handler on docked window: {error}");
    }
    if let Err(error) = win32::register_focus_guard_window(target) {
        eprintln!("failed to register focus guard on docked window: {error}");
    }

    if let Err(error) = set_window_position(target, target_bounds) {
        let _ = win32::unregister_focus_guard_window(target);
        let _ = win32::uninstall_no_activate_handler(target);
        let _ = restore_parent(target, Hwnd(snapshot.original_parent));
        let _ = restore_window_style(target, snapshot.original_style, snapshot.original_ex_style);
        return Err(error);
    }

    let mut entries = lock_entries(&state)?;
    entries.insert(target.0, snapshot);

    Ok(())
}

#[tauri::command]
pub fn undock_window(state: State<'_, DockStateStore>, hwnd: isize) -> Result<(), String> {
    // Restores a docked window to its saved parent, style, and rectangle.
    let target = Hwnd::new(hwnd)?;
    let snapshot = {
        let entries = lock_entries(&state)?;
        entries
            .get(&target.0)
            .cloned()
            .ok_or_else(|| format!("No dock state found for HWND {hwnd}"))?
    };

    restore_docked_snapshot(&snapshot)?;

    let mut entries = lock_entries(&state)?;
    entries.remove(&target.0);

    Ok(())
}

pub fn undock_all_windows(state: &DockStateStore) {
    // Restores every docked window when the Tauri process is exiting.
    let snapshots = match state.entries.lock() {
        Ok(mut entries) => entries
            .drain()
            .map(|(_hwnd, snapshot)| snapshot)
            .collect::<Vec<_>>(),
        Err(_) => {
            eprintln!("failed to undock on exit: dock state store lock is poisoned");
            return;
        }
    };

    for snapshot in snapshots {
        if let Err(error) = restore_docked_snapshot(&snapshot) {
            eprintln!(
                "failed to undock HWND {} during app exit: {error}",
                snapshot.hwnd
            );
        }
    }
}

#[tauri::command]
pub fn resize_docked_window(
    window: Window,
    hwnd: isize,
    bounds: DockPanelBounds,
) -> Result<(), String> {
    // Resizes a docked child window without activating it or changing Z-order.
    let target = Hwnd::new(hwnd)?;
    ensure_valid_hwnd(target)?;
    set_window_position(target, physical_bounds(&window, bounds)?)
}

#[tauri::command]
pub fn position_docked_window(
    window: Window,
    state: State<'_, DockStateStore>,
    hwnd: isize,
    virtual_bounds: DockPanelBounds,
    viewport_bounds: DockPanelBounds,
) -> Result<(), String> {
    // Moves a docked child from virtual canvas coordinates into the visible viewport.
    let target = Hwnd::new(hwnd)?;
    ensure_valid_hwnd(target)?;

    {
        let entries = lock_entries(&state)?;
        if !entries.contains_key(&target.0) {
            return Err(format!("No dock state found for HWND {hwnd}"));
        }
    }

    set_window_position(target, physical_bounds(&window, viewport_bounds)?)?;

    let mut entries = lock_entries(&state)?;
    if let Some(snapshot) = entries.get_mut(&target.0) {
        snapshot.virtual_bounds = virtual_bounds;
    }

    Ok(())
}

#[tauri::command]
pub fn sync_docked_window_bounds(
    window: Window,
    state: State<'_, DockStateStore>,
    camera_bounds: DockPanelBounds,
) -> Result<Vec<DockedWindowInfo>, String> {
    // Saves current child HWND rectangles as virtual bounds before camera movement.
    let container = tauri_hwnd(&window)?;
    let mut entries = lock_entries(&state)?;
    let mut synced_windows = Vec::new();

    for snapshot in entries.values_mut() {
        let target = Hwnd(snapshot.hwnd);
        if !target.is_window() {
            continue;
        }

        let viewport_bounds =
            css_bounds(&window, win32::get_child_window_bounds(container, target)?)?;
        snapshot.virtual_bounds = DockPanelBounds {
            x: camera_bounds.x + viewport_bounds.x,
            y: camera_bounds.y + viewport_bounds.y,
            width: viewport_bounds.width,
            height: viewport_bounds.height,
        };
        synced_windows.push(docked_window_info(target, snapshot.virtual_bounds));
    }

    Ok(synced_windows)
}

fn tauri_hwnd(window: &Window) -> Result<Hwnd, String> {
    // Extracts the native HWND for the invoking Tauri window.
    let hwnd = window
        .hwnd()
        .map_err(|error| format!("Failed to get Tauri HWND: {error}"))?;

    Hwnd::new(hwnd.0 as isize)
}

fn capture_snapshot(
    hwnd: Hwnd,
    original_rect: WindowRect,
    virtual_bounds: DockPanelBounds,
) -> Result<DockStateSnapshot, String> {
    // Captures enough original window state to undo docking later.
    Ok(DockStateSnapshot {
        hwnd: hwnd.0,
        original_parent: get_parent(hwnd)?.0,
        original_style: get_window_style(hwnd)?,
        original_ex_style: get_window_ex_style(hwnd)?,
        original_rect,
        virtual_bounds,
    })
}

fn ensure_valid_hwnd(hwnd: Hwnd) -> Result<(), String> {
    // Rejects stale HWND values before mutating native window state.
    if !hwnd.is_window() {
        return Err(format!("Invalid HWND: {}", hwnd.0));
    }

    Ok(())
}

fn docked_window_info(hwnd: Hwnd, virtual_bounds: DockPanelBounds) -> DockedWindowInfo {
    // Builds display metadata for a docked HWND with a readable fallback title.
    let mut window = win32::get_window_info(hwnd);
    if window.title.trim().is_empty() {
        window.title = format!("HWND {}", hwnd.0);
    }

    DockedWindowInfo {
        hwnd: window.hwnd,
        title: window.title,
        class_name: window.class_name,
        process_id: window.process_id,
        virtual_bounds,
    }
}

fn restore_docked_snapshot(snapshot: &DockStateSnapshot) -> Result<(), String> {
    // Restores one docked HWND from its saved parent, style, and rectangle snapshot.
    let target = Hwnd::new(snapshot.hwnd)?;

    if let Err(error) = win32::uninstall_no_activate_handler(target) {
        eprintln!("failed to uninstall no-activate handler on docked window: {error}");
    }
    if let Err(error) = win32::unregister_focus_guard_window(target) {
        eprintln!("failed to unregister focus guard on docked window: {error}");
    }

    restore_parent(target, Hwnd(snapshot.original_parent))?;
    restore_window_style(target, snapshot.original_style, snapshot.original_ex_style)?;
    restore_window_rect(target, snapshot.original_rect)
}

fn physical_bounds(window: &Window, bounds: DockPanelBounds) -> Result<DockPanelBounds, String> {
    // Converts React CSS pixel bounds into Windows physical pixels.
    let scale_factor = window
        .scale_factor()
        .map_err(|error| format!("Failed to read Tauri scale factor: {error}"))?;

    Ok(DockPanelBounds {
        x: scale_i32(bounds.x, scale_factor),
        y: scale_i32(bounds.y, scale_factor),
        width: scale_i32(bounds.width, scale_factor),
        height: scale_i32(bounds.height, scale_factor),
    })
}

fn css_bounds(window: &Window, bounds: DockPanelBounds) -> Result<DockPanelBounds, String> {
    // Converts Windows physical pixel bounds into React CSS pixels.
    let scale_factor = window
        .scale_factor()
        .map_err(|error| format!("Failed to read Tauri scale factor: {error}"))?;

    Ok(DockPanelBounds {
        x: unscale_i32(bounds.x, scale_factor),
        y: unscale_i32(bounds.y, scale_factor),
        width: unscale_i32(bounds.width, scale_factor),
        height: unscale_i32(bounds.height, scale_factor),
    })
}

fn scale_i32(value: i32, scale_factor: f64) -> i32 {
    // Scales a CSS pixel integer into a rounded physical pixel integer.
    (value as f64 * scale_factor).round() as i32
}

fn unscale_i32(value: i32, scale_factor: f64) -> i32 {
    // Converts Windows physical pixels back into React CSS pixels.
    (value as f64 / scale_factor).round() as i32
}

fn centered_child_virtual_bounds(
    camera_bounds: DockPanelBounds,
    hwnd: Hwnd,
    rect: WindowRect,
) -> Result<DockPanelBounds, String> {
    // Creates initial virtual bounds centered in the current viewport camera.
    let source_scale_factor = get_window_scale_factor(hwnd);
    let width = clamp_child_dimension(
        unscale_i32(rect.right - rect.left, source_scale_factor),
        camera_bounds.width,
    );
    let height = clamp_child_dimension(
        unscale_i32(rect.bottom - rect.top, source_scale_factor),
        camera_bounds.height,
    );

    Ok(DockPanelBounds {
        x: camera_bounds.x + (camera_bounds.width - width) / 2,
        y: camera_bounds.y + (camera_bounds.height - height) / 2,
        width,
        height,
    })
}

fn clamp_child_dimension(size: i32, viewport_size: i32) -> i32 {
    // Limits a newly docked window dimension to 90% of the current Tauri viewport dimension.
    let max_size = ((viewport_size as f64) * 0.9).round() as i32;
    size.min(max_size).max(1)
}

fn viewport_child_bounds(
    virtual_bounds: DockPanelBounds,
    camera_bounds: DockPanelBounds,
) -> DockPanelBounds {
    // Projects virtual canvas bounds into the currently visible viewport.
    DockPanelBounds {
        x: virtual_bounds.x - camera_bounds.x,
        y: virtual_bounds.y - camera_bounds.y,
        width: virtual_bounds.width,
        height: virtual_bounds.height,
    }
}

fn lock_entries<'a>(
    state: &'a State<'_, DockStateStore>,
) -> Result<std::sync::MutexGuard<'a, std::collections::HashMap<isize, DockStateSnapshot>>, String>
{
    // Locks the dock state map and converts poisoning into a command error.
    state
        .entries
        .lock()
        .map_err(|_| "Dock state store lock is poisoned.".to_string())
}

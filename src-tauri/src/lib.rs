mod commands;
mod models;
mod state;
mod win32;

use state::DockStateStore;
use tauri::{Manager, RunEvent, WebviewWindow};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Boots the Tauri shell as the fullscreen-sized native dock container.
    let app = tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                configure_main_window(&window)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::windows::list_windows,
            commands::windows::list_docked_windows,
            commands::windows::dock_window,
            commands::windows::undock_window,
            commands::windows::resize_docked_window,
            commands::windows::position_docked_window,
            commands::windows::sync_docked_window_bounds,
        ])
        .manage(DockStateStore::default())
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            let state = app_handle.state::<DockStateStore>();
            commands::windows::undock_all_windows(state.inner());
        }
    });
}

fn configure_main_window(window: &WebviewWindow) -> tauri::Result<()> {
    // Sizes the decorated Tauri window to the primary monitor and keeps it non-activating.
    window.set_decorations(true)?;
    window.set_always_on_top(false)?;
    window.set_always_on_bottom(true)?;
    window.set_focusable(false)?;
    window.set_skip_taskbar(false)?;
    window.set_resizable(true)?;
    window.maximize()?;

    #[cfg(windows)]
    {
        let hwnd = window.hwnd()?;
        let hwnd = win32::Hwnd(hwnd.0 as isize);
        if let Err(error) = win32::apply_no_activate(hwnd) {
            eprintln!("failed to apply no-activate to Tauri window: {error}");
        }
        if let Err(error) = win32::install_no_activate_handler(hwnd) {
            eprintln!("failed to install no-activate handler: {error}");
        }
        if let Err(error) = win32::register_focus_guard_window(hwnd) {
            eprintln!("failed to register Tauri focus guard: {error}");
        }
        if let Err(error) = win32::install_foreground_restore_hook() {
            eprintln!("failed to install foreground restore hook: {error}");
        }
        if let Err(error) = win32::send_window_to_bottom(hwnd) {
            eprintln!("failed to send Tauri window to bottom: {error}");
        }
    }

    Ok(())
}

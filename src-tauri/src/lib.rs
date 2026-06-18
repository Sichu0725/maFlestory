mod commands;
mod models;
mod state;
mod win32;

use crate::models::OverlayConfig;
use state::DockStateStore;
use state::OverlayStateStore;
use tauri::webview::{Color, WebviewBuilder};
use tauri::{LogicalPosition, LogicalSize, Manager, RunEvent, WebviewUrl, WebviewWindow};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Boots the Tauri shell as the fullscreen-sized native dock container.
    let app = tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                configure_main_window(&window)?;
                let overlay_path = app.path().app_data_dir()?.join("overlay.json");
                let overlay_state = OverlayStateStore::load(overlay_path).map_err(setup_error)?;
                let visible_overlays = overlay_state.visible_overlays().map_err(setup_error)?;
                configure_overlay_webviews(&window, &visible_overlays)?;
                app.manage(overlay_state);
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
            commands::overlays::get_overlay,
            commands::overlays::move_overlay,
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
    window.set_skip_taskbar(true)?;
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

        if let Err(error) = register_main_webview_layer(window) {
            eprintln!("failed to register main webview layer: {error}");
        }
    }

    Ok(())
}

#[cfg(windows)]
fn register_main_webview_layer(window: &WebviewWindow) -> Result<(), String> {
    // Registers the default app-stage webview as the bottom native child layer.
    window
        .as_ref()
        .with_webview(|webview| {
            let controller = webview.controller();
            let mut webview_hwnd = Default::default();
            if unsafe { controller.ParentWindow(&mut webview_hwnd) }.is_err() {
                eprintln!("failed to read main webview HWND");
                return;
            }

            let hwnd = win32::Hwnd(webview_hwnd.0 as isize);
            if let Err(error) = win32::register_bottom_webview(hwnd) {
                eprintln!("failed to register bottom webview HWND: {error}");
            }
            if let Err(error) = win32::send_window_to_bottom(hwnd) {
                eprintln!("failed to send main webview child to bottom: {error}");
            }
        })
        .map_err(|error| format!("Failed to access main webview: {error}"))
}

fn configure_overlay_webviews(
    window: &WebviewWindow,
    overlays: &[OverlayConfig],
) -> tauri::Result<()> {
    // Adds one small child webview for each visible overlay config.
    let host_window = window.as_ref().window();

    for overlay_config in overlays {
        let overlay = host_window.add_child(
            WebviewBuilder::new(
                overlay_config.id.clone(),
                WebviewUrl::App(overlay_config.route.clone().into()),
            )
            .transparent(true)
            .background_color(Color(0, 0, 0, 0))
            .focused(false),
            LogicalPosition::new(overlay_config.bounds.x, overlay_config.bounds.y),
            LogicalSize::new(
                overlay_config.bounds.width as u32,
                overlay_config.bounds.height as u32,
            ),
        )?;

        #[cfg(windows)]
        {
            overlay.with_webview(|webview| {
                let controller = webview.controller();
                let mut overlay_hwnd = Default::default();
                if unsafe { controller.ParentWindow(&mut overlay_hwnd) }.is_err() {
                    eprintln!("failed to read overlay webview HWND");
                    return;
                }

                let hwnd = win32::Hwnd(overlay_hwnd.0 as isize);
                if let Err(error) = win32::register_overlay_webview(hwnd) {
                    eprintln!("failed to register overlay webview HWND: {error}");
                }
                if let Err(error) = win32::install_mouse_no_activate_handler(hwnd) {
                    eprintln!("failed to install overlay no-activate handler: {error}");
                }
                if let Err(error) = win32::bring_window_to_top(hwnd) {
                    eprintln!("failed to raise overlay webview: {error}");
                }
            })?;
        }
    }

    Ok(())
}

fn setup_error(message: String) -> tauri::Error {
    // Converts setup String errors into Tauri IO errors.
    tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, message))
}

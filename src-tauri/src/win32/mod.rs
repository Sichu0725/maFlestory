mod hwnd;
mod window_activation;
mod window_enum;
mod window_layers;
mod window_parent;
mod window_position;
mod window_style;

pub use hwnd::Hwnd;
pub use window_activation::{
    install_foreground_restore_hook, install_mouse_no_activate_handler,
    install_no_activate_handler, register_focus_guard_window, uninstall_no_activate_handler,
    unregister_focus_guard_window,
};
pub use window_enum::{enum_windows, get_window_info};
pub use window_layers::{
    bottom_webview_hwnd, overlay_hwnd, register_bottom_webview, register_overlay_webview,
};
pub use window_parent::{get_parent, restore_parent, set_parent};
pub use window_position::{
    bring_window_to_top, get_child_window_bounds, get_window_rect, get_window_scale_factor,
    restore_window_rect, send_window_to_bottom, set_window_position,
};
pub use window_style::{
    apply_child_style, apply_no_activate, get_window_ex_style, get_window_style,
    restore_window_style,
};

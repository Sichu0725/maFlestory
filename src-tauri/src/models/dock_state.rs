use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct DockPanelBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DockStateSnapshot {
    pub hwnd: isize,
    pub original_parent: isize,
    pub original_style: isize,
    pub original_ex_style: isize,
    pub original_rect: WindowRect,
    pub virtual_bounds: DockPanelBounds,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockedWindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub class_name: Option<String>,
    pub process_id: Option<u32>,
    pub virtual_bounds: DockPanelBounds,
}

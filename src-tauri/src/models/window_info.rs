use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub class_name: Option<String>,
    pub process_id: Option<u32>,
}

use crate::models::WindowInfo;

use super::Hwnd;

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
};

pub fn enum_windows() -> Result<Vec<WindowInfo>, String> {
    // Enumerates visible top-level windows with non-empty titles.
    let mut windows = Vec::<WindowInfo>::new();
    let result =
        unsafe { EnumWindows(Some(enum_window_callback), &mut windows as *mut _ as LPARAM) };

    if result == 0 {
        return Err(last_os_error("EnumWindows failed"));
    }

    Ok(windows)
}

pub fn get_window_info(hwnd: Hwnd) -> WindowInfo {
    // Reads user-facing metadata for one HWND, including child windows.
    let hwnd = hwnd.as_raw();

    WindowInfo {
        hwnd: hwnd as isize,
        title: window_text(hwnd),
        class_name: non_empty_string(class_name(hwnd)),
        process_id: process_id(hwnd),
    }
}

unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> i32 {
    // Collects one visible, user-facing window during EnumWindows traversal.
    let windows = unsafe { &mut *(lparam as *mut Vec<WindowInfo>) };

    if !is_candidate_window(hwnd) {
        return 1;
    }

    let title = window_text(hwnd);
    if title.trim().is_empty() {
        return 1;
    }

    windows.push(WindowInfo {
        hwnd: hwnd as isize,
        title,
        class_name: non_empty_string(class_name(hwnd)),
        process_id: process_id(hwnd),
    });

    1
}

fn is_candidate_window(hwnd: HWND) -> bool {
    // Filters out invisible and owned helper windows from the app-facing list.
    unsafe { IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() }
}

fn window_text(hwnd: HWND) -> String {
    // Reads the UTF-16 window title into a Rust String.
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..written.max(0) as usize])
}

fn class_name(hwnd: HWND) -> String {
    // Reads the Win32 class name for diagnostics and filtering.
    let mut buffer = vec![0u16; 256];
    let written = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..written.max(0) as usize])
}

fn process_id(hwnd: HWND) -> Option<u32> {
    // Reads the owning process id for the supplied HWND.
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id);
    }

    (process_id != 0).then_some(process_id)
}

fn non_empty_string(value: String) -> Option<String> {
    // Normalizes empty Win32 strings into absent optional values.
    (!value.is_empty()).then_some(value)
}

fn last_os_error(context: &str) -> String {
    // Formats the last Windows error with a short operation label.
    format!("{context}: {}", std::io::Error::last_os_error())
}

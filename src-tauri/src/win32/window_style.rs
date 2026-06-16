use super::Hwnd;

use windows_sys::Win32::Foundation::{GetLastError, SetLastError, ERROR_SUCCESS};
use windows_sys::Win32::Graphics::Gdi::{
    RedrawWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, WS_CHILD,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

const STYLE_REFRESH_FLAGS: u32 =
    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_SHOWWINDOW;
const REDRAW_FLAGS: u32 = RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW;

pub fn get_window_style(hwnd: Hwnd) -> Result<isize, String> {
    // Reads the regular Win32 style bits for a window.
    get_window_long(hwnd, GWL_STYLE, "GetWindowLongPtrW(GWL_STYLE)")
}

pub fn get_window_ex_style(hwnd: Hwnd) -> Result<isize, String> {
    // Reads the extended Win32 style bits for a window.
    get_window_long(hwnd, GWL_EXSTYLE, "GetWindowLongPtrW(GWL_EXSTYLE)")
}

pub fn apply_child_style(hwnd: Hwnd) -> Result<(), String> {
    // Converts a top-level target window into a dock-compatible child window.
    let style = get_window_style(hwnd)?;
    let next_style = normalize_child_style(style);
    set_window_long(hwnd, GWL_STYLE, next_style, "SetWindowLongPtrW(GWL_STYLE)")?;

    let ex_style = get_window_ex_style(hwnd)?;
    let next_ex_style = normalize_child_ex_style(ex_style);
    set_window_long(
        hwnd,
        GWL_EXSTYLE,
        next_ex_style,
        "SetWindowLongPtrW(GWL_EXSTYLE)",
    )?;

    refresh_frame(hwnd)
}

pub fn apply_no_activate(hwnd: Hwnd) -> Result<(), String> {
    // Adds WS_EX_NOACTIVATE to a native window.
    let ex_style = get_window_ex_style(hwnd)?;
    let next_ex_style = ex_style | WS_EX_NOACTIVATE as isize;
    set_window_long(
        hwnd,
        GWL_EXSTYLE,
        next_ex_style,
        "SetWindowLongPtrW(GWL_EXSTYLE)",
    )?;
    refresh_frame(hwnd)
}

pub fn restore_window_style(hwnd: Hwnd, style: isize, ex_style: isize) -> Result<(), String> {
    // Restores the regular and extended style bits captured before docking.
    set_window_long(hwnd, GWL_STYLE, style, "SetWindowLongPtrW(GWL_STYLE)")?;
    set_window_long(
        hwnd,
        GWL_EXSTYLE,
        ex_style,
        "SetWindowLongPtrW(GWL_EXSTYLE)",
    )?;
    refresh_frame(hwnd)
}

fn get_window_long(hwnd: Hwnd, index: i32, context: &str) -> Result<isize, String> {
    // Reads a pointer-sized style value and handles zero as a valid result.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        let value = GetWindowLongPtrW(hwnd.as_raw(), index);
        let error = GetLastError();

        if value == 0 && error != ERROR_SUCCESS {
            return Err(format!(
                "{context} failed for {}: {}",
                hwnd.0,
                std::io::Error::from_raw_os_error(error as i32)
            ));
        }

        Ok(value)
    }
}

fn set_window_long(hwnd: Hwnd, index: i32, value: isize, context: &str) -> Result<(), String> {
    // Writes a pointer-sized style value and handles zero as a valid previous value.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        let previous = SetWindowLongPtrW(hwnd.as_raw(), index, value);
        let error = GetLastError();

        if previous == 0 && error != ERROR_SUCCESS {
            return Err(format!(
                "{context} failed for {}: {}",
                hwnd.0,
                std::io::Error::from_raw_os_error(error as i32)
            ));
        }
    }

    Ok(())
}

fn normalize_child_style(style: isize) -> isize {
    // Makes the window a child while preserving app-owned rendering styles.
    (style & !(WS_POPUP as isize)) | WS_CHILD as isize | WS_VISIBLE as isize
}

fn normalize_child_ex_style(ex_style: isize) -> isize {
    // Removes only top-level shell bits and preserves no-activate for docked windows.
    let remove_ex_style = WS_EX_APPWINDOW | WS_EX_TOPMOST;

    ex_style & !(remove_ex_style as isize)
}

fn refresh_frame(hwnd: Hwnd) -> Result<(), String> {
    // Asks Windows to re-read style changes without moving, resizing, or activating.
    let result = unsafe {
        SetWindowPos(
            hwnd.as_raw(),
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            STYLE_REFRESH_FLAGS,
        )
    };

    if result == 0 {
        return Err(format!(
            "SetWindowPos frame refresh failed for {}: {}",
            hwnd.0,
            std::io::Error::last_os_error()
        ));
    }

    redraw_window(hwnd)
}

fn redraw_window(hwnd: Hwnd) -> Result<(), String> {
    // Forces a repaint after style changes so redirected surfaces are recreated.
    let result = unsafe {
        RedrawWindow(
            hwnd.as_raw(),
            std::ptr::null(),
            std::ptr::null_mut(),
            REDRAW_FLAGS,
        )
    };

    if result == 0 {
        return Err(format!(
            "RedrawWindow failed for {}: {}",
            hwnd.0,
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
}

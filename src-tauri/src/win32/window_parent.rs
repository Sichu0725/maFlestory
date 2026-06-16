use super::Hwnd;

use windows_sys::Win32::Foundation::{GetLastError, SetLastError, ERROR_SUCCESS};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetParent, SetParent};

pub fn get_parent(hwnd: Hwnd) -> Result<Hwnd, String> {
    // Reads the current parent or owner HWND for a window.
    ensure_valid_window(hwnd)?;
    Ok(Hwnd(unsafe { GetParent(hwnd.as_raw()) as isize }))
}

pub fn restore_parent(child: Hwnd, parent: Hwnd) -> Result<(), String> {
    // Restores a previously saved parent HWND.
    set_parent_raw(child, parent)
}

pub fn set_parent(child: Hwnd, parent: Hwnd) -> Result<(), String> {
    // Reparents a child window into the supplied parent window.
    ensure_valid_window(child)?;
    ensure_valid_window(parent)?;
    set_parent_raw(child, parent)
}

fn set_parent_raw(child: Hwnd, parent: Hwnd) -> Result<(), String> {
    // Calls SetParent while preserving null as a valid restore target.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        let previous = SetParent(child.as_raw(), parent.as_raw());
        let error = GetLastError();

        if previous.is_null() && error != ERROR_SUCCESS {
            return Err(format!(
                "SetParent failed for {} -> {}: {}",
                child.0,
                parent.0,
                std::io::Error::from_raw_os_error(error as i32)
            ));
        }
    }

    Ok(())
}

fn ensure_valid_window(hwnd: Hwnd) -> Result<(), String> {
    // Rejects stale or null HWND values before calling parent APIs.
    if hwnd.0 == 0 || !hwnd.is_window() {
        return Err(format!("Invalid HWND: {}", hwnd.0));
    }

    Ok(())
}

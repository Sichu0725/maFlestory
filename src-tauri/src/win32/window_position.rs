use crate::models::DockPanelBounds;
use crate::models::WindowRect;

use super::Hwnd;

use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    RedrawWindow, ScreenToClient, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE,
    RDW_UPDATENOW,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SetWindowPos, HWND_BOTTOM, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER,
};

const DEFAULT_DPI: f64 = 96.0;
const POSITION_FLAGS: u32 = SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED;
const BOTTOM_FLAGS: u32 = SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE;
const REDRAW_FLAGS: u32 = RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW;

pub fn get_window_rect(hwnd: Hwnd) -> Result<WindowRect, String> {
    // Reads the current screen-space rectangle for a window.
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    let result = unsafe { GetWindowRect(hwnd.as_raw(), &mut rect) };
    if result == 0 {
        return Err(last_os_error("GetWindowRect failed"));
    }

    Ok(WindowRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

pub fn get_window_scale_factor(hwnd: Hwnd) -> f64 {
    // Reads the DPI scale for the monitor where the target window currently lives.
    let dpi = unsafe { GetDpiForWindow(hwnd.as_raw()) };
    if dpi == 0 {
        return 1.0;
    }

    dpi as f64 / DEFAULT_DPI
}

pub fn set_window_position(hwnd: Hwnd, bounds: DockPanelBounds) -> Result<(), String> {
    // Places a docked child window without activating it or changing Z-order.
    set_position(hwnd, bounds.x, bounds.y, bounds.width, bounds.height)
}

pub fn get_child_window_bounds(parent: Hwnd, child: Hwnd) -> Result<DockPanelBounds, String> {
    // Reads a child HWND rectangle in its parent's client coordinate space.
    let rect = get_window_rect(child)?;
    let mut top_left = POINT {
        x: rect.left,
        y: rect.top,
    };
    let mut bottom_right = POINT {
        x: rect.right,
        y: rect.bottom,
    };

    let top_left_result = unsafe { ScreenToClient(parent.as_raw(), &mut top_left) };
    if top_left_result == 0 {
        return Err(last_os_error("ScreenToClient(top-left) failed"));
    }

    let bottom_right_result = unsafe { ScreenToClient(parent.as_raw(), &mut bottom_right) };
    if bottom_right_result == 0 {
        return Err(last_os_error("ScreenToClient(bottom-right) failed"));
    }

    Ok(DockPanelBounds {
        x: top_left.x,
        y: top_left.y,
        width: bottom_right.x - top_left.x,
        height: bottom_right.y - top_left.y,
    })
}

pub fn restore_window_rect(hwnd: Hwnd, rect: WindowRect) -> Result<(), String> {
    // Restores a top-level window to a previously saved screen-space rectangle.
    set_position(
        hwnd,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

pub fn send_window_to_bottom(hwnd: Hwnd) -> Result<(), String> {
    // Sends a top-level window behind other windows once without enabling always-bottom state.
    let result = unsafe { SetWindowPos(hwnd.as_raw(), HWND_BOTTOM, 0, 0, 0, 0, BOTTOM_FLAGS) };

    if result == 0 {
        return Err(last_os_error("SetWindowPos(HWND_BOTTOM) failed"));
    }

    Ok(())
}

fn set_position(hwnd: Hwnd, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    // Calls SetWindowPos with the project's no-activate positioning contract.
    if width <= 0 || height <= 0 {
        return Err(format!("Window bounds must be positive: {width}x{height}"));
    }

    let result = unsafe {
        SetWindowPos(
            hwnd.as_raw(),
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            POSITION_FLAGS,
        )
    };

    if result == 0 {
        return Err(last_os_error("SetWindowPos failed"));
    }

    redraw_window(hwnd)
}

fn last_os_error(context: &str) -> String {
    // Formats the last Windows error with a short operation label.
    format!("{context}: {}", std::io::Error::last_os_error())
}

fn redraw_window(hwnd: Hwnd) -> Result<(), String> {
    // Requests an immediate repaint after parent or position changes.
    let result = unsafe {
        RedrawWindow(
            hwnd.as_raw(),
            std::ptr::null(),
            std::ptr::null_mut(),
            REDRAW_FLAGS,
        )
    };

    if result == 0 {
        return Err(last_os_error("RedrawWindow failed"));
    }

    Ok(())
}

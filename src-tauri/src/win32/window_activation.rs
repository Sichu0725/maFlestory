use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use super::Hwnd;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetActiveWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, EnumChildWindows, GetForegroundWindow,
    GetWindowThreadProcessId, IsChild, SetForegroundWindow, SetWindowLongPtrW,
    EVENT_SYSTEM_FOREGROUND, GWLP_WNDPROC, MA_NOACTIVATE, WINEVENT_OUTOFCONTEXT, WM_ACTIVATE,
    WM_MOUSEACTIVATE, WM_NCACTIVATE, WM_SETFOCUS, WNDPROC,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActivationGuardMode {
    MouseOnly,
    Strict,
}

#[derive(Clone, Copy)]
struct WndProcEntry {
    previous: isize,
    mode: ActivationGuardMode,
}

#[derive(Clone, Copy)]
struct KeyboardTarget {
    foreground: isize,
    focus: Option<isize>,
}

static PREVIOUS_WNDPROCS: Mutex<Option<HashMap<isize, WndProcEntry>>> = Mutex::new(None);
static GUARDED_FOREGROUND_WINDOWS: Mutex<Option<HashSet<isize>>> = Mutex::new(None);
static LAST_KEYBOARD_TARGET: Mutex<Option<KeyboardTarget>> = Mutex::new(None);
static FOREGROUND_HOOK: Mutex<Option<isize>> = Mutex::new(None);

pub fn install_foreground_restore_hook() -> Result<(), String> {
    // Watches foreground changes and restores the user's previous typing target.
    let mut hook = FOREGROUND_HOOK
        .lock()
        .map_err(|_| "Foreground hook lock is poisoned.".to_string())?;

    if hook.is_some() {
        return Ok(());
    }

    let handle = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(foreground_event_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    if handle.is_null() {
        return Err(format!(
            "SetWinEventHook(EVENT_SYSTEM_FOREGROUND) failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    *hook = Some(handle as isize);
    remember_current_foreground();
    Ok(())
}

pub fn register_focus_guard_window(hwnd: Hwnd) -> Result<(), String> {
    // Registers a HWND that should never become the keyboard foreground owner.
    let mut guarded_windows = GUARDED_FOREGROUND_WINDOWS
        .lock()
        .map_err(|_| "Guarded foreground window lock is poisoned.".to_string())?;
    let guarded_windows = guarded_windows.get_or_insert_with(HashSet::new);
    guarded_windows.insert(hwnd.0);

    Ok(())
}

pub fn unregister_focus_guard_window(hwnd: Hwnd) -> Result<(), String> {
    // Removes a HWND from foreground restoration tracking.
    let mut guarded_windows = GUARDED_FOREGROUND_WINDOWS
        .lock()
        .map_err(|_| "Guarded foreground window lock is poisoned.".to_string())?;
    if let Some(guarded_windows) = guarded_windows.as_mut() {
        guarded_windows.remove(&hwnd.0);
    }

    Ok(())
}

pub fn install_no_activate_handler(hwnd: Hwnd) -> Result<(), String> {
    // Installs strict activation guards on one HWND and its current child HWNDs.
    install_no_activate_handler_for_hwnd(hwnd, ActivationGuardMode::Strict)?;
    unsafe {
        EnumChildWindows(
            hwnd.as_raw(),
            Some(enum_child_window_strict_callback),
            std::ptr::null_mut::<()>() as LPARAM,
        );
    }

    Ok(())
}

pub fn install_mouse_no_activate_handler(hwnd: Hwnd) -> Result<(), String> {
    // Installs mouse-only activation guards so clicks/drags still flow normally.
    install_no_activate_handler_for_hwnd(hwnd, ActivationGuardMode::MouseOnly)?;
    unsafe {
        EnumChildWindows(
            hwnd.as_raw(),
            Some(enum_child_window_mouse_only_callback),
            std::ptr::null_mut::<()>() as LPARAM,
        );
    }

    Ok(())
}

pub fn uninstall_no_activate_handler(hwnd: Hwnd) -> Result<(), String> {
    // Restores original window procedures for one HWND and its current child HWNDs.
    uninstall_no_activate_handler_for_hwnd(hwnd)?;
    unsafe {
        EnumChildWindows(
            hwnd.as_raw(),
            Some(enum_child_window_restore_callback),
            std::ptr::null_mut::<()>() as LPARAM,
        );
    }

    Ok(())
}

unsafe extern "system" fn enum_child_window_strict_callback(hwnd: HWND, _lparam: LPARAM) -> i32 {
    // Applies the strict activation guard to a child HWND.
    let _ = install_no_activate_handler_for_hwnd(Hwnd(hwnd as isize), ActivationGuardMode::Strict);
    1
}

unsafe extern "system" fn enum_child_window_mouse_only_callback(
    hwnd: HWND,
    _lparam: LPARAM,
) -> i32 {
    // Applies the mouse-only activation guard to a child HWND.
    let _ =
        install_no_activate_handler_for_hwnd(Hwnd(hwnd as isize), ActivationGuardMode::MouseOnly);
    1
}

unsafe extern "system" fn enum_child_window_restore_callback(hwnd: HWND, _lparam: LPARAM) -> i32 {
    // Restores the original window procedure for a child HWND.
    let _ = uninstall_no_activate_handler_for_hwnd(Hwnd(hwnd as isize));
    1
}

fn install_no_activate_handler_for_hwnd(
    hwnd: Hwnd,
    mode: ActivationGuardMode,
) -> Result<(), String> {
    // Subclasses one HWND once so clicks do not activate it.
    let mut previous_wndprocs = PREVIOUS_WNDPROCS
        .lock()
        .map_err(|_| "No-activate wndproc map lock is poisoned.".to_string())?;
    let previous_wndprocs = previous_wndprocs.get_or_insert_with(HashMap::new);

    if let Some(entry) = previous_wndprocs.get_mut(&hwnd.0) {
        if mode == ActivationGuardMode::Strict {
            entry.mode = ActivationGuardMode::Strict;
        }
        return Ok(());
    }

    let previous = unsafe {
        SetWindowLongPtrW(
            hwnd.as_raw(),
            GWLP_WNDPROC,
            no_activate_wnd_proc as *const () as isize,
        )
    };
    if previous == 0 {
        return Err(format!(
            "SetWindowLongPtrW(GWLP_WNDPROC) failed for {}: {}",
            hwnd.0,
            std::io::Error::last_os_error()
        ));
    }

    previous_wndprocs.insert(hwnd.0, WndProcEntry { previous, mode });
    Ok(())
}

fn uninstall_no_activate_handler_for_hwnd(hwnd: Hwnd) -> Result<(), String> {
    // Restores one HWND's original window procedure when it was subclassed by this app.
    if !hwnd.is_window() {
        return Ok(());
    }

    let previous = {
        let mut previous_wndprocs = PREVIOUS_WNDPROCS
            .lock()
            .map_err(|_| "No-activate wndproc map lock is poisoned.".to_string())?;
        let Some(previous_wndprocs) = previous_wndprocs.as_mut() else {
            return Ok(());
        };

        previous_wndprocs
            .remove(&hwnd.0)
            .map(|entry| entry.previous)
    };

    let Some(previous) = previous else {
        return Ok(());
    };

    let result = unsafe { SetWindowLongPtrW(hwnd.as_raw(), GWLP_WNDPROC, previous) };
    if result == 0 {
        return Err(format!(
            "SetWindowLongPtrW(GWLP_WNDPROC restore) failed for {}: {}",
            hwnd.0,
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
}

unsafe extern "system" fn no_activate_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Refuses mouse activation while preserving ordinary mouse click/drag messages.
    let mode = activation_guard_mode(hwnd);
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }

    if mode == Some(ActivationGuardMode::Strict) {
        match message {
            WM_SETFOCUS | WM_ACTIVATE => return 0,
            WM_NCACTIVATE => return 1,
            _ => {}
        }
    }

    call_previous_window_proc(hwnd, message, wparam, lparam)
}

fn activation_guard_mode(hwnd: HWND) -> Option<ActivationGuardMode> {
    // Looks up how strongly this HWND should reject activation.
    PREVIOUS_WNDPROCS
        .lock()
        .ok()
        .and_then(|previous_wndprocs| {
            previous_wndprocs
                .as_ref()
                .and_then(|previous_wndprocs| previous_wndprocs.get(&(hwnd as isize)).copied())
        })
        .map(|entry| entry.mode)
}

fn call_previous_window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Delegates unhandled messages to the original window procedure.
    let previous = PREVIOUS_WNDPROCS.lock().ok().and_then(|previous_wndprocs| {
        previous_wndprocs.as_ref().and_then(|previous_wndprocs| {
            previous_wndprocs
                .get(&(hwnd as isize))
                .map(|entry| entry.previous)
        })
    });

    let Some(previous) = previous else {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    };

    let previous_proc: WNDPROC = unsafe { std::mem::transmute(previous) };
    unsafe { CallWindowProcW(previous_proc, hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn foreground_event_callback(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object_id: i32,
    _child_id: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    // Restores focus when a guarded dock surface becomes the foreground window.
    if hwnd.is_null() {
        return;
    }

    let hwnd = Hwnd(hwnd as isize);
    if is_guarded_or_child(hwnd) {
        restore_last_regular_foreground();
        return;
    }

    remember_foreground(hwnd);
}

fn remember_current_foreground() {
    // Stores the foreground HWND when it is not one of the guarded dock surfaces.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return;
    }

    let hwnd = Hwnd(hwnd as isize);
    if !is_guarded_or_child(hwnd) {
        remember_foreground(hwnd);
    }
}

fn remember_foreground(hwnd: Hwnd) {
    // Records the user's current keyboard target for later restoration.
    if !hwnd.is_window() {
        return;
    }

    let keyboard_target = capture_keyboard_target(hwnd);
    if let Ok(mut last_keyboard_target) = LAST_KEYBOARD_TARGET.lock() {
        *last_keyboard_target = Some(keyboard_target);
    }
}

fn restore_last_regular_foreground() {
    // Moves keyboard focus back to the last non-dock foreground and focus HWND.
    let previous = LAST_KEYBOARD_TARGET
        .lock()
        .ok()
        .and_then(|last_keyboard_target| *last_keyboard_target);
    let Some(previous) = previous else {
        return;
    };

    let foreground = Hwnd(previous.foreground);
    if !foreground.is_window() || is_guarded_or_child(foreground) {
        return;
    }

    with_attached_input(foreground, || unsafe {
        SetForegroundWindow(foreground.as_raw());
        SetActiveWindow(foreground.as_raw());

        if let Some(focus) = previous.focus {
            let focus = Hwnd(focus);
            if focus.is_window() {
                SetFocus(focus.as_raw());
            }
        }
    });
}

fn capture_keyboard_target(foreground: Hwnd) -> KeyboardTarget {
    // Captures both the foreground top-level HWND and its focused child/control HWND.
    let focus = with_attached_input(foreground, || {
        let focus = unsafe { GetFocus() };
        (!focus.is_null()).then_some(focus as isize)
    });

    KeyboardTarget {
        foreground: foreground.0,
        focus,
    }
}

fn with_attached_input<T>(target: Hwnd, action: impl FnOnce() -> T) -> T {
    // Temporarily joins input queues so GetFocus/SetFocus can address another thread.
    let current_thread_id = unsafe { GetCurrentThreadId() };
    let target_thread_id =
        unsafe { GetWindowThreadProcessId(target.as_raw(), std::ptr::null_mut()) };
    let should_attach = target_thread_id != 0 && target_thread_id != current_thread_id;
    let attached =
        should_attach && unsafe { AttachThreadInput(current_thread_id, target_thread_id, 1) != 0 };

    let result = action();

    if attached {
        unsafe {
            AttachThreadInput(current_thread_id, target_thread_id, 0);
        }
    }

    result
}

fn is_guarded_or_child(hwnd: Hwnd) -> bool {
    // Checks whether a HWND is guarded directly or belongs to a guarded window tree.
    let Ok(guarded_windows) = GUARDED_FOREGROUND_WINDOWS.lock() else {
        return false;
    };
    let Some(guarded_windows) = guarded_windows.as_ref() else {
        return false;
    };

    guarded_windows.iter().any(|guarded_hwnd| {
        let guarded_hwnd = Hwnd(*guarded_hwnd);
        guarded_hwnd.0 == hwnd.0 || unsafe { IsChild(guarded_hwnd.as_raw(), hwnd.as_raw()) != 0 }
    })
}

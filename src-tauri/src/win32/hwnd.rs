use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hwnd(pub isize);

impl Hwnd {
    pub fn new(value: isize) -> Result<Self, String> {
        // Creates a non-null HWND wrapper from a raw pointer-sized value.
        if value == 0 {
            return Err("HWND must not be null.".to_string());
        }

        Ok(Self(value))
    }

    pub fn as_raw(self) -> HWND {
        // Converts the wrapper into the raw Win32 HWND type.
        self.0 as HWND
    }

    pub fn is_window(self) -> bool {
        // Checks whether Windows currently recognizes the HWND.
        unsafe { IsWindow(self.as_raw()) != 0 }
    }
}

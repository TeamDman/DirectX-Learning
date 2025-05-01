use windows::core::Param;
use windows::core::ParamValue;
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::RegisterClassExW;
use windows::Win32::UI::WindowsAndMessaging::WNDCLASSEXW;

use crate::windy_error::MyResult;

/// Equivalent to the MAKEINTATOM macro in C/C++.
/// Converts a class atom into a PCWSTR that can be used
/// for window creation functions.
///
/// https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-makeintatom
#[allow(non_snake_case)]
pub fn MAKEINTOATOM(atom: u16) -> PCWSTR {
    // MAKEINTATOM(atom) casts the atom value to a pointer type.
    // The low-order word is the atom, and the high-order word is zero.
    // In Rust, we can achieve this by casting the atom to a raw pointer
    // type that corresponds to PCWSTR.
    PCWSTR(atom as *const u16)
}

pub struct ClassIdAtom(u16);
impl From<u16> for ClassIdAtom {
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl Param<PCWSTR> for &ClassIdAtom {
    unsafe fn param(self) -> ParamValue<PCWSTR> {
        ParamValue::Owned(MAKEINTOATOM(self.0))
    }
}
pub fn register_window_class(class: &WNDCLASSEXW) -> MyResult<ClassIdAtom> {
    let atom = unsafe { RegisterClassExW(class) };
    std::debug_assert_ne!(atom, 0);
    Ok(ClassIdAtom(atom))
}

use crate::window_class::WindowClass;
use crate::windy_error::MyResult;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn create_window<W: WindowClass>(
    our_module: HMODULE,
    window_rect: RECT,
    title: PCWSTR,
    mut window_data: W::WindowData,
) -> MyResult<HWND> {
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            W::ID,
            title,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            None,                    // no parent window
            None,                    // no menus
            Some(our_module.into()), // Use instance from GetModuleHandleA
            Some(&mut window_data as *mut _ as _),
        )
    }?;
    Ok(hwnd)
}

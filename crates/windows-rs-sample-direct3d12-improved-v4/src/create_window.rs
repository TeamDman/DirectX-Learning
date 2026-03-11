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
            WINDOW_EX_STYLE(WS_EX_LAYERED.0 as u32),
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

    // --- Translucency Change: Set Layered Window Attributes ---
    // Set a dummy color key (0, 0, 0) and use the alpha channel from the window itself.
    // The alpha value (125) is a global opacity. For per-pixel alpha from D3D,
    // we might need to use LWA_ALPHA and set a single alpha value or
    // more advanced techniques with UpdateLayeredWindow.
    // Let's start with LWA_ALPHA and a high value like 255 (opaque)
    // as the per-pixel alpha will come from the D3D render target.
    unsafe {
        // Set the window to use layered window attributes for per-pixel alpha
        // We can use a solid color key (like black) that won't appear in our rendering
        // and LWA_ALPHA with 255. The DWM then uses the per-pixel alpha from the swap chain.
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
    }
    
    Ok(hwnd)
}

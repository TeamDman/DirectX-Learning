use crate::my_behaviour_bind::Resources;
use crate::windy_error::MyResult;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::Graphics::Dxgi::IDXGIFactory4;
use windows::Win32::UI::WindowsAndMessaging::*;

pub trait WindowClass {
    const ID: PCWSTR;

    type WindowData;
    fn handle(&mut self, message: u32, wparam: WPARAM) -> bool;

    /// The name of the window class.

    fn bind_to_window(
        device: &mut ID3D12Device,
        dxgi_factory: &mut IDXGIFactory4,
        hwnd: &HWND,
        window_size: (u32, u32),
    ) -> MyResult<Resources>;

    fn render(
        // device: &mut ID3D12Device,
        // dxgi_factory: &mut IDXGIFactory4,
        resources: &mut Resources,
    ) -> MyResult<()>;
}

pub fn create_window_class_struct<W: WindowClass>(instance: HMODULE) -> MyResult<WNDCLASSEXW> {
    // WNDCLASSEXW - https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-wndclassexw
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc::<W>),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: W::ID,
        ..Default::default()
    };
    Ok(wc)
}

// Wrapper function to handle potential panics in sample_wndproc
pub fn safe_sample_wndproc<W: WindowClass>(
    behaviour: &mut W,
    message: u32,
    wparam: WPARAM,
) -> bool {
    // Use catch_unwind if you need to handle panics gracefully
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sample_wndproc_impl(behaviour, message, wparam)
    }))
    .unwrap_or(false) // Default to false if panic occurs
}

// Original logic moved here
pub fn sample_wndproc_impl<W: WindowClass>(
    behaviour: &mut W,
    message: u32,
    wparam: WPARAM,
) -> bool {
    behaviour.handle(message, wparam)
}

extern "system" fn wndproc<W: WindowClass>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CREATE {
        unsafe {
            let create_struct: &CREATESTRUCTW = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create_struct.lpCreateParams as _);
        }
        return LRESULT(0);
    }

    let user_data = unsafe { GetWindowLongPtrA(window, GWLP_USERDATA) };
    if user_data == 0 {
        // We can get messages before WM_CREATE or after WM_DESTROY.
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }

    let sample = std::ptr::NonNull::<W>::new(user_data as *mut W);

    // Use a scope to ensure the mutable borrow ends before DefWindowProc
    let handled = if let Some(mut s) = sample {
        match message {
            WM_DESTROY => {
                // Don't call on_destroy here, call it explicitly before exiting run_sample
                unsafe { PostQuitMessage(0) };
                true // Mark as handled
            }
            _ => {
                // Use the safe wrapper
                safe_sample_wndproc(unsafe { s.as_mut() }, message, wparam)
            }
        }
    } else {
        false
    };

    if handled {
        LRESULT(0)
    } else {
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }
}

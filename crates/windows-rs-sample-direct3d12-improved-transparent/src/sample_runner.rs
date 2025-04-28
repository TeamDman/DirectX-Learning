use tracing::warn;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::dx_sample::build_command_line;
use crate::dx_sample::DXSample;

/// Runs a DirectX sample that implements the DXSample trait
pub fn run_sample<S>() -> Result<()>
where
    S: DXSample,
{
    // --- Add info_queue variable ---
    let info_queue: Option<IDXGIInfoQueue>;

    // Wrap the initialization part in a block to handle potential errors and print messages
    let (_hwnd, mut sample) = {
        let instance = unsafe { GetModuleHandleA(None)? };

        let wc = WNDCLASSEXA {
            cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc::<S>),
            hInstance: instance.into(),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            lpszClassName: s!("RustWindowClass"),
            hbrBackground: HBRUSH::default(),
            ..Default::default()
        };

        let command_line = build_command_line();
        // --- Capture info_queue from create_device ---
        let (mut sample_local, local_info_queue) = match S::new(&command_line) {
            Ok(result) => result,
            Err(e) => {
                // If S::new fails early, we won't have an info queue yet.
                // Consider adding message printing here if create_device could fail within S::new
                return Err(e);
            }
        };

        // Store the info_queue from the Sample::new result
        info_queue = local_info_queue;

        let size = sample_local.window_size();

        let atom = unsafe { RegisterClassExA(&wc) };
        debug_assert_ne!(atom, 0, "Failed to register window class");

        let mut window_rect = RECT {
            left: 0,
            top: 0,
            right: size.0,
            bottom: size.1,
        };
        unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false)? };

        let mut title = sample_local.title();
        if command_line.use_warp_device {
            title.push_str(" (WARP)");
        }
        title.push('\0');

        let hwnd = unsafe {
            CreateWindowExA(
                WINDOW_EX_STYLE(WS_EX_LAYERED.0 as u32),
                s!("RustWindowClass"),
                PCSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                window_rect.right - window_rect.left,
                window_rect.bottom - window_rect.top,
                None,
                None,
                Some(instance.into()),
                Some(&mut sample_local as *mut _ as _),
            )
        }?;

        // --- Call bind_to_window and print messages on error ---
        if let Err(e) = sample_local.bind_to_window(&hwnd) {
            // *** This is where we print the messages before returning ***
            // We need the info_queue here. Let's assume it was populated during S::new
            // For this example, we'll pass the info_queue from the outer scope
            print_dxgi_debug_messages(&info_queue); // Print messages
            return Err(e); // Return the original error
        }

        unsafe { _ = ShowWindow(hwnd, SW_SHOW) };
        (hwnd, sample_local) // Return hwnd and sample if successful
    };

    // Main loop remains largely the same
    let mut done = false;
    while !done {
        let mut message = MSG::default();
        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                _ = TranslateMessage(&message);
                DispatchMessageA(&message);
            }
            if message.message == WM_QUIT {
                done = true;
            }
        } else if let Err(e) = sample.render() {
            eprintln!("Render error: {:?}", e);
            // --- Optionally print debug messages on render error too ---
            print_dxgi_debug_messages(&info_queue);
            // done = true; // Decide if render error is fatal
        }
    }

    // Call OnDestroy before dropping sample
    sample.on_destroy();
    // --- Optionally print messages one last time before exit ---
    // print_dxgi_debug_messages(&info_queue);

    Ok(())
}

/// Prints DXGI debug messages from the info queue
pub fn print_dxgi_debug_messages(info_queue: &Option<IDXGIInfoQueue>) {
    if let Some(queue) = info_queue {
        warn!("--- DXGI Debug Messages START ---");
        let num_messages = unsafe { queue.GetNumStoredMessages(DXGI_DEBUG_ALL) }; // Use DXGI_DEBUG_ALL GUID

        for i in 0..num_messages {
            let mut message_size: usize = 0;
            // Get the size of the message
            if unsafe { queue.GetMessage(DXGI_DEBUG_ALL, i, None, &mut message_size) }.is_err() {
                warn!("Error getting size for message {}", i);
                continue;
            }

            // Allocate buffer and get the message
            let mut message_buffer: Vec<u8> = vec![0; message_size];
            let p_message: *mut DXGI_INFO_QUEUE_MESSAGE =
                message_buffer.as_mut_ptr() as *mut DXGI_INFO_QUEUE_MESSAGE;

            if unsafe { queue.GetMessage(DXGI_DEBUG_ALL, i, Some(p_message), &mut message_size) }
                .is_ok()
            {
                unsafe {
                    // Convert the C string description to a Rust string
                    let description_slice = std::slice::from_raw_parts(
                        (*p_message).pDescription as *const u8,
                        (*p_message).DescriptionByteLength,
                    );
                    // Use from_utf8_lossy for safety, trim null terminators/whitespace
                    let description = String::from_utf8_lossy(description_slice)
                        .trim()
                        .to_string();

                    let severity = match (*p_message).Severity {
                        DXGI_INFO_QUEUE_MESSAGE_SEVERITY_CORRUPTION => "CORRUPTION",
                        DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR => "ERROR",
                        DXGI_INFO_QUEUE_MESSAGE_SEVERITY_WARNING => "WARNING",
                        DXGI_INFO_QUEUE_MESSAGE_SEVERITY_INFO => "INFO",
                        DXGI_INFO_QUEUE_MESSAGE_SEVERITY_MESSAGE => "MESSAGE",
                        _ => "UNKNOWN",
                    };

                    warn!(
                        "DXGI Debug [{} ID:{}]: {}",
                        severity,
                        (*p_message).ID,
                        description
                    );
                }
            } else {
                warn!("Error getting message data for message {}", i);
            }
        }
        unsafe { queue.ClearStoredMessages(DXGI_DEBUG_ALL) };
        warn!("--- DXGI Debug Messages END ---");
    } else {
        warn!("--- DXGI Info Queue not available ---");
    }
}

// Wrapper function to handle potential panics in sample_wndproc
pub fn safe_sample_wndproc<S: DXSample>(sample: &mut S, message: u32, wparam: WPARAM) -> bool {
    // Direct call for simplicity here:
    sample_wndproc_impl(sample, message, wparam)
}

// Original logic moved here
pub fn sample_wndproc_impl<S: DXSample>(sample: &mut S, message: u32, wparam: WPARAM) -> bool {
    match message {
        WM_KEYDOWN => {
            sample.on_key_down(wparam.0 as u8);
            true
        }
        WM_KEYUP => {
            sample.on_key_up(wparam.0 as u8);
            true
        }
        WM_PAINT => {
            // WM_PAINT is handled by the main loop's render call when idle
            // We still need DefWindowProc to validate the window region
            false // Let DefWindowProc handle painting validation
        }
        // Handle other messages like WM_SIZE if needed for resizing swap chain etc.
        _ => false,
    }
}

extern "system" fn wndproc<S: DXSample>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CREATE {
        unsafe {
            let create_struct: &CREATESTRUCTA = &*(lparam.0 as *const CREATESTRUCTA);
            SetWindowLongPtrA(window, GWLP_USERDATA, create_struct.lpCreateParams as _);
        }
        return LRESULT(0);
    }

    let user_data = unsafe { GetWindowLongPtrA(window, GWLP_USERDATA) };
    if user_data == 0 {
        // We can get messages before WM_CREATE or after WM_DESTROY.
        return unsafe { DefWindowProcA(window, message, wparam, lparam) };
    }

    // Cast user_data to a pointer to our sample type.
    // Use NonNull for safety if desired, but raw pointer is common here.
    let sample_ptr = user_data as *mut S;
    // Safety: We assume user_data is a valid pointer to S managed by run_sample
    let sample = unsafe { &mut *sample_ptr };

    // Use a scope to ensure the mutable borrow ends before DefWindowProc
    let handled = match message {
        WM_DESTROY => {
            // Don't call on_destroy here, call it explicitly before exiting run_sample
            unsafe { PostQuitMessage(0) };
            true // Mark as handled
        }
        _ => {
            // Use the safe wrapper
            safe_sample_wndproc(sample, message, wparam)
        }
    };

    if handled {
        LRESULT(0)
    } else {
        unsafe { DefWindowProcA(window, message, wparam, lparam) }
    }
}

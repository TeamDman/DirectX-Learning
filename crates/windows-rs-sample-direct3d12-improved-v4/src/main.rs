#![feature(maybe_uninit_array_assume_init)]
pub mod create_window;
pub mod my_behaviour;
pub mod my_behaviour_bind;
pub mod my_window_data;
pub mod window_class;
pub mod windy_error;
pub mod windy_window_class_id;

use create_window::create_window;
use my_behaviour::MyBehaviour;
use my_window_data::MyWindowData;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use tracing::info;
use tracing::warn;
use widestring::U16String;
use window_class::create_window_class_struct;
use window_class::WindowClass;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windy_error::MyResult;
use windy_window_class_id::register_window_class;

pub fn main() -> MyResult<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::SubscriberBuilder::default()
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_target(false)
        .init();
    info!("Ahoy, world!");

    let our_module = get_handle_to_file_used_to_create_the_calling_process()?;

    let window_class = create_window_class_struct::<MyBehaviour>(our_module)?;
    register_window_class(&window_class)?;

    let window_size = (1280, 720);
    let mut window_rect = RECT {
        left: 0,
        top: 0,
        right: window_size.0 as i32,
        bottom: window_size.1 as i32,
    };
    // Calculates the required size of the window rectangle, based on the desired size of the client rectangle.
    unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false)? };

    let use_warp_device = false;
    let title = match use_warp_device {
        true => w!("My Rust Window (WARP)"),
        false => w!("My Rust Window"),
    };

    let hwnd =
        create_window::<MyBehaviour>(our_module, window_rect, title, MyWindowData::default())?;

    let (mut dxgi_factory, mut device) = create_device(use_warp_device)?;
    let mut resources =
        MyBehaviour::bind_to_window(&mut device, &mut dxgi_factory, &hwnd, window_size)?;

    unsafe { _ = ShowWindow(hwnd, SW_SHOW) };

    let mut done = false;
    while !done {
        let mut message = MSG::default();

        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                _ = TranslateMessage(&message);
                DispatchMessageA(&message);
            }

            if message.message == WM_QUIT {
                done = true; // Exit loop
            }
        } else {
            // Render when idle, handle potential errors
            if let Err(e) = MyBehaviour::render(&mut resources) {
                std::eprintln!("Render error: {:?}", e);
                // Decide how to handle render errors, maybe break the loop
                // For now, we'll just print and continue
            }
        }
    }
    Ok(())
}

fn rust_string_to_pcwstr(rust_string: &str) -> U16String {
    // Convert &str to OsString (which can handle Windows paths and names)
    let os_string = OsString::from(rust_string);

    // Convert OsString to a Vec of u16 (UTF-16)
    let wide_chars: Vec<u16> = os_string.encode_wide().collect();

    // Create a U16String from the Vec<u16>. This automatically adds the null terminator.
    U16String::from_vec(wide_chars)
}

fn get_handle_to_file_used_to_create_the_calling_process() -> MyResult<HMODULE> {
    let mut out = Default::default();
    unsafe { GetModuleHandleExW(Default::default(), None, &mut out)? };
    Ok(out)
}

fn create_device(use_warp_device: bool) -> Result<(IDXGIFactory4, ID3D12Device)> {
    let mut debug_flags = DXGI_CREATE_FACTORY_FLAGS(0);
    if cfg!(debug_assertions) {
        unsafe {
            let mut debug: Option<ID3D12Debug> = None;
            if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
                debug.EnableDebugLayer();
                debug_flags |= DXGI_CREATE_FACTORY_DEBUG;
                info!("D3D12 Debug Layer Enabled");
            } else {
                warn!("Warning: D3D12 Debug Layer unavailable.");
            }
        }
    }

    let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(debug_flags) }?;

    let adapter = if use_warp_device {
        info!("Using WARP adapter.");
        unsafe { dxgi_factory.EnumWarpAdapter()? }
    } else {
        get_hardware_adapter(&dxgi_factory)?
    };

    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?; // Request 11_0 feature level
    Ok((dxgi_factory, device.unwrap()))
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    for i in 0.. {
        let adapter = unsafe { factory.EnumAdapters1(i)? };
        let desc = unsafe { adapter.GetDesc1()? };

        if (DXGI_ADAPTER_FLAG(desc.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            continue;
        }

        if unsafe {
            D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_11_0, // Use a common feature level
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
        }
        .is_ok()
        {
            println!(
                "Using hardware adapter: {}",
                String::from_utf16_lossy(&desc.Description)
            );
            return Ok(adapter);
        }
    }
    // Should be unreachable if a D3D12 capable device exists
    Err(Error::new(E_FAIL, "No suitable hardware adapter found."))
}

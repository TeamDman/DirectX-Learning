use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::adapter_utils::get_hardware_adapter;
use crate::dx_sample::SampleCommandLine;

// Create D3D12 Device and DXGI Factory
pub fn create_device(
    command_line: &SampleCommandLine,
) -> Result<(IDXGIFactory4, ID3D12Device, Option<IDXGIInfoQueue>)> {
    // Added Option<IDXGIInfoQueue>
    let mut debug_flags = DXGI_CREATE_FACTORY_FLAGS(0);
    let mut info_queue: Option<IDXGIInfoQueue> = None; // Initialize info_queue

    if cfg!(debug_assertions) {
        let mut debug_enabled = false;
        unsafe {
            // Try ID3D12Debug1 first
            let mut debug1: Option<ID3D12Debug1> = None;
            if D3D12GetDebugInterface::<ID3D12Debug1>(&mut debug1).is_ok() {
                let debug1 = debug1.unwrap();
                println!("D3D12 Debug Layer Enabled (ID3D12Debug1 + GBV)");
                debug1.EnableDebugLayer();
                debug1.SetEnableGPUBasedValidation(true);
                debug_flags |= DXGI_CREATE_FACTORY_DEBUG;
                debug_enabled = true;
            } else {
                // Fallback to ID3D12Debug
                let mut debug: Option<ID3D12Debug> = None;
                if D3D12GetDebugInterface::<ID3D12Debug>(&mut debug).is_ok() {
                    let debug = debug.unwrap();
                    println!("D3D12 Debug Layer Enabled (ID3D12Debug)");
                    debug.EnableDebugLayer();
                    debug_flags |= DXGI_CREATE_FACTORY_DEBUG;
                    debug_enabled = true;
                } else {
                    eprintln!("Warning: D3D12 Debug Layer unavailable.");
                }
            }

            // --- If debug was enabled, try to get the Info Queue ---
            if debug_enabled {
                let queue = DXGIGetDebugInterface1::<IDXGIInfoQueue>(0);
                match queue {
                    Ok(q) => {
                        println!("DXGI Info Queue obtained.");
                        // Optional: Set break on severity here if desired
                        // queue.as_ref().unwrap().SetBreakOnSeverity(DXGI_DEBUG_ALL, DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, true);
                        // queue.as_ref().unwrap().SetBreakOnSeverity(DXGI_DEBUG_ALL, DXGI_INFO_QUEUE_MESSAGE_SEVERITY_CORRUPTION, true);
                        info_queue = Some(q);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to get DXGI Info Queue: {:?}", e);
                    }
                }
            }
        }
    }

    // Create DXGI Factory (pass the debug flag if set)
    let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(debug_flags) }?;

    // Select Adapter (remains the same)
    let adapter = if command_line.use_warp_device {
        println!("Using WARP adapter.");
        unsafe { dxgi_factory.EnumWarpAdapter()? }
    } else {
        get_hardware_adapter(&dxgi_factory)?
    };

    // Create Device (remains the same)
    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;

    // Return factory, device, and the info_queue
    Ok((dxgi_factory, device.unwrap(), info_queue))
}

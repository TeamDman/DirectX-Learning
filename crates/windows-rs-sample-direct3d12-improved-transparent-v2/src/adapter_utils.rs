use tracing::warn;
use windows::core::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::D3D12CreateDevice;
use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::Graphics::Dxgi::*;
use tracing::info;

/// Enumerates hardware adapters and returns the first one that supports Direct3D 12.
///
/// This function iterates through all available DXGI adapters, skipping software adapters,
/// and returns the first hardware adapter that supports Direct3D 12 Feature Level 11.0.
///
/// # Arguments
///
/// * `factory` - The DXGI factory to use for enumerating adapters
///
/// # Returns
///
/// * `Ok(IDXGIAdapter1)` - The first compatible hardware adapter
/// * `Err(Error)` - If no suitable adapter is found or another error occurs
pub fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    info!("Enumerating Adapters:");
    for i in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break, // No more adapters
            Err(e) => return Err(e),                             // Other error
        };

        let desc = unsafe { adapter.GetDesc1()? };
        let adapter_name = String::from_utf16_lossy(&desc.Description);
        
        // Skip Software Adapter
        if (DXGI_ADAPTER_FLAG(desc.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            info!("Adapter {}: {} (Software Adapter - Skipping)", i, adapter_name);
            continue;
        }

        // Check for Direct3D 12 support
        let supports_d3d12 = unsafe {
            D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_11_0, // Check for basic D3D12 support
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
        }
        .is_ok();
        
        if supports_d3d12 {
            info!("Adapter {}: {} (Selected)", i, adapter_name);
            return Ok(adapter);
        } else {
            warn!("Adapter {}: {} (Does not support D3D12 Feature Level 11.0)", i, adapter_name);
        }
    }

    Err(Error::new(
        DXGI_ERROR_NOT_FOUND, // Or E_FAIL
        "No suitable D3D12 hardware adapter found.",
    ))
}

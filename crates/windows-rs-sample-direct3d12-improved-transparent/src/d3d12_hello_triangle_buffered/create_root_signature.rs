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

// Create Root Signature
pub fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    // An empty root signature is sufficient for this sample.
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        ..Default::default()
    };

    let mut signature_blob = None;
    let mut error_blob = None;

    let serialize_result = unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1, // Use version 1.0 or 1.1 if needed
            &mut signature_blob,
            Some(&mut error_blob),
        )
    };

    if let Err(e) = serialize_result {
        if let Some(error) = error_blob {
            let error_msg = unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(
                    error.GetBufferPointer() as *const u8,
                    error.GetBufferSize(),
                ))
            };
            eprintln!("Root Signature Serialization Error: {}", error_msg);
        }
        return Err(e);
    }

    let signature_blob = signature_blob.unwrap(); // Safe after check

    // Fix: Create a slice from the blob pointer and size
    let signature_data: &[u8] = unsafe {
        std::slice::from_raw_parts(
            signature_blob.GetBufferPointer() as *const u8,
            signature_blob.GetBufferSize(),
        )
    };

    unsafe {
        device.CreateRootSignature(
            0, // nodeMask
            signature_data,
        )
    }
}

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

use crate::dx_sample::SampleCommandLine;

use super::create_device::create_device;
use super::sample::Sample;

pub fn new(command_line: &SampleCommandLine) -> Result<(Sample, Option<IDXGIInfoQueue>)> {
    // Call the modified create_device
    let (dxgi_factory, device, info_queue) = create_device(command_line)?;

    // Return both the Sample and the Info Queue
    Ok((
        Sample {
            dxgi_factory,
            device,
            resources: None,
            window_size: (1280, 720),
        },
        info_queue,
    ))
}

use windows::core::*;
use windows::Win32::Graphics::Dxgi::*;

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

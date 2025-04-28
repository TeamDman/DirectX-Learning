use crate::d3d12_hello_triangle_buffered::wait_for_gpu_idle::wait_for_gpu_idle;
use crate::dx_sample::DXSample;
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

use super::sample::Sample;

pub fn on_destroy(sample: &mut Sample) {
    if let Some(resources) = &mut sample.resources {
        // Wait for GPU to finish all frames before releasing resources
        if let Err(e) = wait_for_gpu_idle(resources) {
            eprintln!("Error waiting for GPU idle on destroy: {:?}", e);
        }
        // Close the event handle
        unsafe {
            if !resources.fence_event.is_invalid() {
                // Use .ok() to ignore bool return, or handle error
                CloseHandle(resources.fence_event).ok();
            }
        }
    }
    // Resources are dropped automatically when `self.resources` goes out of scope
    println!("Sample destroyed.");
}

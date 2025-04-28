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

use super::resources::Resources;

// Wait for the GPU to finish work up to the fence value for the *current* frame index.
pub fn wait_for_gpu(resources: &mut Resources) -> Result<()> {
    let current_frame_index = resources.frame_index as usize;
    let fence_value_to_wait_for = resources.fence_values[current_frame_index];

    // Schedule a Signal command in the queue.
    unsafe {
        resources
            .command_queue
            .Signal(&resources.fence, fence_value_to_wait_for)?
    };

    // Wait until the fence has been processed.
    if unsafe { resources.fence.GetCompletedValue() } < fence_value_to_wait_for {
        unsafe {
            resources
                .fence
                .SetEventOnCompletion(fence_value_to_wait_for, resources.fence_event)?;
            WaitForSingleObjectEx(resources.fence_event, INFINITE, false); // Use FALSE constant
        }
    }

    // Increment the fence value for the *next* time this frame index is used.
    resources.fence_values[current_frame_index] += 1;

    Ok(())
}

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
// Prepare to render the next frame.
pub fn move_to_next_frame(resources: &mut Resources) -> Result<()> {
    // Schedule a Signal command in the queue for the *current* frame.
    let current_frame_index = resources.frame_index as usize;
    let fence_value_to_signal = resources.fence_values[current_frame_index];
    unsafe {
        resources
            .command_queue
            .Signal(&resources.fence, fence_value_to_signal)?
    };

    // Update the frame index to the next buffer in the swap chain.
    resources.frame_index = unsafe { resources.swap_chain.GetCurrentBackBufferIndex() };
    let next_frame_index = resources.frame_index as usize;

    // Check if the next frame's command allocator is ready (i.e., GPU finished using it).
    // We check against the fence value that *will be* signaled when the work for
    // that frame index *last time* it was used is complete.
    let fence_value_to_check = resources.fence_values[next_frame_index];
    if unsafe { resources.fence.GetCompletedValue() } < fence_value_to_check {
        unsafe {
            resources
                .fence
                .SetEventOnCompletion(fence_value_to_check, resources.fence_event)?;
            WaitForSingleObjectEx(resources.fence_event, INFINITE, false);
        }
    }

    // Set the fence value for the *next* time we render to this frame index.
    // This value will be signaled by the command queue when the commands we are
    // *about* to record for this frame index have finished executing.
    resources.fence_values[next_frame_index] = fence_value_to_signal + 1;

    Ok(())
}

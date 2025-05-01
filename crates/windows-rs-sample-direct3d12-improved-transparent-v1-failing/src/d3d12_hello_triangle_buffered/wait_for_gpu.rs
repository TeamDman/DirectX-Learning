use windows::core::*;
use windows::Win32::System::Threading::*;

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

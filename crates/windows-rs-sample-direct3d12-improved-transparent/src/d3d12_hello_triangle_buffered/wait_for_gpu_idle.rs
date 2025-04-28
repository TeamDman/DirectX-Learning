use windows::core::*;
use windows::Win32::System::Threading::*;

use super::resources::Resources;

// Wait for *all* submitted GPU work to complete. Used before destruction.
pub fn wait_for_gpu_idle(resources: &mut Resources) -> Result<()> {
    // Signal the fence with a value one greater than the last submitted value
    // across all frames. Find the max value submitted.
    let max_fence_value = *resources.fence_values.iter().max().unwrap_or(&0);
    let idle_fence_value = max_fence_value; // Signal with the last submitted value

    unsafe {
        resources
            .command_queue
            .Signal(&resources.fence, idle_fence_value)?
    };

    // Wait until the fence reaches this value.
    if unsafe { resources.fence.GetCompletedValue() } < idle_fence_value {
        unsafe {
            resources
                .fence
                .SetEventOnCompletion(idle_fence_value, resources.fence_event)?;
            WaitForSingleObjectEx(resources.fence_event, INFINITE, false);
        }
    }
    Ok(())
}

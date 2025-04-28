use crate::d3d12_hello_triangle_buffered::wait_for_gpu_idle::wait_for_gpu_idle;
use windows::Win32::Foundation::*;

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

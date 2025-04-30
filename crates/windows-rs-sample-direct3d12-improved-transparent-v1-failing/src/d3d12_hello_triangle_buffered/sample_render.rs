use windows::core::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;

use super::move_to_next_frame::move_to_next_frame;
use super::populate_command_list::populate_command_list;
use super::sample::Sample;

pub fn render(sample: &mut Sample) -> Result<()> {
    if let Some(resources) = &mut sample.resources {
        // Wait for the command allocator for the current frame index to be available.
        // This is handled implicitly by move_to_next_frame before the *next* render,
        // ensuring the fence value check passes.

        // Record commands
        populate_command_list(resources)?;

        // Execute the command list
        let command_lists = [Some(resources.command_list.cast::<ID3D12CommandList>()?)];
        unsafe { resources.command_queue.ExecuteCommandLists(&command_lists) };

        // Present the frame (vsync enabled with interval 1)
        // Use DXGI_PRESENT(0) for flags
        unsafe { resources.swap_chain.Present(1, DXGI_PRESENT(0)) }.ok()?;

        // Prepare for the next frame (synchronization)
        move_to_next_frame(resources)?;
    }
    Ok(())
}

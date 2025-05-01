use windows::core::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;

use super::resources::Resources;
use super::transition_barrier::transition_barrier;

pub fn populate_command_list(resources: &mut Resources) -> Result<()> {
    let command_allocator = &resources.command_allocators[resources.frame_index as usize];
    unsafe { command_allocator.Reset()? };

    let command_list = &resources.command_list;
    unsafe { command_list.Reset(command_allocator, &resources.pso)? };

    // Set necessary state.
    unsafe {
        command_list.SetGraphicsRootSignature(&resources.root_signature);
        command_list.RSSetViewports(&[resources.viewport]);
        command_list.RSSetScissorRects(&[resources.scissor_rect]);
    }

    // Indicate that the back buffer will be used as a render target.
    let barrier_rt = transition_barrier(
        &resources.render_targets[resources.frame_index as usize],
        D3D12_RESOURCE_STATE_PRESENT,
        D3D12_RESOURCE_STATE_RENDER_TARGET,
    );
    unsafe { command_list.ResourceBarrier(&[barrier_rt]) };

    let rtv_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: unsafe { resources.rtv_heap.GetCPUDescriptorHandleForHeapStart() }.ptr
            + (resources.frame_index * resources.rtv_descriptor_size) as usize,
    };

    unsafe { command_list.OMSetRenderTargets(1, Some(&rtv_handle), false, None) };

    // Record commands.
    // --- Translucency Change ---
    // Clear with alpha 0 for a transparent background. RGBA format.
    let clear_color = [0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32];
    // --- End Translucency Change ---
    unsafe {
        command_list.ClearRenderTargetView(rtv_handle, &clear_color, None);
        command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        command_list.IASetVertexBuffers(0, Some(&[resources.vbv]));
        command_list.DrawInstanced(3, 1, 0, 0); // Draw 3 vertices (one triangle)

        // Indicate that the back buffer will now be used to present.
        let barrier_present = transition_barrier(
            &resources.render_targets[resources.frame_index as usize],
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
        );
        command_list.ResourceBarrier(&[barrier_present]);
    }

    unsafe { command_list.Close() }
}

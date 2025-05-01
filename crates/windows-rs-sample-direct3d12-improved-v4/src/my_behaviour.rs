use crate::my_behaviour_bind::Resources;
use crate::my_window_data::MyWindowData;
use crate::window_class::WindowClass;
use crate::windy_error::MyResult;
use std::mem::ManuallyDrop;
use tracing::info;
use windows::core::w;
use windows::core::Interface;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::WPARAM;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::IDXGIFactory4;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct MyBehaviour {}
impl WindowClass for MyBehaviour {
    const ID: PCWSTR = w!("RustWindowClass");

    type WindowData = MyWindowData;
    fn handle(&mut self, message: u32, wparam: WPARAM) -> bool {
        match message {
            WM_KEYDOWN => {
                info!("WM_KEYDOWN: {}", wparam.0 as u8);
                true
            }
            WM_KEYUP => {
                info!("WM_KEYUP: {}", wparam.0 as u8);
                true
            }
            WM_PAINT => {
                // WM_PAINT is handled by the main loop's render call when idle
                // We still need DefWindowProc to validate the window region
                false // Let DefWindowProc handle painting validation
            }
            _ => false,
        }
    }
    fn bind_to_window(
        device: &mut ID3D12Device,
        dxgi_factory: &mut IDXGIFactory4,
        hwnd: &HWND,
        window_size: (u32, u32),
    ) -> MyResult<Resources> {
        Ok(crate::my_behaviour_bind::bind_to_window(
            device,
            dxgi_factory,
            hwnd,
            window_size,
        )?)
    }
    fn render(
        // device: &mut ID3D12Device,
        // dxgi_factory: &mut IDXGIFactory4,
        resources: &mut Resources,
    ) -> MyResult<()> {
        // Return Result
        // Record all the commands we need to render the scene into the command list.
        populate_command_list(resources)?; // Pass mutable ref

        // Execute the command list.
        let command_lists = [Some(resources.command_list.cast::<ID3D12CommandList>()?)];
        unsafe { resources.command_queue.ExecuteCommandLists(&command_lists) };

        // Present the frame.
        // First argument is sync interval. Setting to 0 disables vsync.
        // Second argument is present flags. DXGI_PRESENT_ALLOW_TEARING requires specific setup.
        unsafe { resources.swap_chain.Present(1, DXGI_PRESENT::default()) }.ok()?;

        // Prepare the next frame
        move_to_next_frame(resources)?; // Use the new synchronization function
        Ok(())
    }
}

fn populate_command_list(resources: &mut Resources) -> MyResult<()> {
    // Needs mutable resources
    // Command list allocators can only be reset when the associated
    // command lists have finished execution on the GPU. Fences are used
    // to determine GPU execution progress. `move_to_next_frame` ensures
    // the allocator for the current frame_index is ready.
    let command_allocator = &resources.command_allocators[resources.frame_index as usize];
    unsafe { command_allocator.Reset()? };

    let command_list = &resources.command_list;

    // However, when ExecuteCommandList() is called on a particular
    // command list, that command list can then be reset at any time and
    // must be before re-recording.
    unsafe {
        command_list.Reset(command_allocator, &resources.pso)?;
    }

    // Set necessary state.
    unsafe {
        command_list.SetGraphicsRootSignature(&resources.root_signature);
        command_list.RSSetViewports(&[resources.viewport]);
        command_list.RSSetScissorRects(&[resources.scissor_rect]);
    }

    // Indicate that the back buffer will be used as a render target.
    let barrier = transition_barrier(
        &resources.render_targets[resources.frame_index as usize],
        D3D12_RESOURCE_STATE_PRESENT,
        D3D12_RESOURCE_STATE_RENDER_TARGET,
    );
    unsafe { command_list.ResourceBarrier(&[barrier]) };

    let rtv_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: unsafe { resources.rtv_heap.GetCPUDescriptorHandleForHeapStart() }.ptr
            + (resources.frame_index * resources.rtv_descriptor_size) as usize,
    };

    unsafe { command_list.OMSetRenderTargets(1, Some(&rtv_handle), false, None) };

    // Record commands.
    let clear_color = [0.0_f32, 0.2_f32, 0.4_f32, 1.0_f32]; // RGBA
    unsafe {
        command_list.ClearRenderTargetView(rtv_handle, &clear_color, None); // Use array directly
        command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        command_list.IASetVertexBuffers(0, Some(&[resources.vbv]));
        command_list.DrawInstanced(3, 1, 0, 0);

        // Indicate that the back buffer will now be used to present.
        command_list.ResourceBarrier(&[transition_barrier(
            &resources.render_targets[resources.frame_index as usize],
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
        )]);
    }

    unsafe { command_list.Close() }?;
    Ok(())
}

// Prepare to render the next frame.
fn move_to_next_frame(resources: &mut Resources) -> MyResult<()> {
    // Schedule a Signal command in the queue for the *current* frame.
    let current_fence_value = resources.fence_values[resources.frame_index as usize];
    unsafe {
        resources
            .command_queue
            .Signal(&resources.fence, current_fence_value)?
    };

    // Update the frame index to the next frame.
    resources.frame_index = unsafe { resources.swap_chain.GetCurrentBackBufferIndex() };

    // Check if the next frame is ready to be rendered to yet.
    // We need to wait if the GPU hasn't finished processing the commands
    // associated with this frame index yet (indicated by its fence value).
    if unsafe { resources.fence.GetCompletedValue() }
        < resources.fence_values[resources.frame_index as usize]
    {
        unsafe {
            resources.fence.SetEventOnCompletion(
                resources.fence_values[resources.frame_index as usize],
                resources.fence_event,
            )?;
            // Consider adding a timeout
            WaitForSingleObjectEx(resources.fence_event, INFINITE, false);
        }
    }

    // Set the fence value for the *next* frame (which is now the current frame index).
    // This value will be signaled by the command queue when the commands for this frame
    // (which we are about to record) have finished executing on the GPU.
    resources.fence_values[resources.frame_index as usize] = current_fence_value + 1;

    Ok(())
}

fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            // Use ManuallyDrop to prevent premature Drop of the inner union field
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { std::mem::transmute_copy(resource) }, // Clone resource pointer
                StateBefore: state_before,
                StateAfter: state_after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
    }
}

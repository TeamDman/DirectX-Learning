use std::mem::MaybeUninit;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::create_pipeline_state::create_pipeline_state;
use super::create_root_signature::create_root_signature;
use super::create_vertex_buffer::create_vertex_buffer;
use super::resources::Resources;
use super::sample::Sample;
use super::MaybeUninitHelper;
use super::FRAME_COUNT;

pub fn bind_to_window(sample: &mut Sample, hwnd: &HWND) -> Result<()> {
    let command_queue: ID3D12CommandQueue = unsafe {
        sample
            .device
            .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            })?
    };

    let (width, height) = sample.window_size; // Use stored size

    let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
        BufferCount: FRAME_COUNT,
        Width: width as u32,
        Height: height as u32,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM, // Format supports alpha
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        // --- Translucency Change ---
        // Tell DXGI/DWM the swap chain uses premultiplied alpha for composition.
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        // --- End Translucency Change ---
        ..Default::default()
    };

    // Create swap chain using IDXGIFactory2 for CreateSwapChainForComposition
    let factory2: IDXGIFactory2 = sample.dxgi_factory.cast()?; // Cast should succeed
    let swap_chain_base: IDXGISwapChain1 = unsafe {
        factory2.CreateSwapChainForComposition(
            &command_queue,
            &swap_chain_desc,
            None, // No restrict to output
        )?
    };
    let swap_chain: IDXGISwapChain3 = swap_chain_base.cast()?;

    // --- Removed GDI Layered Window Transparency ---
    // Remove the WS_EX_LAYERED style and SetLayeredWindowAttributes call.
    // DWM composition with DXGI_ALPHA_MODE_PREMULTIPLIED handles the transparency.

    // Prevent automatic Alt+Enter fullscreen transitions
    unsafe {
        sample
            .dxgi_factory
            .MakeWindowAssociation(*hwnd, DXGI_MWA_NO_ALT_ENTER)?;
    }

    let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() };

    // Create RTV descriptor heap
    let rtv_heap: ID3D12DescriptorHeap = unsafe {
        sample
            .device
            .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                NumDescriptors: FRAME_COUNT,
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, // Ensure flags are set
                ..Default::default()
            })
    }?;

    let rtv_descriptor_size = unsafe {
        sample
            .device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
    };
    let rtv_handle = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

    // Create RTVs for each swap chain buffer
    let mut render_targets: [MaybeUninit<ID3D12Resource>; FRAME_COUNT as usize] =
        MaybeUninit::uninit_array();
    for i in 0..FRAME_COUNT {
        let resource: ID3D12Resource = unsafe { swap_chain.GetBuffer(i)? };
        let current_rtv_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: rtv_handle.ptr + (i * rtv_descriptor_size) as usize,
        };
        unsafe {
            sample
                .device
                .CreateRenderTargetView(&resource, None, current_rtv_handle);
        }
        render_targets[i as usize].write(resource);
    }
    // Safety: All elements initialized in the loop above
    let render_targets = unsafe { MaybeUninit::array_assume_init(render_targets) };

    // --- Frame Buffering Resources ---
    let mut command_allocators: [MaybeUninit<ID3D12CommandAllocator>; FRAME_COUNT as usize] =
        MaybeUninit::uninit_array();
    for i in 0..FRAME_COUNT {
        command_allocators[i as usize].write(unsafe {
            sample
                .device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?
        });
    }
    // Safety: All elements initialized in the loop above
    let command_allocators = unsafe { MaybeUninit::array_assume_init(command_allocators) };

    let fence = unsafe { sample.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
    // let fence_values = [1u64; FRAME_COUNT as usize]; // Start fence values at 1

    // Fix: Initialize fence values to 0, matching C++ sample
    let fence_values = [0u64; FRAME_COUNT as usize];

    let fence_event = unsafe { CreateEventA(None, false, false, None)? };
    if fence_event.is_invalid() {
        return Err(Error::from_win32());
    }
    // --- End Frame Buffering Resources ---

    // Viewport and Scissor Rect
    let viewport = D3D12_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: width as f32,
        Height: height as f32,
        MinDepth: D3D12_MIN_DEPTH,
        MaxDepth: D3D12_MAX_DEPTH,
    };
    let scissor_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };

    // Core graphics objects
    let root_signature = create_root_signature(&sample.device)?;
    let pso = create_pipeline_state(&sample.device, &root_signature)?;

    // Create the command list using the first allocator.
    let command_list: ID3D12GraphicsCommandList = unsafe {
        sample.device.CreateCommandList(
            0, // nodeMask
            D3D12_COMMAND_LIST_TYPE_DIRECT,
            &command_allocators[frame_index as usize], // Use current frame's allocator
            &pso,                                      // Initial PSO
        )
    }?;
    // Close command list initially
    unsafe { command_list.Close()? };

    // Vertex Buffer
    let aspect_ratio = width as f32 / height as f32;
    let (vertex_buffer, vbv) = create_vertex_buffer(&sample.device, aspect_ratio)?;

    // Store resources
    let mut resources = Resources {
        command_queue,
        swap_chain,
        frame_index,
        render_targets,
        rtv_heap,
        rtv_descriptor_size,
        viewport,
        scissor_rect,
        command_allocators,
        root_signature,
        pso,
        command_list,
        vertex_buffer,
        vbv,
        fence,
        fence_values,
        fence_event,
    };

    // Initial GPU synchronization
    // Note: The C++ sample calls WaitForGpu here, which signals and increments
    // the *initial* fence value (fence_values[0]). Let's replicate that.
    // We need to signal with the *current* value (0) and then increment it.
    let initial_fence_value = resources.fence_values[resources.frame_index as usize];
    unsafe {
        resources
            .command_queue
            .Signal(&resources.fence, initial_fence_value)?;
        // Wait only if needed (likely not for value 0)
        if resources.fence.GetCompletedValue() < initial_fence_value {
            resources
                .fence
                .SetEventOnCompletion(initial_fence_value, resources.fence_event)?;
            WaitForSingleObjectEx(resources.fence_event, INFINITE, false);
        }
        // Increment the fence value for the current frame index *after* signaling/waiting.
        resources.fence_values[resources.frame_index as usize] += 1;
    }

    sample.resources = Some(resources);

    Ok(())
}

#![feature(maybe_uninit_array_assume_init)]

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

// Import the DXSample trait and related items from the new module
mod dx_sample;
use dx_sample::build_command_line;
use dx_sample::DXSample;
use dx_sample::SampleCommandLine;

// Import the sample runner module
mod sample_runner;
use sample_runner::run_sample;

// Removed BOOL helper, use TRUE/FALSE directly

// Removed DXSample trait definition - now in dx_sample.rs

// Removed SampleCommandLine struct and build_command_line function - now in dx_sample.rs

// Removed run_sample function - now in sample_runner.rs

// Removed print_dxgi_debug_messages function - now in sample_runner.rs

// Removed safe_sample_wndproc function - now in sample_runner.rs

// Removed sample_wndproc_impl function - now in sample_runner.rs

// Removed wndproc function - now in sample_runner.rs

fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    println!("Enumerating Adapters:");
    for i in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break, // No more adapters
            Err(e) => return Err(e.into()),                      // Other error
        };

        let desc = unsafe { adapter.GetDesc1()? };
        let adapter_name = String::from_utf16_lossy(&desc.Description);
        print!("  Adapter {}: {} ", i, adapter_name);

        // Skip Software Adapter
        if (DXGI_ADAPTER_FLAG(desc.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            println!("(Software Adapter - Skipping)");
            continue;
        }

        // Check for Direct3D 12 support
        if unsafe {
            D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_11_0, // Check for basic D3D12 support
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
        }
        .is_ok()
        {
            println!("(Selected)");
            return Ok(adapter);
        } else {
            println!("(Does not support D3D12 Feature Level 11.0)");
        }
    }

    Err(Error::new(
        DXGI_ERROR_NOT_FOUND, // Or E_FAIL
        "No suitable D3D12 hardware adapter found.",
    ))
}

mod d3d12_hello_triangle_buffered {
    use windows::Win32::Graphics::Dxgi::DXGIGetDebugInterface1;

    // Renamed module
    use super::*;
    use std::mem::ManuallyDrop;
    use std::mem::MaybeUninit; // Added MaybeUninit

    const FRAME_COUNT: u32 = 2; // Use 2 for basic buffering, 3 for potentially smoother results

    pub struct Sample {
        dxgi_factory: IDXGIFactory4,
        device: ID3D12Device,
        resources: Option<Resources>,
        window_size: (i32, i32), // Store window size
    }

    struct Resources {
        command_queue: ID3D12CommandQueue,
        swap_chain: IDXGISwapChain3,
        frame_index: u32,
        render_targets: [ID3D12Resource; FRAME_COUNT as usize],
        rtv_heap: ID3D12DescriptorHeap,
        rtv_descriptor_size: u32, // Changed to u32 to match API
        viewport: D3D12_VIEWPORT,
        scissor_rect: RECT,
        // --- Frame Buffering Changes ---
        command_allocators: [ID3D12CommandAllocator; FRAME_COUNT as usize],
        fence: ID3D12Fence,
        fence_values: [u64; FRAME_COUNT as usize], // Fence value for each frame
        fence_event: HANDLE,
        // --- End Frame Buffering Changes ---
        root_signature: ID3D12RootSignature,
        pso: ID3D12PipelineState,
        command_list: ID3D12GraphicsCommandList,
        vertex_buffer: ID3D12Resource, // Keep vertex buffer handle
        vbv: D3D12_VERTEX_BUFFER_VIEW,
    }

    impl DXSample for Sample {
        fn new(command_line: &SampleCommandLine) -> Result<(Self, Option<IDXGIInfoQueue>)> {
            // Call the modified create_device
            let (dxgi_factory, device, info_queue) = create_device(command_line)?;

            // Return both the Sample and the Info Queue
            Ok((
                Sample {
                    dxgi_factory,
                    device,
                    resources: None,
                    window_size: (1280, 720),
                },
                info_queue,
            ))
        }

        fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
            let command_queue: ID3D12CommandQueue = unsafe {
                self.device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                    Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                    ..Default::default()
                })?
            };

            let (width, height) = self.window_size; // Use stored size

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

            // Create swap chain using IDXGIFactory2 for CreateSwapChainForHwnd
            let factory2: IDXGIFactory2 = self.dxgi_factory.cast()?; // Cast should succeed
            let swap_chain_base: IDXGISwapChain1 = unsafe {
                factory2.CreateSwapChainForHwnd(
                    &command_queue,
                    *hwnd,
                    &swap_chain_desc,
                    None, // No fullscreen desc
                    None, // No restrict to output
                )?
            };
            let swap_chain: IDXGISwapChain3 = swap_chain_base.cast()?;

            // Prevent automatic Alt+Enter fullscreen transitions
            unsafe {
                self.dxgi_factory
                    .MakeWindowAssociation(*hwnd, DXGI_MWA_NO_ALT_ENTER)?;
            }

            let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() };

            // Create RTV descriptor heap
            let rtv_heap: ID3D12DescriptorHeap = unsafe {
                self.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        NumDescriptors: FRAME_COUNT,
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                        Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, // Ensure flags are set
                        ..Default::default()
                    })
            }?;

            let rtv_descriptor_size = unsafe {
                self.device
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
                    self.device
                        .CreateRenderTargetView(&resource, None, current_rtv_handle);
                }
                render_targets[i as usize].write(resource);
            }
            // Safety: All elements initialized in the loop above
            let render_targets = unsafe { MaybeUninit::array_assume_init(render_targets) };

            // --- Frame Buffering Resources ---
            let mut command_allocators: [MaybeUninit<ID3D12CommandAllocator>;
                FRAME_COUNT as usize] = MaybeUninit::uninit_array();
            for i in 0..FRAME_COUNT {
                command_allocators[i as usize].write(unsafe {
                    self.device
                        .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?
                });
            }
            // Safety: All elements initialized in the loop above
            let command_allocators = unsafe { MaybeUninit::array_assume_init(command_allocators) };

            let fence = unsafe { self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
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
            let root_signature = create_root_signature(&self.device)?;
            let pso = create_pipeline_state(&self.device, &root_signature)?;

            // Create the command list using the first allocator.
            let command_list: ID3D12GraphicsCommandList = unsafe {
                self.device.CreateCommandList(
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
            let (vertex_buffer, vbv) = create_vertex_buffer(&self.device, aspect_ratio)?;

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

            // // Initial GPU synchronization (wait for setup)
            // wait_for_gpu(&mut resources)?;

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

            self.resources = Some(resources);

            Ok(())
        }

        fn title(&self) -> String {
            "D3D12 Transparent Triangle (Frame Buffered)".into() // Updated title
        }

        fn window_size(&self) -> (i32, i32) {
            self.window_size
        }

        fn render(&mut self) -> Result<()> {
            if let Some(resources) = &mut self.resources {
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

        fn on_destroy(&mut self) {
            if let Some(resources) = &mut self.resources {
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
    }

    // --- Frame Buffering Synchronization ---

    // Wait for the GPU to finish work up to the fence value for the *current* frame index.
    fn wait_for_gpu(resources: &mut Resources) -> Result<()> {
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

    // Wait for *all* submitted GPU work to complete. Used before destruction.
    fn wait_for_gpu_idle(resources: &mut Resources) -> Result<()> {
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

    // Prepare to render the next frame.
    fn move_to_next_frame(resources: &mut Resources) -> Result<()> {
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

    // --- End Frame Buffering Synchronization ---

    // --- Rendering Logic ---

    fn populate_command_list(resources: &mut Resources) -> Result<()> {
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

    // --- Resource Creation Helpers ---

    // Helper for resource transitions
    fn transition_barrier(
        resource: &ID3D12Resource,
        state_before: D3D12_RESOURCE_STATES,
        state_after: D3D12_RESOURCE_STATES,
    ) -> D3D12_RESOURCE_BARRIER {
        D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: unsafe { std::mem::transmute_copy(resource) },
                    StateBefore: state_before,
                    StateAfter: state_after,
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                }),
            },
        }
    }

    // Create D3D12 Device and DXGI Factory
    fn create_device(
        command_line: &SampleCommandLine,
    ) -> Result<(IDXGIFactory4, ID3D12Device, Option<IDXGIInfoQueue>)> {
        // Added Option<IDXGIInfoQueue>
        let mut debug_flags = DXGI_CREATE_FACTORY_FLAGS(0);
        let mut info_queue: Option<IDXGIInfoQueue> = None; // Initialize info_queue

        if cfg!(debug_assertions) {
            let mut debug_enabled = false;
            unsafe {
                // Try ID3D12Debug1 first
                let mut debug1: Option<ID3D12Debug1> = None;
                if D3D12GetDebugInterface::<ID3D12Debug1>(&mut debug1).is_ok() {
                    let debug1 = debug1.unwrap();
                    println!("D3D12 Debug Layer Enabled (ID3D12Debug1 + GBV)");
                    debug1.EnableDebugLayer();
                    debug1.SetEnableGPUBasedValidation(true);
                    debug_flags |= DXGI_CREATE_FACTORY_DEBUG;
                    debug_enabled = true;
                } else {
                    // Fallback to ID3D12Debug
                    let mut debug: Option<ID3D12Debug> = None;
                    if D3D12GetDebugInterface::<ID3D12Debug>(&mut debug).is_ok() {
                        let debug = debug.unwrap();
                        println!("D3D12 Debug Layer Enabled (ID3D12Debug)");
                        debug.EnableDebugLayer();
                        debug_flags |= DXGI_CREATE_FACTORY_DEBUG;
                        debug_enabled = true;
                    } else {
                        eprintln!("Warning: D3D12 Debug Layer unavailable.");
                    }
                }

                // --- If debug was enabled, try to get the Info Queue ---
                if debug_enabled {
                    let queue = DXGIGetDebugInterface1::<IDXGIInfoQueue>(0);
                    match queue {
                        Ok(q) => {
                            println!("DXGI Info Queue obtained.");
                            // Optional: Set break on severity here if desired
                            // queue.as_ref().unwrap().SetBreakOnSeverity(DXGI_DEBUG_ALL, DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, true);
                            // queue.as_ref().unwrap().SetBreakOnSeverity(DXGI_DEBUG_ALL, DXGI_INFO_QUEUE_MESSAGE_SEVERITY_CORRUPTION, true);
                            info_queue = Some(q);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to get DXGI Info Queue: {:?}", e);
                        }
                    }
                }
            }
        }

        // Create DXGI Factory (pass the debug flag if set)
        let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(debug_flags) }?;

        // Select Adapter (remains the same)
        let adapter = if command_line.use_warp_device {
            println!("Using WARP adapter.");
            unsafe { dxgi_factory.EnumWarpAdapter()? }
        } else {
            get_hardware_adapter(&dxgi_factory)?
        };

        // Create Device (remains the same)
        let mut device: Option<ID3D12Device> = None;
        unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;

        // Return factory, device, and the info_queue
        Ok((dxgi_factory, device.unwrap(), info_queue))
    }
    // Create Root Signature
    fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
        // An empty root signature is sufficient for this sample.
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            ..Default::default()
        };

        let mut signature_blob = None;
        let mut error_blob = None;

        let serialize_result = unsafe {
            D3D12SerializeRootSignature(
                &desc,
                D3D_ROOT_SIGNATURE_VERSION_1, // Use version 1.0 or 1.1 if needed
                &mut signature_blob,
                Some(&mut error_blob),
            )
        };

        if let Err(e) = serialize_result {
            if let Some(error) = error_blob {
                let error_msg = unsafe {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        error.GetBufferPointer() as *const u8,
                        error.GetBufferSize(),
                    ))
                };
                eprintln!("Root Signature Serialization Error: {}", error_msg);
            }
            return Err(e);
        }

        let signature_blob = signature_blob.unwrap(); // Safe after check

        // Fix: Create a slice from the blob pointer and size
        let signature_data: &[u8] = unsafe {
            std::slice::from_raw_parts(
                signature_blob.GetBufferPointer() as *const u8,
                signature_blob.GetBufferSize(),
            )
        };

        unsafe {
            device.CreateRootSignature(
                0, // nodeMask
                signature_data,
            )
        }
    }

    // Create Pipeline State Object (PSO)
    fn create_pipeline_state(
        device: &ID3D12Device,
        root_signature: &ID3D12RootSignature,
    ) -> Result<ID3D12PipelineState> {
        let compile_flags = if cfg!(debug_assertions) {
            D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION
        } else {
            0 // No flags for release
        };

        // Find shaders.hlsl relative to executable
        let exe_path = std::env::current_exe().expect("Failed to get executable path");
        let asset_path = exe_path
            .parent()
            .expect("Failed to get executable directory");
        let mut shaders_hlsl_path = asset_path.join("shaders.hlsl");

        if !shaders_hlsl_path.exists() {
            // Attempt to find in src directory as fallback (useful during development)
            let fallback_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("shaders.hlsl");
            if fallback_path.exists() {
                eprintln!(
                    "Warning: shaders.hlsl not found next to executable, using src/shaders.hlsl"
                );
                shaders_hlsl_path = fallback_path;
            } else {
                panic!(
                    "shaders.hlsl not found next to executable ({:?}) or in src/",
                    asset_path.join("shaders.hlsl")
                );
            }
        }

        let shaders_hlsl: HSTRING = shaders_hlsl_path.to_str().unwrap().into();

        // Compile shaders
        let vs_entry = s!("VSMain");
        let ps_entry = s!("PSMain");
        let vs_target = s!("vs_5_0");
        let ps_target = s!("ps_5_0");

        let vertex_shader = compile_shader(&shaders_hlsl, vs_entry, vs_target, compile_flags)?;
        let pixel_shader = compile_shader(&shaders_hlsl, ps_entry, ps_target, compile_flags)?;

        // Define vertex input layout
        let input_element_descs: [D3D12_INPUT_ELEMENT_DESC; 2] = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"),
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("COLOR"),
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT, // Includes alpha
                AlignedByteOffset: 12, // Offset after position (3 floats * 4 bytes/float)
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
        ];

        // Describe PSO
        let pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: unsafe { std::mem::transmute_copy(root_signature) },
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { vertex_shader.GetBufferPointer() },
                BytecodeLength: unsafe { vertex_shader.GetBufferSize() },
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { pixel_shader.GetBufferPointer() },
                BytecodeLength: unsafe { pixel_shader.GetBufferSize() },
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_element_descs.as_ptr(),
                NumElements: input_element_descs.len() as u32,
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE, // Draw back faces too
                ..Default::default()
            },
            // --- Translucency Change ---
            // Explicitly define blend state. Default disables blending but allows alpha write.
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: FALSE,  // No multisampling
                IndependentBlendEnable: FALSE, // Same blend for all RTs (only have 1)
                RenderTarget: [
                    D3D12_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: FALSE, // Blending is OFF (triangle is opaque)
                        LogicOpEnable: FALSE,
                        // The rest are ignored if BlendEnable is FALSE, but set defaults:
                        SrcBlend: D3D12_BLEND_ONE,
                        DestBlend: D3D12_BLEND_ZERO,
                        BlendOp: D3D12_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D12_BLEND_ONE,
                        DestBlendAlpha: D3D12_BLEND_ZERO,
                        BlendOpAlpha: D3D12_BLEND_OP_ADD,
                        LogicOp: D3D12_LOGIC_OP_NOOP,
                        // Ensure alpha channel from PS is written to the RT
                        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    };
                    8 // Initialize all 8 render target blend descs
                ],
            },
            // --- End Translucency Change ---
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: FALSE, // No depth buffer
                StencilEnable: FALSE,
                ..Default::default()
            },
            SampleMask: u32::MAX,
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: [
                DXGI_FORMAT_R8G8B8A8_UNORM, // Format of our render target
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
            ],
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            }, // No MSAA
            ..Default::default()
        };

        // Create PSO
        unsafe { device.CreateGraphicsPipelineState(&pso_desc) }
    }

    // Helper to compile shaders
    fn compile_shader(
        hlsl_path: &HSTRING,
        entry_point: PCSTR,
        target: PCSTR,
        flags: u32,
    ) -> Result<ID3DBlob> {
        let mut shader_blob = None;
        let mut error_blob = None;
        let result = unsafe {
            D3DCompileFromFile(
                hlsl_path,
                None, // Defines
                None, // Include handler
                entry_point,
                target,
                flags,
                0, // Effect flags
                &mut shader_blob,
                Some(&mut error_blob),
            )
        };

        if let Err(e) = result {
            if let Some(error) = error_blob {
                let error_msg = unsafe {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        error.GetBufferPointer() as *const u8,
                        error.GetBufferSize(),
                    ))
                };
                // Use from_utf8_lossy for safe display of potentially non-UTF8 PCSTR
                let entry_point_str = unsafe { String::from_utf8_lossy(entry_point.as_bytes()) };
                let target_str = unsafe { String::from_utf8_lossy(target.as_bytes()) };
                eprintln!(
                    "Shader Compile Error ({} {}): {}",
                    entry_point_str, target_str, error_msg
                );
            }
            Err(e)
        } else {
            Ok(shader_blob.unwrap()) // Safe on success
        }
    }

    // Create Vertex Buffer
    fn create_vertex_buffer(
        device: &ID3D12Device,
        aspect_ratio: f32,
    ) -> Result<(ID3D12Resource, D3D12_VERTEX_BUFFER_VIEW)> {
        #[repr(C)]
        struct Vertex {
            position: [f32; 3], // x, y, z
            color: [f32; 4],    // r, g, b, a
        }

        // Define triangle vertices with opaque alpha (1.0)
        let vertices = [
            Vertex {
                position: [0.0, 0.25 * aspect_ratio, 0.0],
                color: [1.0, 0.0, 0.0, 1.0], // Red, Opaque
            },
            Vertex {
                position: [0.25, -0.25 * aspect_ratio, 0.0],
                color: [0.0, 1.0, 0.0, 1.0], // Green, Opaque
            },
            Vertex {
                position: [-0.25, -0.25 * aspect_ratio, 0.0],
                color: [0.0, 0.0, 1.0, 1.0], // Blue, Opaque
            },
        ];
        let vertex_buffer_size = std::mem::size_of_val(&vertices) as u64;

        // Create upload heap resource for vertex buffer (simple approach)
        let heap_props = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_UPLOAD,
            ..Default::default()
        };

        // Fix: Manually construct D3D12_RESOURCE_DESC for a buffer
        let resource_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0, // Default alignment
            Width: vertex_buffer_size,
            Height: 1,                   // Required for buffers
            DepthOrArraySize: 1,         // Required for buffers
            MipLevels: 1,                // Required for buffers
            Format: DXGI_FORMAT_UNKNOWN, // Required for buffers
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            }, // Required for buffers
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, // Required for buffers
            Flags: D3D12_RESOURCE_FLAG_NONE,
        };

        let mut vertex_buffer: Option<ID3D12Resource> = None;
        unsafe {
            device.CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &resource_desc,
                D3D12_RESOURCE_STATE_GENERIC_READ,
                None, // No optimized clear value
                &mut vertex_buffer,
            )?
        };
        let vertex_buffer = vertex_buffer.unwrap();
        unsafe { vertex_buffer.SetName(w!("VertexBuffer")) }.ok(); // Assign name for debugging

        // Map, copy data, unmap
        unsafe {
            let mut data_ptr = std::ptr::null_mut();
            let read_range = D3D12_RANGE { Begin: 0, End: 0 }; // We do not intend to read
            vertex_buffer.Map(0, Some(&read_range), Some(&mut data_ptr))?;
            std::ptr::copy_nonoverlapping(
                vertices.as_ptr(),
                data_ptr as *mut Vertex,
                vertices.len(),
            );
            vertex_buffer.Unmap(0, None); // Null range indicates potential write to whole buffer
        }

        // Create vertex buffer view
        let vbv = D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
            StrideInBytes: std::mem::size_of::<Vertex>() as u32,
            SizeInBytes: vertex_buffer_size as u32,
        };

        Ok((vertex_buffer, vbv))
    }

    // --- std::mem::MaybeUninit helpers ---
    trait MaybeUninitHelper<T, const N: usize> {
        fn uninit_array() -> [MaybeUninit<T>; N];
        unsafe fn array_assume_init(array: [MaybeUninit<T>; N]) -> [T; N];
    }

    impl<T, const N: usize> MaybeUninitHelper<T, N> for MaybeUninit<T> {
        #[inline]
        fn uninit_array() -> [MaybeUninit<T>; N] {
            // Safety: An uninitialized `[MaybeUninit<_>; N]` is valid.
            unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }
        }

        #[inline]
        unsafe fn array_assume_init(array: [MaybeUninit<T>; N]) -> [T; N] {
            // Safety: The caller guarantees that all elements of the array are initialized.
            // Transmute the array pointer. Requires careful handling of memory layout.
            let ptr = &array as *const _ as *const [T; N];
            ptr.read()
            // Note: This assumes the layout of `[MaybeUninit<T>; N]` is identical to `[T; N]`,
            // which is guaranteed by the language.
        }
    }
} // end mod d3d12_hello_triangle_buffered

fn main() -> Result<()> {
    println!("Starting D3D12 Transparent Triangle Sample...");
    if let Err(e) = run_sample::<d3d12_hello_triangle_buffered::Sample>() {
        // Error already printed by run_sample or the print_dxgi_debug_messages helper
        eprintln!("Sample execution failed: {:?}", e);
        // The debug messages should have been printed just before this.
        return Err(e); // Propagate error for exit code
    }
    println!("Sample finished successfully.");
    Ok(())
}

// Ensure you have a `shaders.hlsl` file next to your executable with:
/*
struct PSInput
{
    float4 position : SV_POSITION; // Clip space position
    float4 color    : COLOR;       // Vertex color + alpha passed from VS
};

// Vertex Shader: Passes position and color through
PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
{
    PSInput result;

    // In this simple case, the input position is already in clip space (-1 to 1)
    // For 3D rendering, you would multiply by Model-View-Projection matrices here.
    result.position = position;

    // Pass the vertex color (including alpha) to the Pixel Shader
    result.color = color;

    return result;
}

// Pixel Shader: Returns the interpolated color (including alpha)
float4 PSMain(PSInput input) : SV_TARGET
{
    // The 'input.color' contains the interpolated vertex color.
    // The alpha channel (input.color.a) determines the opacity.
    // Since our vertices have alpha 1.0, the triangle will be opaque.
    // The final pixel color written to the render target includes this alpha.
    return input.color;
}
*/

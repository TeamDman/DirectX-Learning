use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

trait DXSample {
    fn new(command_line: &SampleCommandLine) -> Result<Self>
    where
        Self: Sized;

    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()>;
    fn on_destroy(&mut self); // Added for cleanup synchronization

    fn update(&mut self) {}
    fn render(&mut self) -> Result<()> {
        // Changed to return Result
        Ok(())
    }
    fn on_key_up(&mut self, _key: u8) {}
    fn on_key_down(&mut self, _key: u8) {}

    fn title(&self) -> String {
        "DXSample".into()
    }

    fn window_size(&self) -> (i32, i32) {
        (640, 480)
    }
}

#[derive(Clone)]
struct SampleCommandLine {
    use_warp_device: bool,
}

fn build_command_line() -> SampleCommandLine {
    let mut use_warp_device = false;

    for arg in std::env::args() {
        if arg.eq_ignore_ascii_case("-warp") || arg.eq_ignore_ascii_case("/warp") {
            use_warp_device = true;
        }
    }

    SampleCommandLine { use_warp_device }
}

fn run_sample<S>() -> Result<()>
where
    S: DXSample,
{
    let instance = unsafe { GetModuleHandleA(None)? };

    let wc = WNDCLASSEXA {
        cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc::<S>),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: s!("RustWindowClass"),
        ..Default::default()
    };

    let command_line = build_command_line();
    let mut sample = S::new(&command_line)?;

    let size = sample.window_size();

    let atom = unsafe { RegisterClassExA(&wc) };
    debug_assert_ne!(atom, 0);

    let mut window_rect = RECT {
        left: 0,
        top: 0,
        right: size.0,
        bottom: size.1,
    };
    unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false)? };

    let mut title = sample.title();

    if command_line.use_warp_device {
        title.push_str(" (WARP)");
    }

    title.push('\0'); // Null-terminate the string for C API

    let hwnd = unsafe {
        CreateWindowExA(
            WINDOW_EX_STYLE::default(),
            s!("RustWindowClass"),
            PCSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            None,                  // no parent window
            None,                  // no menus
            Some(instance.into()), // Use instance from GetModuleHandleA
            Some(&mut sample as *mut _ as _),
        )
    }?;

    sample.bind_to_window(&hwnd)?;
    unsafe { _ = ShowWindow(hwnd, SW_SHOW) };

    let mut done = false;
    while !done {
        let mut message = MSG::default();

        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                _ = TranslateMessage(&message);
                DispatchMessageA(&message);
            }

            if message.message == WM_QUIT {
                done = true; // Exit loop
            }
        } else {
            // Render when idle, handle potential errors
            if let Err(e) = sample.render() {
                eprintln!("Render error: {:?}", e);
                // Decide how to handle render errors, maybe break the loop
                // For now, we'll just print and continue
            }
        }
    }

    // Call OnDestroy for cleanup synchronization before dropping sample
    sample.on_destroy();

    Ok(())
}

// Wrapper function to handle potential panics in sample_wndproc
fn safe_sample_wndproc<S: DXSample>(sample: &mut S, message: u32, wparam: WPARAM) -> bool {
    // Use catch_unwind if you need to handle panics gracefully,
    // otherwise direct call is fine for simpler examples.
    // std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    //     sample_wndproc_impl(sample, message, wparam)
    // })).unwrap_or(false) // Default to false if panic occurs

    // Direct call for simplicity here:
    sample_wndproc_impl(sample, message, wparam)
}

// Original logic moved here
fn sample_wndproc_impl<S: DXSample>(sample: &mut S, message: u32, wparam: WPARAM) -> bool {
    match message {
        WM_KEYDOWN => {
            sample.on_key_down(wparam.0 as u8);
            true
        }
        WM_KEYUP => {
            sample.on_key_up(wparam.0 as u8);
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

extern "system" fn wndproc<S: DXSample>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CREATE {
        unsafe {
            let create_struct: &CREATESTRUCTA = &*(lparam.0 as *const CREATESTRUCTA);
            SetWindowLongPtrA(window, GWLP_USERDATA, create_struct.lpCreateParams as _);
        }
        return LRESULT(0);
    }

    let user_data = unsafe { GetWindowLongPtrA(window, GWLP_USERDATA) };
    if user_data == 0 {
        // We can get messages before WM_CREATE or after WM_DESTROY.
        return unsafe { DefWindowProcA(window, message, wparam, lparam) };
    }

    let sample = std::ptr::NonNull::<S>::new(user_data as *mut S);

    // Use a scope to ensure the mutable borrow ends before DefWindowProc
    let handled = if let Some(mut s) = sample {
        match message {
            WM_DESTROY => {
                // Don't call on_destroy here, call it explicitly before exiting run_sample
                unsafe { PostQuitMessage(0) };
                true // Mark as handled
            }
            _ => {
                // Use the safe wrapper
                safe_sample_wndproc(unsafe { s.as_mut() }, message, wparam)
            }
        }
    } else {
        false
    };

    if handled {
        LRESULT(0)
    } else {
        unsafe { DefWindowProcA(window, message, wparam, lparam) }
    }
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    for i in 0.. {
        let adapter = unsafe { factory.EnumAdapters1(i)? };
        let desc = unsafe { adapter.GetDesc1()? };

        if (DXGI_ADAPTER_FLAG(desc.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            continue;
        }

        if unsafe {
            D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_11_0, // Use a common feature level
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
        }
        .is_ok()
        {
            println!(
                "Using hardware adapter: {}",
                String::from_utf16_lossy(&desc.Description)
            );
            return Ok(adapter);
        }
    }
    // Should be unreachable if a D3D12 capable device exists
    Err(Error::new(E_FAIL, "No suitable hardware adapter found."))
}

mod d3d12_hello_triangle_buffered {
    // Renamed module
    use super::*;
    use std::mem::ManuallyDrop;

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
        fn new(command_line: &SampleCommandLine) -> Result<Self> {
            let (dxgi_factory, device) = create_device(command_line)?;
            Ok(Sample {
                dxgi_factory,
                device,
                resources: None,
                window_size: (1280, 720), // Default size
            })
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
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };

            let swap_chain_base: IDXGISwapChain1 = unsafe {
                self.dxgi_factory.CreateSwapChainForHwnd(
                    &command_queue,
                    *hwnd,
                    &swap_chain_desc,
                    None,
                    None,
                )?
            };
            let swap_chain: IDXGISwapChain3 = swap_chain_base.cast()?;

            unsafe {
                self.dxgi_factory
                    .MakeWindowAssociation(*hwnd, DXGI_MWA_NO_ALT_ENTER)?;
            }

            let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() };

            let rtv_heap: ID3D12DescriptorHeap = unsafe {
                self.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        NumDescriptors: FRAME_COUNT,
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                        ..Default::default()
                    })
            }?;

            let rtv_descriptor_size = unsafe {
                self.device
                    .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
            };
            let rtv_handle = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

            let mut render_targets: [MaybeUninit<ID3D12Resource>; FRAME_COUNT as usize] =
                MaybeUninit::uninit_array();

            for i in 0..FRAME_COUNT {
                let resource: ID3D12Resource = unsafe { swap_chain.GetBuffer(i)? };
                unsafe {
                    self.device.CreateRenderTargetView(
                        &resource,
                        None,
                        D3D12_CPU_DESCRIPTOR_HANDLE {
                            ptr: rtv_handle.ptr + (i * rtv_descriptor_size) as usize,
                        },
                    );
                }
                render_targets[i as usize].write(resource);
            }
            // Safety: All elements initialized in the loop above
            let render_targets = unsafe { MaybeUninit::array_assume_init(render_targets) };

            // --- Frame Buffering Changes ---
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
            let fence_values = [0u64; FRAME_COUNT as usize]; // Initialize fence values
            let fence_event = unsafe { CreateEventA(None, false, false, None)? };
            if fence_event.is_invalid() {
                return Err(Error::from_win32());
            }
            // --- End Frame Buffering Changes ---

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

            let root_signature = create_root_signature(&self.device)?;
            let pso = create_pipeline_state(&self.device, &root_signature)?;

            // Create the command list using the first allocator.
            let command_list: ID3D12GraphicsCommandList = unsafe {
                self.device.CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &command_allocators[frame_index as usize], // Use current frame's allocator
                    &pso,                                      // Initial PSO
                )
            }?;
            // Command lists are created in the recording state, but there is nothing
            // to record yet. The main loop expects it to be closed, so close it now.
            unsafe { command_list.Close()? };

            let aspect_ratio = width as f32 / height as f32;
            let (vertex_buffer, vbv) = create_vertex_buffer(&self.device, aspect_ratio)?;

            let mut resources = Resources {
                command_queue,
                swap_chain,
                frame_index,
                render_targets,
                rtv_heap,
                rtv_descriptor_size,
                viewport,
                scissor_rect,
                command_allocators, // Store the array
                root_signature,
                pso,
                command_list,
                vertex_buffer,
                vbv,
                fence,
                fence_values, // Store the array
                fence_event,
            };

            // Wait for GPU to finish any initial setup potentially submitted implicitly
            // (though less critical here as we close the list immediately).
            // This mirrors the C++ sample's initial WaitForGpu().
            wait_for_gpu(&mut resources)?;

            self.resources = Some(resources);

            Ok(())
        }

        fn title(&self) -> String {
            "D3D12 Hello Triangle (Frame Buffered)".into()
        }

        fn window_size(&self) -> (i32, i32) {
            self.window_size
        }

        fn render(&mut self) -> Result<()> {
            // Return Result
            if let Some(resources) = &mut self.resources {
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
            }
            Ok(())
        }

        // Called before the sample is dropped
        fn on_destroy(&mut self) {
            if let Some(resources) = &mut self.resources {
                // Ensure that the GPU is no longer referencing resources that are about to be
                // cleaned up by the destructor.
                if let Err(e) = wait_for_gpu(resources) {
                    eprintln!("Error waiting for GPU on destroy: {:?}", e);
                }
                unsafe {
                    // Ensure the handle is valid before closing
                    if !resources.fence_event.is_invalid() {
                        CloseHandle(resources.fence_event).unwrap();
                        // Optionally invalidate the handle in the struct if it were mutable
                        // resources.fence_event = HANDLE::invalid();
                    }
                }
            }
            // Resources are dropped automatically when `self.resources` goes out of scope
        }
    }

    // --- Frame Buffering: New/Modified Helper Functions ---

    // Wait for pending GPU work to complete for the *current* frame index.
    // Used for initial setup and final cleanup.
    fn wait_for_gpu(resources: &mut Resources) -> Result<()> {
        // Schedule a Signal command in the queue for the current frame index.
        let fence_value = resources.fence_values[resources.frame_index as usize];
        unsafe {
            resources
                .command_queue
                .Signal(&resources.fence, fence_value)?
        };

        // Wait until the fence has been processed.
        if unsafe { resources.fence.GetCompletedValue() } < fence_value {
            unsafe {
                resources
                    .fence
                    .SetEventOnCompletion(fence_value, resources.fence_event)?;
                // Consider adding a timeout to WaitForSingleObjectEx for robustness
                WaitForSingleObjectEx(resources.fence_event, INFINITE, false);
            }
        }

        // Increment the fence value for the current frame *after* waiting.
        // Note: The C++ sample increments before waiting, which seems slightly off.
        // Incrementing after ensures the value represents work *submitted* for this frame index.
        // However, for the MoveToNextFrame logic, the C++ approach works. Let's stick
        // to the C++ sample's logic for MoveToNextFrame consistency.
        // We'll increment it here for the initial wait, mirroring C++'s WaitForGpu.
        resources.fence_values[resources.frame_index as usize] += 1;

        Ok(())
    }

    // Prepare to render the next frame.
    fn move_to_next_frame(resources: &mut Resources) -> Result<()> {
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

    // --- End Frame Buffering Helpers ---

    fn populate_command_list(resources: &mut Resources) -> Result<()> {
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

        unsafe { command_list.Close() }
    }

    // (Keep transition_barrier, create_device, create_root_signature,
    // create_pipeline_state, create_vertex_buffer, Vertex struct as before)

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

    // Create D3D12 Device and DXGI Factory
    fn create_device(command_line: &SampleCommandLine) -> Result<(IDXGIFactory4, ID3D12Device)> {
        let mut debug_flags = DXGI_CREATE_FACTORY_FLAGS(0);
        if cfg!(debug_assertions) {
            unsafe {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
                    debug.EnableDebugLayer();
                    debug_flags |= DXGI_CREATE_FACTORY_DEBUG;
                    println!("D3D12 Debug Layer Enabled");
                } else {
                    eprintln!("Warning: D3D12 Debug Layer unavailable.");
                }
            }
        }

        let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(debug_flags) }?;

        let adapter = if command_line.use_warp_device {
            println!("Using WARP adapter.");
            unsafe { dxgi_factory.EnumWarpAdapter()? }
        } else {
            get_hardware_adapter(&dxgi_factory)?
        };

        let mut device: Option<ID3D12Device> = None;
        unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?; // Request 11_0 feature level
        Ok((dxgi_factory, device.unwrap()))
    }

    // Create Root Signature
    fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            ..Default::default()
        };

        let mut signature_blob = None;
        let mut error_blob = None;

        let signature = unsafe {
            D3D12SerializeRootSignature(
                &desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature_blob,
                Some(&mut error_blob), // Capture potential errors
            )
        };

        if let Err(e) = signature {
            if let Some(error) = error_blob {
                let error_msg = unsafe {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        error.GetBufferPointer() as *const u8,
                        error.GetBufferSize(),
                    ))
                };
                eprintln!("Root Signature Error: {}", error_msg);
            }
            return Err(e); // Propagate the original error
        }

        let signature_blob = signature_blob.unwrap(); // Safe to unwrap after check

        unsafe {
            device.CreateRootSignature(
                0, // nodeMask, usually 0 for single GPU
                std::slice::from_raw_parts(
                    signature_blob.GetBufferPointer() as *const u8, // Use const u8 ptr
                    signature_blob.GetBufferSize(),
                ),
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

        // Ensure shaders.hlsl is next to the executable
        let exe_path = std::env::current_exe().expect("Failed to get executable path");
        let asset_path = exe_path
            .parent()
            .expect("Failed to get executable directory");
        let shaders_hlsl_path = asset_path.join("shaders.hlsl");

        if !shaders_hlsl_path.exists() {
            panic!("shaders.hlsl not found at {:?}", shaders_hlsl_path);
        }

        let shaders_hlsl: HSTRING = shaders_hlsl_path.to_str().unwrap().into();

        let vs_entry = s!("VSMain");
        let ps_entry = s!("PSMain");
        let vs_target = s!("vs_5_0");
        let ps_target = s!("ps_5_0");

        let vertex_shader = compile_shader(&shaders_hlsl, vs_entry, vs_target, compile_flags)?;
        let pixel_shader = compile_shader(&shaders_hlsl, ps_entry, ps_target, compile_flags)?;

        let input_element_descs: [D3D12_INPUT_ELEMENT_DESC; 2] = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"), // Use s! macro
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default() // Other fields are 0 or default
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("COLOR"), // Use s! macro
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                AlignedByteOffset: 12, // Offset after position (3 * 4 bytes)
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
        ];

        let pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: unsafe { std::mem::transmute_copy(root_signature) }, // Clone pointer
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
                CullMode: D3D12_CULL_MODE_NONE, // Render both sides for simplicity
                ..Default::default()
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: FALSE,
                IndependentBlendEnable: FALSE,
                RenderTarget: [
                    D3D12_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: FALSE, // Disable blending
                        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                        ..Default::default() // Sensible defaults for other fields
                    }; 8 // Initialize all 8 render target blend descs
                ],
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: FALSE,   // No depth testing needed
                StencilEnable: FALSE, // No stencil testing needed
                ..Default::default()
            },
            SampleMask: u32::MAX,
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: [
                // Array needs initialization
                DXGI_FORMAT_R8G8B8A8_UNORM,
                DXGI_FORMAT_UNKNOWN, // Mark unused slots
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
                Some(&mut error_blob), // Capture errors
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
                eprintln!(
                    "Shader Compile Error ({} {}): {}",
                    unsafe { entry_point.to_string() }?,
                    unsafe { target.to_string() }?,
                    error_msg
                );
            }
            Err(e) // Propagate the error
        } else {
            Ok(shader_blob.unwrap()) // Safe to unwrap on success
        }
    }

    // Create Vertex Buffer
    fn create_vertex_buffer(
        device: &ID3D12Device,
        aspect_ratio: f32,
    ) -> Result<(ID3D12Resource, D3D12_VERTEX_BUFFER_VIEW)> {
        #[repr(C)] // Ensure C-compatible layout
        struct Vertex {
            position: [f32; 3],
            color: [f32; 4],
        }

        let vertices = [
            Vertex {
                position: [0.0, 0.25 * aspect_ratio, 0.0],
                color: [1.0, 0.0, 0.0, 1.0], // Red
            },
            Vertex {
                position: [0.25, -0.25 * aspect_ratio, 0.0],
                color: [0.0, 1.0, 0.0, 1.0], // Green
            },
            Vertex {
                position: [-0.25, -0.25 * aspect_ratio, 0.0],
                color: [0.0, 0.0, 1.0, 1.0], // Blue
            },
        ];
        let vertex_buffer_size = std::mem::size_of_val(&vertices) as u64;

        // Use an UPLOAD heap for simplicity. For performance-critical apps,
        // use a DEFAULT heap and an intermediate UPLOAD heap for transfer.
        let heap_props = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_UPLOAD,
            ..Default::default()
        };
        let resource_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: vertex_buffer_size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, // Required for buffers
            ..Default::default()
        };

        let mut vertex_buffer: Option<ID3D12Resource> = None;
        unsafe {
            device.CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &resource_desc,
                D3D12_RESOURCE_STATE_GENERIC_READ, // Initial state for upload heap
                None,                              // No optimized clear value
                &mut vertex_buffer,
            )?
        };
        let vertex_buffer = vertex_buffer.unwrap(); // Safe to unwrap after check

        // Map, copy data, and unmap
        unsafe {
            let mut data_ptr = std::ptr::null_mut();
            // We do not intend to read from this resource on the CPU. (Empty range)
            let read_range = D3D12_RANGE { Begin: 0, End: 0 };
            vertex_buffer.Map(0, Some(&read_range), Some(&mut data_ptr))?;
            std::ptr::copy_nonoverlapping(
                vertices.as_ptr(),
                data_ptr as *mut Vertex,
                vertices.len(),
            );
            // Null range indicates entire resource might have been written
            vertex_buffer.Unmap(0, None);
        }

        let vbv = D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
            StrideInBytes: std::mem::size_of::<Vertex>() as u32,
            SizeInBytes: vertex_buffer_size as u32,
        };

        Ok((vertex_buffer, vbv))
    }

    // --- std::mem::MaybeUninit helpers ---
    // Helper to create an array of MaybeUninit, needed because ID3D12Resource etc. don't impl Copy
    use std::mem::MaybeUninit;
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
            // This is a tricky transmute; alternative is pointer-based initialization.
            (&array as *const _ as *const [T; N]).read()
            // Or, less efficiently but perhaps safer without deep unsafe knowledge:
            // array.map(|elem| elem.assume_init()) // Requires T: Copy, which COM ptrs are not.
            // Need the transmute or pointer manipulation for non-Copy types.
        }
    }
} // end mod d3d12_hello_triangle_buffered

fn main() -> Result<()> {
    // Use the buffered version
    run_sample::<d3d12_hello_triangle_buffered::Sample>()?;
    Ok(())
}

// Ensure you have a `shaders.hlsl` file next to your executable with:
/*
struct PSInput
{
    float4 position : SV_POSITION;
    float4 color : COLOR;
};

PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
{
    PSInput result;
    result.position = position;
    result.color = color;
    return result;
}

float4 PSMain(PSInput input) : SV_TARGET
{
    return input.color;
}
*/

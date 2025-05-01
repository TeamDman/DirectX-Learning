use std::mem::MaybeUninit;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::*;

const FRAME_COUNT: u32 = 2; // Use 2 for basic buffering, 3 for potentially smoother results

pub struct Resources {
    pub command_queue: ID3D12CommandQueue,
    pub swap_chain: IDXGISwapChain3,
    pub frame_index: u32,
    pub render_targets: [ID3D12Resource; FRAME_COUNT as usize],
    pub rtv_heap: ID3D12DescriptorHeap,
    pub rtv_descriptor_size: u32, // Changed to u32 to match API
    pub viewport: D3D12_VIEWPORT,
    pub scissor_rect: RECT,
    // --- Frame Buffering Changes ---
    pub command_allocators: [ID3D12CommandAllocator; FRAME_COUNT as usize],
    pub fence: ID3D12Fence,
    pub fence_values: [u64; FRAME_COUNT as usize], // Fence value for each frame
    pub fence_event: HANDLE,
    // --- End Frame Buffering Changes ---
    pub root_signature: ID3D12RootSignature,
    pub pso: ID3D12PipelineState,
    pub command_list: ID3D12GraphicsCommandList,
    pub vertex_buffer: ID3D12Resource, // Keep vertex buffer handle
    pub vbv: D3D12_VERTEX_BUFFER_VIEW,
}

pub fn bind_to_window(
    device: &mut ID3D12Device,
    dxgi_factory: &mut IDXGIFactory4,
    hwnd: &HWND,
    window_size: (u32, u32),
) -> Result<Resources> {
    let command_queue: ID3D12CommandQueue = unsafe {
        device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?
    };

    let (width, height) = window_size; // Use stored size

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
        dxgi_factory.CreateSwapChainForHwnd(&command_queue, *hwnd, &swap_chain_desc, None, None)?
    };
    let swap_chain: IDXGISwapChain3 = swap_chain_base.cast()?;

    unsafe {
        dxgi_factory.MakeWindowAssociation(*hwnd, DXGI_MWA_NO_ALT_ENTER)?;
    }

    let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() };

    let rtv_heap: ID3D12DescriptorHeap = unsafe {
        device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            NumDescriptors: FRAME_COUNT,
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            ..Default::default()
        })
    }?;

    let rtv_descriptor_size =
        unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) };
    let rtv_handle = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

    let mut render_targets: [MaybeUninit<ID3D12Resource>; FRAME_COUNT as usize] =
        MaybeUninit::uninit_array();

    for i in 0..FRAME_COUNT {
        let resource: ID3D12Resource = unsafe { swap_chain.GetBuffer(i)? };
        unsafe {
            device.CreateRenderTargetView(
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
    let mut command_allocators: [MaybeUninit<ID3D12CommandAllocator>; FRAME_COUNT as usize] =
        MaybeUninit::uninit_array();
    for i in 0..FRAME_COUNT {
        command_allocators[i as usize]
            .write(unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)? });
    }
    // Safety: All elements initialized in the loop above
    let command_allocators = unsafe { MaybeUninit::array_assume_init(command_allocators) };

    let fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
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
        right: width as i32,
        bottom: height as i32,
    };

    let root_signature = create_root_signature(&device)?;
    let pso = create_pipeline_state(&device, &root_signature)?;

    // Create the command list using the first allocator.
    let command_list: ID3D12GraphicsCommandList = unsafe {
        device.CreateCommandList(
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
    let (vertex_buffer, vbv) = create_vertex_buffer(&device, aspect_ratio)?;

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

    Ok(resources)
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
        std::ptr::copy_nonoverlapping(vertices.as_ptr(), data_ptr as *mut Vertex, vertices.len());
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
trait MaybeUninitHelper<T, const N: usize> {
    fn uninit_array() -> [MaybeUninit<T>; N];
    // unsafe fn array_assume_init(array: [MaybeUninit<T>; N]) -> [T; N];
}

impl<T, const N: usize> MaybeUninitHelper<T, N> for MaybeUninit<T> {
    #[inline]
    fn uninit_array() -> [MaybeUninit<T>; N] {
        // Safety: An uninitialized `[MaybeUninit<_>; N]` is valid.
        unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }
    }

    // #[inline]
    // unsafe fn array_assume_init(array: [MaybeUninit<T>; N]) -> [T; N] {
    //     // Safety: The caller guarantees that all elements of the array are initialized.
    //     // This is a tricky transmute; alternative is pointer-based initialization.
    //     (&array as *const _ as *const [T; N]).read()
    //     // Or, less efficiently but perhaps safer without deep unsafe knowledge:
    //     // array.map(|elem| elem.assume_init()) // Requires T: Copy, which COM ptrs are not.
    //     // Need the transmute or pointer manipulation for non-Copy types.
    // }
}

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

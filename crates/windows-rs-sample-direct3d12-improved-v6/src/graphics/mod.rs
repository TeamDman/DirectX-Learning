use eyre::Context;
use std::path::PathBuf;
use teamy_windows::module::get_current_module;
use teamy_windows::string::EasyPCWSTR;
use tracing::info;
use windows::Win32::Foundation::{E_FAIL, FALSE, HANDLE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompileFromFile};
use windows::Win32::Graphics::Direct3D::{D3D_FEATURE_LEVEL_11_0, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObjectEx};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Error, HSTRING, Interface, Owned, PCSTR, s, w};

const FRAME_COUNT: usize = 2;
const WINDOW_CLASS_NAME: windows::core::PCWSTR = w!("DirectXLearningTransparentTriangleV5");

#[derive(Debug, Clone)]
pub struct TransparentTriangleOptions {
    pub width: u32,
    pub height: u32,
    pub use_warp_device: bool,
    pub title: String,
}

pub fn run(options: TransparentTriangleOptions) -> eyre::Result<()> {
    info!(?options, "Starting transparent triangle sample");

    let hwnd = create_window(&options)?;
    let mut renderer = Renderer::new(hwnd, &options)?;

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }

    message_loop(&mut renderer)
}

fn message_loop(renderer: &mut Renderer) -> eyre::Result<()> {
    loop {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            if message.message == WM_QUIT {
                return Ok(());
            }

            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        renderer.render()?;
    }
}

fn create_window(options: &TransparentTriangleOptions) -> eyre::Result<HWND> {
    let instance = get_current_module()?;

    let window_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: WINDOW_CLASS_NAME,
        ..Default::default()
    };

    let atom = unsafe { RegisterClassExW(&window_class) };
    if atom == 0 {
        info!("Window class registration returned 0, assuming the class already exists");
    }

    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let width = options.width as i32;
    let height = options.height as i32;
    let x = (screen_width - width) / 2;
    let y = (screen_height - height) / 2;
    let title = options.title.as_str().easy_pcwstr()?;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_NOREDIRECTIONBITMAP,
            WINDOW_CLASS_NAME,
            title.as_ref(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .wrap_err("Failed to create transparent window")?;

    Ok(hwnd)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u32 == 0x1B => {
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

#[derive(Debug)]
struct Renderer {
    _hwnd: HWND,
    _dxgi_factory: IDXGIFactory4,
    _device: ID3D12Device,
    _dcomp_device: IDCompositionDevice,
    _dcomp_target: IDCompositionTarget,
    _dcomp_visual: IDCompositionVisual,
    command_queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    render_targets: [ID3D12Resource; FRAME_COUNT],
    rtv_heap: ID3D12DescriptorHeap,
    rtv_descriptor_size: u32,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
    fence: ID3D12Fence,
    fence_value: u64,
    fence_event: Owned<HANDLE>,
    root_signature: ID3D12RootSignature,
    pipeline_state: ID3D12PipelineState,
    _vertex_buffer: ID3D12Resource,
    vertex_buffer_view: D3D12_VERTEX_BUFFER_VIEW,
    viewport: D3D12_VIEWPORT,
    scissor_rect: RECT,
}

impl Renderer {
    fn new(hwnd: HWND, options: &TransparentTriangleOptions) -> eyre::Result<Self> {
        let (dxgi_factory, device) = create_device(options.use_warp_device)?;
        let command_queue = create_command_queue(&device)?;
        let swap_chain = create_swap_chain(&dxgi_factory, &command_queue, options.width, options.height)?;
        unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)? };

        let (dcomp_device, dcomp_target, dcomp_visual) =
            attach_swap_chain_to_window(hwnd, &device, &swap_chain)?;

        let (rtv_heap, rtv_descriptor_size, render_targets) =
            create_render_targets(&device, &swap_chain)?;
        let command_allocator: ID3D12CommandAllocator =
            unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }?;
        let root_signature = create_root_signature(&device)?;
        let pipeline_state = create_pipeline_state(&device, &root_signature)?;
        let command_list: ID3D12GraphicsCommandList = unsafe {
            device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &command_allocator,
                &pipeline_state,
            )
        }?;
        unsafe { command_list.Close()? };

        let (vertex_buffer, vertex_buffer_view) = create_vertex_buffer(&device)?;
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }?;
        let fence_event = unsafe { Owned::new(CreateEventW(None, false, false, None)?) };

        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: options.width as f32,
            Height: options.height as f32,
            MinDepth: D3D12_MIN_DEPTH,
            MaxDepth: D3D12_MAX_DEPTH,
        };
        let scissor_rect = RECT {
            left: 0,
            top: 0,
            right: options.width as i32,
            bottom: options.height as i32,
        };

        Ok(Self {
            _hwnd: hwnd,
            _dxgi_factory: dxgi_factory,
            _device: device,
            _dcomp_device: dcomp_device,
            _dcomp_target: dcomp_target,
            _dcomp_visual: dcomp_visual,
            command_queue,
            swap_chain,
            render_targets,
            rtv_heap,
            rtv_descriptor_size,
            command_allocator,
            command_list,
            fence,
            fence_value: 1,
            fence_event,
            root_signature,
            pipeline_state,
            _vertex_buffer: vertex_buffer,
            vertex_buffer_view,
            viewport,
            scissor_rect,
        })
    }

    fn render(&mut self) -> eyre::Result<()> {
        let frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() as usize };
        let current_target = &self.render_targets[frame_index];

        unsafe {
            self.command_allocator.Reset()?;
            self.command_list
                .Reset(&self.command_allocator, &self.pipeline_state)?;

            self.command_list.SetGraphicsRootSignature(&self.root_signature);
            self.command_list.RSSetViewports(&[self.viewport]);
            self.command_list.RSSetScissorRects(&[self.scissor_rect]);

            self.command_list.ResourceBarrier(&[transition_barrier(
                current_target,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            )]);

            let rtv_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: self.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr
                    + frame_index * self.rtv_descriptor_size as usize,
            };
            self.command_list
                .OMSetRenderTargets(1, Some(&rtv_handle), false, None);

            let clear_color = [0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32];
            self.command_list
                .ClearRenderTargetView(rtv_handle, &clear_color, None);
            self.command_list
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.command_list
                .IASetVertexBuffers(0, Some(&[self.vertex_buffer_view]));
            self.command_list.DrawInstanced(3, 1, 0, 0);

            self.command_list.ResourceBarrier(&[transition_barrier(
                current_target,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            )]);
            self.command_list.Close()?;
        }

        let command_lists = [Some(self.command_list.cast::<ID3D12CommandList>()?)];
        unsafe {
            self.command_queue.ExecuteCommandLists(&command_lists);
            self.swap_chain.Present(1, DXGI_PRESENT(0)).ok()?;
        }

        self.wait_for_gpu()
    }

    fn wait_for_gpu(&mut self) -> eyre::Result<()> {
        unsafe {
            self.command_queue.Signal(&self.fence, self.fence_value)?;
            if self.fence.GetCompletedValue() < self.fence_value {
                self.fence
                    .SetEventOnCompletion(self.fence_value, *self.fence_event)?;
                WaitForSingleObjectEx(*self.fence_event, INFINITE, false);
            }
        }
        self.fence_value += 1;
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.wait_for_gpu();
    }
}

fn create_device(use_warp_device: bool) -> eyre::Result<(IDXGIFactory4, ID3D12Device)> {
    let mut dxgi_flags = DXGI_CREATE_FACTORY_FLAGS(0);
    if cfg!(debug_assertions) {
        unsafe {
            let mut debug = None;
            if D3D12GetDebugInterface::<ID3D12Debug>(&mut debug).is_ok() {
                if let Some(debug) = debug {
                    debug.EnableDebugLayer();
                    dxgi_flags |= DXGI_CREATE_FACTORY_DEBUG;
                }
            }
        }
    }

    let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(dxgi_flags) }?;
    let adapter = if use_warp_device {
        info!("Using WARP adapter");
        unsafe { dxgi_factory.EnumWarpAdapter() }?
    } else {
        get_hardware_adapter(&dxgi_factory)?
    };

    let mut device = None;
    unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;
    let device = device.expect("device should be initialized after D3D12CreateDevice succeeds");
    Ok((dxgi_factory, device))
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> eyre::Result<IDXGIAdapter1> {
    for index in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(error.into()),
        };

        let description = unsafe { adapter.GetDesc1() }?;
        let is_software = (DXGI_ADAPTER_FLAG(description.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE;
        if is_software {
            continue;
        }

        let mut test_device: Option<ID3D12Device> = None;
        if unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut test_device) }
            .is_ok()
        {
            return Ok(adapter);
        }
    }

    Err(Error::new(E_FAIL, "No suitable D3D12 adapter found").into())
}

fn create_command_queue(device: &ID3D12Device) -> eyre::Result<ID3D12CommandQueue> {
    Ok(unsafe {
        device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?
    })
}

fn create_swap_chain(
    factory: &IDXGIFactory4,
    command_queue: &ID3D12CommandQueue,
    width: u32,
    height: u32,
) -> eyre::Result<IDXGISwapChain3> {
    let factory2: IDXGIFactory2 = factory.cast()?;
    let description = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: FRAME_COUNT as u32,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
    };

    let swap_chain: IDXGISwapChain1 = unsafe {
        factory2.CreateSwapChainForComposition(command_queue, &description, None)?
    };
    Ok(swap_chain.cast()?)
}

fn attach_swap_chain_to_window(
    hwnd: HWND,
    _device: &ID3D12Device,
    swap_chain: &IDXGISwapChain3,
) -> eyre::Result<(IDCompositionDevice, IDCompositionTarget, IDCompositionVisual)> {
    let dcomp_device: IDCompositionDevice =
        unsafe { DCompositionCreateDevice::<_, IDCompositionDevice>(None::<&IDXGIDevice>) }?;
    let dcomp_target = unsafe { dcomp_device.CreateTargetForHwnd(hwnd, true) }?;
    let dcomp_visual = unsafe { dcomp_device.CreateVisual() }?;

    unsafe {
        dcomp_visual.SetContent(swap_chain)?;
        dcomp_target.SetRoot(&dcomp_visual)?;
        dcomp_device.Commit()?;
    }

    Ok((dcomp_device, dcomp_target, dcomp_visual))
}

fn create_render_targets(
    device: &ID3D12Device,
    swap_chain: &IDXGISwapChain3,
) -> eyre::Result<(
    ID3D12DescriptorHeap,
    u32,
    [ID3D12Resource; FRAME_COUNT],
)> {
    let rtv_heap: ID3D12DescriptorHeap = unsafe {
        device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            NumDescriptors: FRAME_COUNT as u32,
            ..Default::default()
        })?
    };
    let rtv_descriptor_size = unsafe {
        device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
    };
    let heap_start = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

    let mut render_targets = std::array::from_fn(|_| None::<ID3D12Resource>);
    for (index, slot) in render_targets.iter_mut().enumerate() {
        let resource: ID3D12Resource = unsafe { swap_chain.GetBuffer(index as u32) }?;
        let descriptor = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: heap_start.ptr + index * rtv_descriptor_size as usize,
        };
        unsafe { device.CreateRenderTargetView(&resource, None, descriptor) };
        *slot = Some(resource);
    }

    Ok((rtv_heap, rtv_descriptor_size, render_targets.map(Option::unwrap)))
}

fn create_root_signature(device: &ID3D12Device) -> eyre::Result<ID3D12RootSignature> {
    let description = D3D12_ROOT_SIGNATURE_DESC {
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        ..Default::default()
    };

    let mut signature = None;
    let mut error = None;
    unsafe {
        D3D12SerializeRootSignature(
            &description,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut signature,
            Some(&mut error),
        )
    }
    .map_err(|err| shader_error(err, error).wrap_err("Failed to serialize root signature"))?;

    let signature = signature.expect("root signature blob should be initialized");
    Ok(unsafe {
        device.CreateRootSignature(
            0,
            std::slice::from_raw_parts(
                signature.GetBufferPointer() as *const u8,
                signature.GetBufferSize(),
            ),
        )?
    })
}

fn create_pipeline_state(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
) -> eyre::Result<ID3D12PipelineState> {
    let compile_flags = if cfg!(debug_assertions) {
        D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION
    } else {
        0
    };

    let shader_path = shader_path();
    let shader_path_hstring: HSTRING = shader_path.to_string_lossy().as_ref().into();
    let vertex_shader = compile_shader(&shader_path_hstring, s!("VSMain"), s!("vs_5_0"), compile_flags)?;
    let pixel_shader = compile_shader(&shader_path_hstring, s!("PSMain"), s!("ps_5_0"), compile_flags)?;

    let input_layout = [
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: s!("POSITION"),
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            ..Default::default()
        },
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: s!("COLOR"),
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            AlignedByteOffset: 12,
            ..Default::default()
        },
    ];

    let blend_target = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: TRUE,
        LogicOpEnable: FALSE,
        SrcBlend: D3D12_BLEND_ONE,
        DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };

    let description = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::ManuallyDrop::new(Some(root_signature.clone())),
        VS: shader_bytecode(&vertex_shader),
        PS: shader_bytecode(&pixel_shader),
        BlendState: D3D12_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [blend_target; 8],
        },
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            FrontCounterClockwise: FALSE,
            DepthBias: D3D12_DEFAULT_DEPTH_BIAS,
            DepthBiasClamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
            SlopeScaledDepthBias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
            DepthClipEnable: TRUE,
            MultisampleEnable: FALSE,
            AntialiasedLineEnable: FALSE,
            ForcedSampleCount: 0,
            ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            DepthEnable: FALSE,
            StencilEnable: FALSE,
            ..Default::default()
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: input_layout.as_ptr(),
            NumElements: input_layout.len() as u32,
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: [
            DXGI_FORMAT_B8G8R8A8_UNORM,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
        ],
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };

    Ok(unsafe { device.CreateGraphicsPipelineState(&description) }?)
}

fn compile_shader(
    path: &HSTRING,
    entry_point: PCSTR,
    target: PCSTR,
    flags: u32,
) -> eyre::Result<ID3DBlob> {
    let mut shader = None;
    let mut error = None;
    unsafe {
        D3DCompileFromFile(
            path,
            None,
            None,
            entry_point,
            target,
            flags,
            0,
            &mut shader,
            Some(&mut error),
        )
    }
    .map_err(|err| shader_error(err, error))?;

    Ok(shader.expect("shader blob should be initialized"))
}

fn shader_error(error: windows::core::Error, blob: Option<ID3DBlob>) -> eyre::Error {
    if let Some(blob) = blob {
        let bytes = unsafe {
            std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())
        };
        eyre::eyre!("{error}: {}", String::from_utf8_lossy(bytes).trim())
    } else {
        error.into()
    }
}

fn shader_bytecode(shader: &ID3DBlob) -> D3D12_SHADER_BYTECODE {
    D3D12_SHADER_BYTECODE {
        pShaderBytecode: unsafe { shader.GetBufferPointer() },
        BytecodeLength: unsafe { shader.GetBufferSize() },
    }
}

fn shader_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("shaders.hlsl")
}

fn create_vertex_buffer(
    device: &ID3D12Device,
) -> eyre::Result<(ID3D12Resource, D3D12_VERTEX_BUFFER_VIEW)> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Vertex {
        position: [f32; 3],
        color: [f32; 4],
    }

    let vertices = [
        Vertex {
            position: [0.0, 0.6, 0.0],
            color: [1.0, 0.2, 0.2, 1.0],
        },
        Vertex {
            position: [0.55, -0.4, 0.0],
            color: [0.2, 1.0, 0.4, 1.0],
        },
        Vertex {
            position: [-0.55, -0.4, 0.0],
            color: [0.2, 0.5, 1.0, 1.0],
        },
    ];
    let buffer_size = std::mem::size_of_val(&vertices) as u64;

    let mut vertex_buffer = None;
    unsafe {
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_UPLOAD,
                ..Default::default()
            },
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: buffer_size,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_GENERIC_READ,
            None,
            &mut vertex_buffer,
        )?
    };
    let vertex_buffer: ID3D12Resource =
        vertex_buffer.expect("vertex buffer should be initialized");

    unsafe {
        let mut mapped = std::ptr::null_mut();
        vertex_buffer.Map(0, None, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(
            vertices.as_ptr(),
            mapped as *mut Vertex,
            vertices.len(),
        );
        vertex_buffer.Unmap(0, None);
    }

    Ok((
        vertex_buffer.clone(),
        D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
            SizeInBytes: buffer_size as u32,
            StrideInBytes: std::mem::size_of::<Vertex>() as u32,
        },
    ))
}

fn transition_barrier(
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: std::mem::ManuallyDrop::new(Some(resource.clone())),
                StateBefore: before,
                StateAfter: after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
    }
}

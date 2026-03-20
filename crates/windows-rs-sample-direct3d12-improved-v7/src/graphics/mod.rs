use eyre::Context;
use std::path::PathBuf;
use std::ptr::NonNull;
use teamy_windows::module::get_current_module;
use teamy_windows::string::EasyPCWSTR;
use tracing::info;
use windows::Win32::Foundation::{E_FAIL, FALSE, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompileFromFile};
use windows::Win32::Graphics::Direct3D::{D3D_FEATURE_LEVEL_11_0, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObjectEx};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Error, HSTRING, Interface, Owned, PCSTR, s, w};

const FRAME_COUNT: usize = 2;
const WINDOW_CLASS_NAME: windows::core::PCWSTR = w!("DirectXLearningTransparentTriangleV6");

const TRIANGLE_VERTEX_COUNT: usize = 3;
const CURSOR_RING_SEGMENTS: usize = 48;
const CURSOR_RING_VERTEX_COUNT: usize = CURSOR_RING_SEGMENTS * 6;
const CURSOR_ARM_COUNT: usize = 4;
const CURSOR_ARM_VERTEX_COUNT: usize = CURSOR_ARM_COUNT * 6;
const MAX_VERTEX_COUNT: usize = TRIANGLE_VERTEX_COUNT + CURSOR_RING_VERTEX_COUNT + CURSOR_ARM_VERTEX_COUNT;

const CURSOR_RING_INNER_RADIUS: f32 = 14.0;
const CURSOR_RING_OUTER_RADIUS: f32 = 17.5;
const CURSOR_ARM_LENGTH: f32 = 18.0;
const CURSOR_ARM_THICKNESS: f32 = 2.5;
const CURSOR_ARM_GAP: f32 = 7.0;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

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
        hCursor: unsafe { LoadCursorW(None, IDC_CROSS)? },
        lpszClassName: WINDOW_CLASS_NAME,
        ..Default::default()
    };

    let atom = unsafe { RegisterClassExW(&window_class) };
    if atom == 0 {
        info!("Window class registration returned 0, assuming the class already exists");
    }

    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let width = screen_width;
    let height = screen_height;
    let x = 0;
    let y = 0;
    let title = options.title.as_str().easy_pcwstr()?;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_TOPMOST,
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
        WM_SETCURSOR => {
            if let Ok(cursor) = unsafe { LoadCursorW(None, IDC_CROSS) } {
                let _ = unsafe { SetCursor(Some(cursor)) };
                return LRESULT(1);
            }
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

#[derive(Debug)]
struct Renderer {
    hwnd: HWND,
    _dxgi_factory: IDXGIFactory4,
    _device: ID3D12Device,
    allow_tearing: bool,
    command_queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    render_targets: [ID3D12Resource; FRAME_COUNT],
    rtv_heap: ID3D12DescriptorHeap,
    rtv_descriptor_size: u32,
    command_allocators: [ID3D12CommandAllocator; FRAME_COUNT],
    command_list: ID3D12GraphicsCommandList,
    fence: ID3D12Fence,
    next_fence_value: u64,
    frame_fence_values: [u64; FRAME_COUNT],
    fence_event: Owned<HANDLE>,
    frame_latency_waitable_object: Owned<HANDLE>,
    root_signature: ID3D12RootSignature,
    pipeline_state: ID3D12PipelineState,
    vertex_buffer: ID3D12Resource,
    vertex_buffer_ptr: NonNull<Vertex>,
    vertex_buffer_view: D3D12_VERTEX_BUFFER_VIEW,
    scratch_vertices: Vec<Vertex>,
    viewport: D3D12_VIEWPORT,
    scissor_rect: RECT,
    width: u32,
    height: u32,
}

impl Renderer {
    fn new(hwnd: HWND, options: &TransparentTriangleOptions) -> eyre::Result<Self> {
        let (dxgi_factory, device) = create_device(options.use_warp_device)?;
        let allow_tearing = supports_allow_tearing(&dxgi_factory);
        let command_queue = create_command_queue(&device)?;
        let (width, height) = client_size(hwnd)?;
        let swap_chain =
            create_swap_chain(&dxgi_factory, &command_queue, hwnd, width, height, allow_tearing)?;
        unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)? };
        unsafe { swap_chain.SetMaximumFrameLatency(1)? };
        let frame_latency_waitable_object = unsafe {
            Owned::new(swap_chain.GetFrameLatencyWaitableObject())
        };
        info!(allow_tearing, "Swap chain tearing support");

        let (rtv_heap, rtv_descriptor_size, render_targets) =
            create_render_targets(&device, &swap_chain)?;
        let command_allocators = create_command_allocators(&device)?;
        let root_signature = create_root_signature(&device)?;
        let pipeline_state = create_pipeline_state(&device, &root_signature)?;
        let command_list: ID3D12GraphicsCommandList = unsafe {
            device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &command_allocators[0],
                &pipeline_state,
            )
        }?;
        unsafe { command_list.Close()? };

        let (vertex_buffer, vertex_buffer_view) = create_vertex_buffer(&device)?;
        let vertex_buffer_ptr = {
            let mut mapped = std::ptr::null_mut();
            unsafe { vertex_buffer.Map(0, None, Some(&mut mapped))? };
            NonNull::new(mapped as *mut Vertex)
                .ok_or_else(|| eyre::eyre!("Vertex buffer map returned a null pointer"))?
        };
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }?;
        let fence_event = unsafe { Owned::new(CreateEventW(None, false, false, None)?) };

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

        Ok(Self {
            hwnd,
            _dxgi_factory: dxgi_factory,
            _device: device,
            allow_tearing,
            command_queue,
            swap_chain,
            render_targets,
            rtv_heap,
            rtv_descriptor_size,
            command_allocators,
            command_list,
            fence,
            next_fence_value: 1,
            frame_fence_values: [0; FRAME_COUNT],
            fence_event,
            frame_latency_waitable_object,
            root_signature,
            pipeline_state,
            vertex_buffer,
            vertex_buffer_ptr,
            vertex_buffer_view,
            scratch_vertices: Vec::with_capacity(MAX_VERTEX_COUNT),
            viewport,
            scissor_rect,
            width,
            height,
        })
    }

    fn render(&mut self) -> eyre::Result<()> {
        self.wait_for_frame_latency()?;
        let frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() as usize };
        self.wait_for_frame(frame_index)?;

        let cursor_position = self.sample_cursor_position()?;
        let vertex_count = self.update_scene_vertices(cursor_position);

        let current_target = &self.render_targets[frame_index];
        let command_allocator = &self.command_allocators[frame_index];

        unsafe {
            command_allocator.Reset()?;
            self.command_list
                .Reset(command_allocator, &self.pipeline_state)?;

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
            self.command_list.DrawInstanced(vertex_count as u32, 1, 0, 0);

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
            let present_flags = if self.allow_tearing {
                DXGI_PRESENT_ALLOW_TEARING
            } else {
                DXGI_PRESENT(0)
            };
            self.swap_chain.Present(0, present_flags).ok()?;
        }

        self.signal_frame(frame_index)
    }

    fn wait_for_frame_latency(&self) -> eyre::Result<()> {
        if self.frame_latency_waitable_object.0.is_null() {
            return Err(eyre::eyre!("Swap chain did not provide a frame latency waitable object"));
        }

        unsafe {
            WaitForSingleObjectEx(*self.frame_latency_waitable_object, INFINITE, false);
        }

        Ok(())
    }

    fn sample_cursor_position(&self) -> eyre::Result<Option<(f32, f32)>> {
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) }.wrap_err("Failed to query cursor position")?;

        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }
            .wrap_err("Failed to query the window rectangle")?;

        let x = (point.x - window_rect.left) as f32;
        let y = (point.y - window_rect.top) as f32;
        if x < 0.0 || y < 0.0 || x >= self.width as f32 || y >= self.height as f32 {
            return Ok(None);
        }

        Ok(Some((x, y)))
    }

    fn update_scene_vertices(&mut self, cursor_position: Option<(f32, f32)>) -> usize {
        self.scratch_vertices.clear();
        append_demo_triangle(&mut self.scratch_vertices);

        if let Some((cursor_x, cursor_y)) = cursor_position {
            append_cursor_target(
                &mut self.scratch_vertices,
                self.width as f32,
                self.height as f32,
                cursor_x,
                cursor_y,
            );
        }

        // Keep the upload buffer mapped to avoid extra per-frame CPU jitter.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.scratch_vertices.as_ptr(),
                self.vertex_buffer_ptr.as_ptr(),
                self.scratch_vertices.len(),
            );
        }

        self.scratch_vertices.len()
    }

    fn wait_for_frame(&self, frame_index: usize) -> eyre::Result<()> {
        let fence_value = self.frame_fence_values[frame_index];
        if fence_value == 0 {
            return Ok(());
        }

        unsafe {
            if self.fence.GetCompletedValue() < fence_value {
                self.fence.SetEventOnCompletion(fence_value, *self.fence_event)?;
                WaitForSingleObjectEx(*self.fence_event, INFINITE, false);
            }
        }

        Ok(())
    }

    fn signal_frame(&mut self, frame_index: usize) -> eyre::Result<()> {
        let fence_value = self.next_fence_value;
        unsafe {
            self.command_queue.Signal(&self.fence, fence_value)?;
        }
        self.frame_fence_values[frame_index] = fence_value;
        self.next_fence_value += 1;
        Ok(())
    }

    fn wait_for_gpu(&mut self) -> eyre::Result<()> {
        let fence_value = self.next_fence_value;
        unsafe {
            self.command_queue.Signal(&self.fence, fence_value)?;
            if self.fence.GetCompletedValue() < fence_value {
                self.fence.SetEventOnCompletion(fence_value, *self.fence_event)?;
                WaitForSingleObjectEx(*self.fence_event, INFINITE, false);
            }
        }
        self.next_fence_value += 1;
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.wait_for_gpu();
        unsafe {
            self.vertex_buffer.Unmap(0, None);
        }
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

fn create_command_allocators(
    device: &ID3D12Device,
) -> eyre::Result<[ID3D12CommandAllocator; FRAME_COUNT]> {
    let mut allocators = std::array::from_fn(|_| None::<ID3D12CommandAllocator>);
    for slot in &mut allocators {
        *slot = Some(unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }?);
    }

    Ok(allocators.map(Option::unwrap))
}

fn create_swap_chain(
    factory: &IDXGIFactory4,
    command_queue: &ID3D12CommandQueue,
    hwnd: HWND,
    width: u32,
    height: u32,
    allow_tearing: bool,
) -> eyre::Result<IDXGISwapChain3> {
    let mut flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32;
    if allow_tearing {
        flags |= DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 as u32;
    }

    let description = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: FRAME_COUNT as u32,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: flags,
    };

    let swap_chain: IDXGISwapChain1 = unsafe {
        factory.CreateSwapChainForHwnd(command_queue, hwnd, &description, None, None)?
    };
    Ok(swap_chain.cast()?)
}

fn supports_allow_tearing(factory: &IDXGIFactory4) -> bool {
    let Ok(factory) = factory.cast::<IDXGIFactory5>() else {
        return false;
    };

    let mut allow_tearing = FALSE;
    unsafe {
        factory.CheckFeatureSupport(
            DXGI_FEATURE_PRESENT_ALLOW_TEARING,
            &mut allow_tearing as *mut _ as *mut _,
            std::mem::size_of_val(&allow_tearing) as u32,
        )
    }
    .is_ok()
        && allow_tearing.as_bool()
}

fn client_size(hwnd: HWND) -> eyre::Result<(u32, u32)> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) }.wrap_err("Failed to query client size")?;
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    Ok((width, height))
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
    let buffer_size = (std::mem::size_of::<Vertex>() * MAX_VERTEX_COUNT) as u64;

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

    Ok((
        vertex_buffer.clone(),
        D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
            SizeInBytes: buffer_size as u32,
            StrideInBytes: std::mem::size_of::<Vertex>() as u32,
        },
    ))
}

fn append_demo_triangle(vertices: &mut Vec<Vertex>) {
    vertices.extend_from_slice(&[
        Vertex {
            position: [0.0, 0.6, 0.0],
            color: [1.0, 0.2, 0.2, 0.85],
        },
        Vertex {
            position: [0.55, -0.4, 0.0],
            color: [0.2, 1.0, 0.4, 0.85],
        },
        Vertex {
            position: [-0.55, -0.4, 0.0],
            color: [0.2, 0.5, 1.0, 0.85],
        },
    ]);
}

fn append_cursor_target(
    vertices: &mut Vec<Vertex>,
    width: f32,
    height: f32,
    cursor_x: f32,
    cursor_y: f32,
) {
    let ring_color = [1.0, 1.0, 1.0, 0.95];
    let arm_color = [1.0, 0.15, 0.15, 0.95];

    append_ring(
        vertices,
        width,
        height,
        cursor_x,
        cursor_y,
        CURSOR_RING_INNER_RADIUS,
        CURSOR_RING_OUTER_RADIUS,
        ring_color,
    );

    append_rect(
        vertices,
        width,
        height,
        cursor_x - CURSOR_ARM_THICKNESS * 0.5,
        cursor_y - CURSOR_RING_OUTER_RADIUS - CURSOR_ARM_LENGTH,
        CURSOR_ARM_THICKNESS,
        CURSOR_ARM_LENGTH - CURSOR_ARM_GAP,
        arm_color,
    );
    append_rect(
        vertices,
        width,
        height,
        cursor_x - CURSOR_ARM_THICKNESS * 0.5,
        cursor_y + CURSOR_RING_OUTER_RADIUS + CURSOR_ARM_GAP,
        CURSOR_ARM_THICKNESS,
        CURSOR_ARM_LENGTH - CURSOR_ARM_GAP,
        arm_color,
    );
    append_rect(
        vertices,
        width,
        height,
        cursor_x - CURSOR_RING_OUTER_RADIUS - CURSOR_ARM_LENGTH,
        cursor_y - CURSOR_ARM_THICKNESS * 0.5,
        CURSOR_ARM_LENGTH - CURSOR_ARM_GAP,
        CURSOR_ARM_THICKNESS,
        arm_color,
    );
    append_rect(
        vertices,
        width,
        height,
        cursor_x + CURSOR_RING_OUTER_RADIUS + CURSOR_ARM_GAP,
        cursor_y - CURSOR_ARM_THICKNESS * 0.5,
        CURSOR_ARM_LENGTH - CURSOR_ARM_GAP,
        CURSOR_ARM_THICKNESS,
        arm_color,
    );
}

fn append_ring(
    vertices: &mut Vec<Vertex>,
    width: f32,
    height: f32,
    center_x: f32,
    center_y: f32,
    inner_radius: f32,
    outer_radius: f32,
    color: [f32; 4],
) {
    for segment in 0..CURSOR_RING_SEGMENTS {
        let angle_start = std::f32::consts::TAU * segment as f32 / CURSOR_RING_SEGMENTS as f32;
        let angle_end = std::f32::consts::TAU * (segment + 1) as f32 / CURSOR_RING_SEGMENTS as f32;

        let inner_start = (
            center_x + inner_radius * angle_start.cos(),
            center_y + inner_radius * angle_start.sin(),
        );
        let inner_end = (
            center_x + inner_radius * angle_end.cos(),
            center_y + inner_radius * angle_end.sin(),
        );
        let outer_start = (
            center_x + outer_radius * angle_start.cos(),
            center_y + outer_radius * angle_start.sin(),
        );
        let outer_end = (
            center_x + outer_radius * angle_end.cos(),
            center_y + outer_radius * angle_end.sin(),
        );

        push_quad(
            vertices,
            width,
            height,
            outer_start,
            outer_end,
            inner_end,
            inner_start,
            color,
        );
    }
}

fn append_rect(
    vertices: &mut Vec<Vertex>,
    width: f32,
    height: f32,
    left: f32,
    top: f32,
    rect_width: f32,
    rect_height: f32,
    color: [f32; 4],
) {
    push_quad(
        vertices,
        width,
        height,
        (left, top),
        (left + rect_width, top),
        (left + rect_width, top + rect_height),
        (left, top + rect_height),
        color,
    );
}

fn push_quad(
    vertices: &mut Vec<Vertex>,
    width: f32,
    height: f32,
    top_left: (f32, f32),
    top_right: (f32, f32),
    bottom_right: (f32, f32),
    bottom_left: (f32, f32),
    color: [f32; 4],
) {
    let top_left = to_ndc(width, height, top_left.0, top_left.1);
    let top_right = to_ndc(width, height, top_right.0, top_right.1);
    let bottom_right = to_ndc(width, height, bottom_right.0, bottom_right.1);
    let bottom_left = to_ndc(width, height, bottom_left.0, bottom_left.1);

    vertices.extend_from_slice(&[
        Vertex {
            position: top_left,
            color,
        },
        Vertex {
            position: top_right,
            color,
        },
        Vertex {
            position: bottom_right,
            color,
        },
        Vertex {
            position: top_left,
            color,
        },
        Vertex {
            position: bottom_right,
            color,
        },
        Vertex {
            position: bottom_left,
            color,
        },
    ]);
}

fn to_ndc(width: f32, height: f32, x: f32, y: f32) -> [f32; 3] {
    [
        (x / width) * 2.0 - 1.0,
        1.0 - (y / height) * 2.0,
        0.0,
    ]
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

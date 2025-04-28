use directx_fugue::init_dx11;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM}; // Added COLORREF
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{HBRUSH, UpdateWindow}; // Added HBRUSH
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

static CLASS_NAME: &str = "MyWindowClass";

#[repr(C)]
struct Vertex {
    pos: [f32; 2], // x, y
}

pub fn main() -> eyre::Result<()> {
    // --- Setup (Window Class, Instance) ---
    let hinstance = unsafe { GetModuleHandleW(None)? }.into();
    let class_name_ws = widestring::U16CString::from_str(CLASS_NAME)?;

    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name_ws.as_ptr()),
        // *** FIXED: Use HBRUSH(0) for null brush ***
        hbrBackground: HBRUSH::default(),
        style: CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    // --- Create Window ---
    let wnd_name_ws = widestring::U16CString::from_str("DX11 Transparent Triangle")?;
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT,
            PCWSTR(class_name_ws.as_ptr()),
            PCWSTR(wnd_name_ws.as_ptr()),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            Some(hinstance),
            None,
        )?
    };

    // (after CreateWindowExW)
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowLongW;
        use windows::Win32::UI::WindowsAndMessaging::LWA_ALPHA;
        use windows::Win32::UI::WindowsAndMessaging::LWA_COLORKEY;
        use windows::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes;
        use windows::Win32::UI::WindowsAndMessaging::SetWindowLongW;
        use windows::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED;
        use windows::Win32::UI::WindowsAndMessaging::WS_EX_TRANSPARENT;

        // turn on WS_EX_LAYERED + WS_EX_TRANSPARENT for click‐through
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let new_ex = (ex as u32 | WS_EX_LAYERED.0 as u32 | WS_EX_TRANSPARENT.0 as u32) as i32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex);

        // make black pixels fully‐transparent, but let your pixel shader α pass through
        SetLayeredWindowAttributes(
            hwnd,
            COLORREF(0x00_00_00), // colour key = pure black
            255,                  // ignored unless you use LWA_ALPHA alone
            LWA_COLORKEY | LWA_ALPHA,
        )?;
    }

    // --- Make window layered ---
    unsafe {
        // *** FIXED: Wrap color key in COLORREF() ***
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
    }

    // --- Show Window ---
    unsafe {
        _ = ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd).ok()?;
    }
    tracing::info!("Created window: {:?}", hwnd);

    // --- Initialize D3D11 ---
    let (device, context, swapchain, rtv) = init_dx11(hwnd)?;
    tracing::info!("DirectX 11 initialized!");

    // 3) Create & bind an alpha‐blend state once after device creation:
    let mut blend_state = None;
    unsafe {
        // build one RT blend‐desc…
        let rt_desc = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D11_BLEND_SRC_ALPHA,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_ZERO,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };

        // … and repeat it for all 8 targets:
        let blend_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: false.into(),
            IndependentBlendEnable: false.into(),
            RenderTarget: [rt_desc; 8],
        };

        device.CreateBlendState(&blend_desc, Some(&mut blend_state))?;
        context.OMSetBlendState(blend_state.as_ref(), None, 0xFFFF_FFFF);
    }

    // --- Load Shader Bytecode ---
    let vs_bytecode = include_bytes!("../shaders/triangle_vs.cso");
    let ps_bytecode = include_bytes!("../shaders/triangle_ps.cso");

    // --- Create Shaders ---
    let mut vertex_shader = None;
    unsafe {
        device.CreateVertexShader(
            vs_bytecode,
            None, // No class linkage
            Some(&mut vertex_shader),
        )?;
    }
    let vertex_shader = vertex_shader.unwrap();

    let mut pixel_shader = None;
    unsafe {
        device.CreatePixelShader(
            ps_bytecode,
            None, // No class linkage
            Some(&mut pixel_shader),
        )?;
    }
    let pixel_shader = pixel_shader.unwrap();

    // --- Define Triangle Vertices ---
    let vertices = [
        Vertex { pos: [0.0, 0.75] },
        Vertex { pos: [0.75, -0.75] },
        Vertex {
            pos: [-0.75, -0.75],
        },
    ];

    // --- Create Vertex Buffer ---
    let buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: (vertices.len() * size_of::<Vertex>()) as u32,
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        ..Default::default()
    };
    let subresource_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: vertices.as_ptr() as *const _,
        ..Default::default()
    };

    let mut vertex_buffer = None;
    unsafe {
        device.CreateBuffer(
            &buffer_desc,
            Some(&subresource_data),
            Some(&mut vertex_buffer),
        )?;
    }
    let vertex_buffer = vertex_buffer.unwrap();

    // --- Create Input Layout ---
    let input_element_desc = [D3D11_INPUT_ELEMENT_DESC {
        SemanticName: windows::core::s!("POSITION"),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 0,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    }];

    let mut input_layout = None;
    unsafe {
        device.CreateInputLayout(
            &input_element_desc,
            vs_bytecode, // Use VS bytecode for validation
            Some(&mut input_layout),
        )?;
    }
    let input_layout = input_layout.unwrap();

    // --- Create Blend State for Alpha Blending ---
    let mut blend_desc = D3D11_BLEND_DESC::default();
    blend_desc.RenderTarget[0].BlendEnable = true.into();
    blend_desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    blend_desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    blend_desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    blend_desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    blend_desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    blend_desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    blend_desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;

    let blend_state = blend_state.unwrap(); // Unwrap the Option

    // --- Main Loop ---
    let mut msg = MSG::default();
    loop {
        // --- Message Pump ---
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.into() {
            if msg.message == WM_QUIT {
                return Ok(());
            }
            unsafe {
                _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // --- Set up Graphics Pipeline State ---
        unsafe {
            // Input Assembler (IA)
            // 1) build the little arrays on the stack
            let vertex_buffers = [Some(vertex_buffer.clone())];
            let strides = [size_of::<Vertex>() as u32];
            let offsets = [0u32];

            // 2) pass their .as_ptr() into IASetVertexBuffers
            context.IASetVertexBuffers(
                0, // start slot
                1, // number of buffers
                Some(vertex_buffers.as_ptr()),
                Some(strides.as_ptr()),
                Some(offsets.as_ptr()),
            );

            context.IASetInputLayout(&input_layout);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            // Vertex Shader (VS) & Pixel Shader (PS)
            context.VSSetShader(&vertex_shader, None);
            context.PSSetShader(&pixel_shader, None);

            // Rasterizer (RS) - Set Viewport
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect)?;
            let viewport = D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: (rect.right - rect.left) as f32,
                Height: (rect.bottom - rect.top) as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            context.RSSetViewports(Some(&[viewport]));

            // Output Merger (OM)
            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            let blend_factor = [0.0, 0.0, 0.0, 0.0];
            context.OMSetBlendState(&blend_state, Some(&blend_factor), 0xffffffff);
        }

        // --- Clear Render Target ---
        let clear_color = [0.0, 0.0, 0.0, 0.0]; // Transparent black
        unsafe {
            context.ClearRenderTargetView(&rtv, &clear_color);
        }

        // 2) Clear to transparent black each frame
        unsafe {
            context.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 0.0]);
        }

        // --- Draw the Triangle ---
        unsafe {
            context.Draw(vertices.len() as u32, 0);
        }

        // --- Present the Frame ---
        unsafe { swapchain.Present(1, DXGI_PRESENT::default()) }.ok()?;
    }
}

// --- Window Procedure ---
extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

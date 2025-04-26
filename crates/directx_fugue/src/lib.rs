use windows::Win32::Foundation::HMODULE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_BGRA_SUPPORT;
use windows::Win32::Graphics::Direct3D11::D3D11_SDK_VERSION;
use windows::Win32::Graphics::Direct3D11::D3D11CreateDeviceAndSwapChain;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;
use windows::Win32::Graphics::Direct3D11::ID3D11RenderTargetView;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_DESC;
use windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_SCALING_UNSPECIFIED;
use windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED;
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::Win32::Graphics::Dxgi::DXGI_SWAP_CHAIN_DESC;
use windows::Win32::Graphics::Dxgi::DXGI_SWAP_EFFECT_DISCARD;
use windows::Win32::Graphics::Dxgi::DXGI_USAGE_RENDER_TARGET_OUTPUT;
use windows::Win32::Graphics::Dxgi::IDXGISwapChain;

// // Make the window 100% transparent & click-through
// SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA | LWA_COLORKEY)?;

pub fn init_dx11(
    hwnd: HWND,
) -> windows::core::Result<(
    ID3D11Device,
    ID3D11DeviceContext,
    IDXGISwapChain,
    ID3D11RenderTargetView,
)> {
    // 1. Describe the swapchain
    let sd = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: 0,
            Height: 0,
            RefreshRate: Default::default(),
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 1,
        OutputWindow: hwnd,
        Windowed: true.into(),
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    };

    // 2. Create device + swapchain
    let mut device = None;
    let mut context = None;
    let mut swapchain = None;
    unsafe {
        D3D11CreateDeviceAndSwapChain(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None, // pFeatureLevels
            D3D11_SDK_VERSION,
            Some(&sd),
            Some(&mut swapchain),
            Some(&mut device),
            None, // don't care about feature level
            Some(&mut context),
        )?;
    }

    let dev = device.unwrap();
    let ctx = context.unwrap();
    let sc = swapchain.unwrap();

    // 3. Make render target view
    let back_buffer =
        unsafe { sc.GetBuffer::<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>(0)? };
    let mut rtv = None;
    unsafe {
        dev.CreateRenderTargetView(&back_buffer, None, Some(&mut rtv))?;
    }
    let rtv = rtv.unwrap();


    Ok((dev, ctx, sc, rtv))
}

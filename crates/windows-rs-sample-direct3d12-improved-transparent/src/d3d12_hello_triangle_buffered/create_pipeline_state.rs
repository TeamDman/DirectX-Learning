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

use super::compile_shader::compile_shader;

// Create Pipeline State Object (PSO)
pub fn create_pipeline_state(
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
            eprintln!("Warning: shaders.hlsl not found next to executable, using src/shaders.hlsl");
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

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

// Create Vertex Buffer
pub fn create_vertex_buffer(
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
        std::ptr::copy_nonoverlapping(vertices.as_ptr(), data_ptr as *mut Vertex, vertices.len());
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

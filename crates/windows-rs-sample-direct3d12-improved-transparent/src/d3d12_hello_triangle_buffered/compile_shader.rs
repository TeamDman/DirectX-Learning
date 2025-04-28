use windows::core::*;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::Win32::Graphics::Direct3D::*;

// Helper to compile shaders
pub fn compile_shader(
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

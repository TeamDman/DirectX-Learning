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
pub mod d3d12_hello_triangle_buffered;
mod dx_sample;
use dx_sample::build_command_line;
use dx_sample::DXSample;
use dx_sample::SampleCommandLine;

mod sample_runner;
use sample_runner::run_sample;
mod adapter_utils;
use adapter_utils::get_hardware_adapter;

pub fn main() -> Result<()> {
    println!("Starting D3D12 Transparent Triangle Sample...");
    if let Err(e) = run_sample::<d3d12_hello_triangle_buffered::sample::Sample>() {
        eprintln!("Sample execution failed: {:?}", e);
        return Err(e);
    }
    println!("Sample finished successfully.");
    Ok(())
}

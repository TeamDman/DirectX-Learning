#![feature(maybe_uninit_array_assume_init)]

use windows::core::*;
pub mod d3d12_hello_triangle_buffered;
mod dx_sample;

mod sample_runner;
use sample_runner::run_sample;
mod adapter_utils;

pub fn main() -> Result<()> {
    println!("Starting D3D12 Transparent Triangle Sample...");
    if let Err(e) = run_sample::<d3d12_hello_triangle_buffered::sample::Sample>() {
        eprintln!("Sample execution failed: {:?}", e);
        return Err(e);
    }
    println!("Sample finished successfully.");
    Ok(())
}

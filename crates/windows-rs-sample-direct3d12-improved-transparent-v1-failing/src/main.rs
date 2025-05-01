#![feature(maybe_uninit_array_assume_init)]

use eyre::bail;
use tracing::error;
use tracing::info;
pub mod d3d12_hello_triangle_buffered;
use sample_runner::run_sample;

mod adapter_utils;
mod dx_sample;
mod sample_runner;

pub fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::SubscriberBuilder::default()
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_target(false)
        .init();
    info!("Ahoy, world!");
    info!("Starting D3D12 Transparent Triangle Sample...");
    if let Err(e) = run_sample::<d3d12_hello_triangle_buffered::sample::Sample>() {
        error!("Sample execution failed: {:?}", e);
        bail!(e);
    }
    info!("Sample finished successfully.");
    Ok(())
}

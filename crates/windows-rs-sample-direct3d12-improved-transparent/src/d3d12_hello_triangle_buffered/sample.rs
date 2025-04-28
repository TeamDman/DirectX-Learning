use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;

use crate::dx_sample::DXSample;

use super::resources::Resources;
use super::sample_bind_to_window::bind_to_window;
use super::sample_new::new;

pub struct Sample {
    pub dxgi_factory: IDXGIFactory4,
    pub device: ID3D12Device,
    pub resources: Option<Resources>,
    pub window_size: (i32, i32), // Store window size
}

impl DXSample for Sample {
    fn update(&mut self) {}

    fn render(&mut self) -> Result<()> {
        Ok(())
    }

    fn on_key_up(&mut self, _key: u8) {}

    fn on_key_down(&mut self, _key: u8) {}

    fn title(&self) -> String {
        "D3D12 Transparent Triangle (Frame Buffered)".into()
    }

    fn window_size(&self) -> (i32, i32) {
        self.window_size
    }

    fn new(
        command_line: &crate::dx_sample::SampleCommandLine,
    ) -> Result<(Self, Option<IDXGIInfoQueue>)>
    where
        Self: Sized,
    {
        new(command_line)
    }

    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
        bind_to_window(self, hwnd)
    }

    fn on_destroy(&mut self) {
        super::sample_on_destroy::on_destroy(self);
    }
}

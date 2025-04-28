use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dxgi::*;

/// Trait for DirectX samples that provides a common interface
/// for initialization, rendering, and window management.
pub trait DXSample {
    /// Creates a new sample instance with the given command line arguments
    fn new(command_line: &SampleCommandLine) -> Result<(Self, Option<IDXGIInfoQueue>)>
    where
        Self: Sized;

    /// Binds the sample to a window handle
    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()>;
    
    /// Called when the sample is being destroyed for cleanup
    fn on_destroy(&mut self);

    /// Update method called each frame (empty by default)
    fn update(&mut self) {}
    
    /// Render method called each frame (returns Ok by default)
    fn render(&mut self) -> Result<()> {
        Ok(())
    }
    
    /// Called when a key is released
    fn on_key_up(&mut self, _key: u8) {}
    
    /// Called when a key is pressed
    fn on_key_down(&mut self, _key: u8) {}

    /// Returns the window title (default: "DXSample")
    fn title(&self) -> String {
        "DXSample".into()
    }

    /// Returns the window size (default: 640x480)
    fn window_size(&self) -> (i32, i32) {
        (640, 480)
    }
}

/// Command line arguments for sample initialization
#[derive(Clone)]
pub struct SampleCommandLine {
    pub use_warp_device: bool,
}

/// Builds a SampleCommandLine from the process arguments
pub fn build_command_line() -> SampleCommandLine {
    let mut use_warp_device = false;

    for arg in std::env::args() {
        if arg.eq_ignore_ascii_case("-warp") || arg.eq_ignore_ascii_case("/warp") {
            use_warp_device = true;
        }
    }

    SampleCommandLine { use_warp_device }
} 
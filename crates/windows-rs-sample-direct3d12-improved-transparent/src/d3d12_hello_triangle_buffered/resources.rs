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

use super::FRAME_COUNT;

pub struct Resources {
    pub command_queue: ID3D12CommandQueue,
    pub swap_chain: IDXGISwapChain3,
    pub frame_index: u32,
    pub render_targets: [ID3D12Resource; FRAME_COUNT as usize],
    pub rtv_heap: ID3D12DescriptorHeap,
    pub rtv_descriptor_size: u32, // Changed to u32 to match API
    pub viewport: D3D12_VIEWPORT,
    pub scissor_rect: RECT,
    // --- Frame Buffering Changes ---
    pub command_allocators: [ID3D12CommandAllocator; FRAME_COUNT as usize],
    pub fence: ID3D12Fence,
    pub fence_values: [u64; FRAME_COUNT as usize], // Fence value for each frame
    pub fence_event: HANDLE,
    // --- End Frame Buffering Changes ---
    pub root_signature: ID3D12RootSignature,
    pub pso: ID3D12PipelineState,
    pub command_list: ID3D12GraphicsCommandList,
    pub vertex_buffer: ID3D12Resource, // Keep vertex buffer handle
    pub vbv: D3D12_VERTEX_BUFFER_VIEW,
}

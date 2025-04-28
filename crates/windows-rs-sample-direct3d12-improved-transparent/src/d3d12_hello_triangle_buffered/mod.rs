const FRAME_COUNT: u32 = 2; // Use 2 for basic buffering, 3 for potentially smoother results

pub mod compile_shader;
pub mod create_device;
pub mod create_pipeline_state;
pub mod create_root_signature;
pub mod create_vertex_buffer;
pub mod move_to_next_frame;
pub mod populate_command_list;
pub mod resources;
pub mod sample;
pub mod sample_bind_to_window;
pub mod sample_new;
pub mod sample_on_destroy;
pub mod sample_render;
pub mod transition_barrier;
pub mod wait_for_gpu;
pub mod wait_for_gpu_idle;

// Renamed module
use std::mem::MaybeUninit; // Added MaybeUninit

// --- std::mem::MaybeUninit helpers ---
pub trait MaybeUninitHelper<T, const N: usize> {
    fn uninit_array() -> [MaybeUninit<T>; N];
    unsafe fn array_assume_init(array: [MaybeUninit<T>; N]) -> [T; N];
}

impl<T, const N: usize> MaybeUninitHelper<T, N> for MaybeUninit<T> {
    #[inline]
    fn uninit_array() -> [MaybeUninit<T>; N] {
        // Safety: An uninitialized `[MaybeUninit<_>; N]` is valid.
        unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }
    }

    #[inline]
    unsafe fn array_assume_init(array: [MaybeUninit<T>; N]) -> [T; N] {
        // Safety: The caller guarantees that all elements of the array are initialized.
        // Transmute the array pointer. Requires careful handling of memory layout.
        let ptr = &array as *const _ as *const [T; N];
        ptr.read()
        // Note: This assumes the layout of `[MaybeUninit<T>; N]` is identical to `[T; N]`,
        // which is guaranteed by the language.
    }
}

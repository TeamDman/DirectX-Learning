What changed

- Added runtime tearing detection and enabled it when supported.
  - `Renderer` now tracks `allow_tearing` at `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:162`.
  - Support is probed via `IDXGIFactory5::CheckFeatureSupport` in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:547`.
  - The swap chain now includes `DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING` when available in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:514`.
  - `Present` now uses `DXGI_PRESENT_ALLOW_TEARING` when supported in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:330`.

- Removed per-frame vertex allocation jitter.
  - Added a reusable scratch vector in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:180`.
  - `update_scene_vertices` now clears and reuses that buffer instead of allocating a new `Vec` every frame in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:365`.

- Kept the upload vertex buffer persistently mapped.
  - The buffer is mapped once during renderer creation in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:220`.
  - A cached CPU pointer is stored at `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:178`.
  - Each frame now writes directly into the mapped upload memory instead of `Map`/`Unmap` every frame.
  - The buffer is unmapped on shutdown in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:435`.

What I did not change

- I kept your late input sampling order intact because it was already correct: wait on frame latency, wait for the frame fence, then sample cursor and build vertices.
- I did not hide the OS cursor. That would remove the visible comparison, but it would not truly make the software cursor use the OS cursor path.

Validation

- `cargo check` succeeded.
- The workspace still has unrelated warnings in older sample crates, but mod.rs in v7 is clean and compiles.

Reality check

- These changes should shave off avoidable app-side latency and jitter.
- The OS cursor can still appear slightly ahead because Windows renders it on a special lower-latency path that your swap chain does not share.

Natural next steps

1. Run `cargo run window show` in windows-rs-sample-direct3d12-improved-v7 and compare cursor alignment with tearing enabled on your monitor.
2. If you want the visible mismatch gone entirely, I can patch this sample to hide the OS cursor while the app is active.
3. If you want the dragged object to feel “welded” to the pointer, I can help move that visual onto the real OS cursor path instead of the swap chain.

Made changes.
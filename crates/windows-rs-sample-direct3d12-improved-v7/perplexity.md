https://www.perplexity.ai/search/looking-at-input-lag-with-mous-Mlah56ZsRjWq802Wdhw22A

You’re already very close to the “ideal” flip‑model setup; most of the remaining latency will be from vsync/compositor and any extra frame of buffering on your side. Here are the concrete changes I’d make to push it as low as possible.

## 1. Present flags and tearing

Right now you call:

```rust
self.swap_chain.Present(0, DXGI_PRESENT(0)).ok()?;
```

Two things to try:

- Enable tearing on the swap chain: add `DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING` to `Flags` when creating it, and pass `DXGI_PRESENT_ALLOW_TEARING` in `Present` when you are not explicitly vsynced. This can cut one frame of latency on systems that support it.  
- For strictly vsynced mode, keep `SyncInterval = 1` and no tearing; for lowest latency, use `SyncInterval = 0` + tearing and accept possible tearing artifacts.

This is the standard way to get the lowest possible latency on modern Windows with flip model.

## 2. Make sure DWM is actually direct‑flipping

You’re using:

```rust
WS_POPUP | WS_VISIBLE
WS_EX_APPWINDOW | WS_EX_TOPMOST
DXGI_SWAP_EFFECT_FLIP_DISCARD
```

That should be compatible with the “optimized flip / direct flip” path when the window fully covers a monitor. To increase the chances you actually get that path:

- Ensure the window exactly matches the monitor bounds and is truly borderless fullscreen (which you already do).  
- Avoid layered windows or per‑pixel alpha on this swap chain; composited layered windows force the desktop compositor to re‑composite, which adds latency. Use the swap chain alpha of `DXGI_ALPHA_MODE_IGNORE` as you do, not a transparent layered window.

If you ever change this into a layered/transparent window, expect extra latency from DWM that you cannot remove fully.

## 3. Avoid extra frame buffering in your own code

You already:

- Use `FRAME_COUNT: usize = 2`.  
- Call `swap_chain.SetMaximumFrameLatency(1)`.  
- Wait on the frame‑latency waitable object at the top of `render`.  
- Track a fence per frame and wait on that frame’s fence before reusing its command allocator and render target.

This is correct for a one‑frame‑in‑flight model. To make sure you’re not accidentally adding another frame:

- Confirm you never call `wait_for_gpu()` each frame; you only do it in `Drop`, which is good.  
- Ensure `frame_fence_values` are initialized to `0` (they are), and that `wait_for_frame` returns early when `0` (it does). No change needed here.

So from a CPU/GPU pipelining standpoint, you’re already at the minimum.

## 4. Sample input as late as possible in the frame

In your loop:

```rust
self.wait_for_frame_latency()?;
let frame_index = self.swap_chain.GetCurrentBackBufferIndex() as usize;
self.wait_for_frame(frame_index)?;

let cursor_position = self.sample_cursor_position()?;
let vertex_count = self.update_scene_vertices(cursor_position)?;
```

This is already “late” in the frame (after the waitable object and after making sure the GPU has finished with the back buffer). There are two micro‑optimizations:

- Move any non‑essential work (logging, allocations, etc.) before `wait_for_frame_latency`. The window‑proc work already happens outside `render`, so you’re mostly fine.  
- Pre‑allocate the `vertices` vector once and reuse it between frames instead of allocating each time. That removes some CPU jitter, which can subtly affect perceived latency at high frame rates.

The latency gain from this will be small, but it tightens the worst‑case frame time.

## 5. Confirm cursor path vs drawn cursor

You’re using a system cursor (`LoadCursorW` + `SetCursor`), and you are drawing your own cursor graphic. The system cursor is rendered on a special low‑latency hardware/compositor path that often bypasses the normal swap‑chain pipeline, so:

- The system cursor will almost always be slightly *ahead* of anything drawn via your swap chain.  
- This is exactly the effect you’re describing in the Bevy/DirectX thread: the OS cursor and a software‑drawn cursor in a low‑latency flip model can appear “identical”, but as soon as you add any buffering they diverge.

If you want your cursor graphic and your “hit test” to feel identical, you essentially have two options:

- Let the OS cursor be the “truth” (do not draw your own cursor at all, only draw the dragged stuff). You’re already sampling `GetCursorPos` directly, which is as immediate as you can get from user space.  
- Or, hide the OS cursor while your app is active and use only the drawn cursor (set `hCursor` to `null` or call `ShowCursor(FALSE)`), so that there’s no visual reference that bypasses your pipeline.

You can’t make the drawn cursor beat or even quite match the special cursor path’s latency; you can only hide the difference.

## 6. Check for accidental frame rate limiting

Make sure nothing else is limiting your frame rate:

- You pass `Present(0, ...)`, which is good for lowest latency; if you change the first argument to `1`, you reintroduce a guaranteed frame interval and add latency.  
- Ensure you haven’t enabled anything like power‑saver mode or application‑level FPS caps that would reduce your render loop frequency; your `message_loop` is a standard “pump messages then render” loop, which is fine.

If you want to test the raw input‑to‑render latency, temporarily render a very simple solid color or rectangle instead of the full cursor shape and measure how tightly it tracks the OS cursor when you move quickly. That isolates GPU time vs logic time.

***

If you apply:

1. `DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING` + `DXGI_PRESENT_ALLOW_TEARING` when okay with tearing.  
2. Keep the window as a non‑layered, fullscreen borderless flip‑discard swap chain.  
3. Stick with two buffers and `SetMaximumFrameLatency(1)` as you already do.  
4. Sample `GetCursorPos` right before building your vertices and avoid extra per‑frame allocations.  
5. Consider hiding the OS cursor and using only your drawn cursor during interaction.

then you’re essentially at the minimum latency Windows will give you from user‑mode with flip model. Anything left over is compositor / panel / USB latency that you cannot remove in this code.
> now the software cursor feels ahead of the OS cursor, how is that possible?
> specifically I mean that the software cursor is responding sooner than the OS cross cursor, where previously it was behind. It's not a simple misalignment in position, changing velocity and direction the software responds first which I don't understand


What you’re seeing is plausible after the tearing change. The software cursor is not getting future input; it is being displayed on a different schedule than the OS cursor.

The most likely reason is `Present(0, DXGI_PRESENT_ALLOW_TEARING)` in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:330`.

What changed

- Before, your swap chain was effectively more synchronized with the display/compositor.
- Now, with:
  - `DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING` in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:523`
  - `DXGI_PRESENT_ALLOW_TEARING` in `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:326`
- your rendered frame can land immediately, even mid-refresh.

Why that can make software look "ahead"

- The software cursor is sampled very late:
  - `GetCursorPos` at `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:350`
  - vertices updated immediately after at `crates/windows-rs-sample-direct3d12-improved-v7/src/graphics/mod.rs:365`
- Then you present without waiting for vblank.
- That means the monitor can start showing the new software-cursor position right away, as part of a torn frame.

Meanwhile the OS cursor is on a separate path

- The OS cursor is not part of your swap chain.
- Windows may update it through hardware cursor composition or compositor scheduling.
- Those updates do not have to line up exactly with your torn present timing.

So the ordering can flip:

- Previously: software looked behind because your frame was arriving later.
- Now: software can look ahead because your frame is allowed to appear immediately, while the OS cursor graphic is updated on a different cadence.

This is especially noticeable on velocity and direction changes because those are the moments where timing differences are easiest to see.

Important detail

- "Ahead" here does not mean your app knows the mouse position before Windows does.
- It means your app’s already-sampled position is reaching the screen sooner than the visible OS cursor sprite update.

Why this can feel weird

With tearing, the software cursor may be shown:

- in the current refresh
- on only part of the screen at first
- before the next fully settled OS cursor update

That can make the software cursor appear more responsive than the system cursor, even though the OS owns the input first.

What this implies

- If your goal is lowest apparent lag for the software cursor, this is actually consistent with success.
- If your goal is for the software cursor and OS cursor to visually match, tearing makes that harder, not easier.

Best options from here

1. Hide the OS cursor while testing the software cursor.
   - This removes the comparison against a separately timed path.
2. Add a CLI toggle for tearing so you can compare:
   - `vsynced/no-tearing`
   - `sync 0 + tearing`
3. If you want the dragged visual to be perfectly welded to the pointer, move it onto the OS cursor path instead of the swap chain.

If you want, I can patch `v7` next to add a `--tearing` or `--vsync` flag so you can switch modes and confirm this behavior experimentally.
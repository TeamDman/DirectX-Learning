https://github.com/bevyengine/bevy/issues/3317#issuecomment-2529937363


caesay commented on Dec 9, 2024
caesay
on Dec 9, 2024 · edited by caesay
Just adding my 2c (for windows).

I am attempting to migrate my C++ app which uses DX11/12 API's directly.

I set up my swap chain like so:

DXGI_SWAP_CHAIN_DESC1 description{};
description.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
description.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
description.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
description.BufferCount = 2;
description.SampleDesc.Count = 1; // only 1 Anti-Alias sample (more here is higher quality - 4 is maximum safe value)
description.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
description.Flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT; // requires DX 11.2
description.Width = width;
description.Height = height;

// create swap chain. our D2D1 bitmaps will render to this swap chain. 
// note: CreateSwapChainForComposition can be used to create a fully transparent window
DxRef<IDXGISwapChain1> swap1;
HR(dxgi->CreateSwapChainForHwnd(devdxgi, hwnd, &description, nullptr, nullptr, swap1.put()));
HR(swap1->QueryInterface(&swap2));
HR(swap2->SetMaximumFrameLatency(1));

waitobj = swap2->GetFrameLatencyWaitableObject();
What you get at the end is an HANDLE wait object (which can be used with WaitForSingleObjectEx(waitobj, ...)) at the beginning of your render loop.

You wait on this handle at the beginning of your render loop, then capture your input and draw to your buffer. Then this buffer is directly flipped on to the display front buffer by DX.

If your window is borderless fullscreen with DXGI_SWAP_EFFECT_FLIP_DISCARD, DWM will enter an optimised flip model and flip your rendered buffer directly on to the display buffer with no extra copying or delays. What this means is that the frame will be on screen, visible to the user, possibly even just 1ms after retrieving input if drawing simple apps. I draw a graphical cursor in my C++ app, and if you were to record the screen you'd see that the OS cursor is always rendered at precisely the same location as the graphical cursor rendered by my app.

As far as I know, with literally any other combination of settings other than what I've shared above, you will get some frames of latency just from the DWM alone after present, unless you are in fullscreen exclusive.

As far as I can tell, it doesn't seem possible to set up a DX swap chain with this configuration in bevy / wgpu at the moment, and even with the bevy_framepace plugin, there is still significant latency compared to the proper DX flip model I had before.




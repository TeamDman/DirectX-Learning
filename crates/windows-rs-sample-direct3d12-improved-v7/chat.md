
so we would have to create a composite cursor where our object is welded to the cursor by rendering our object to an object we then set as the cursor?

Yes, that is the basic idea if you want zero visible separation.

You would:

Render or rasterize a combined image
Put both the cursor graphic and the dragged object graphic into that image
Set the hotspot to the exact pointer location inside that image
Use that image as the OS cursor
Then the whole thing moves on the OS cursor path, so the object is visually welded to the cursor.

Important constraints

This works best when the dragged visual is small enough to behave like a cursor.
Cursors are not a general-purpose scene graph.
Rebuilding a cursor every frame is possible in principle, but usually awkward and not ideal for rich or large visuals.
Very large drag previews or complex animated objects are usually a poor fit for cursor APIs.
So the practical answer is:

For a small dragged badge, reticle, icon, or window-tab preview: yes, a composite cursor is the right way to get perfect lock.
For a real app window, panel, or large object: usually no, that is not a good cursor candidate. You would normally accept software-render lag or hide the OS cursor and render everything yourself.
A good rule

If the thing can reasonably be thought of as a pointer shape, make it part of the cursor.
If the thing is really a scene object, don’t force it into the cursor system unless you absolutely need perfect visible lock.
On Windows this usually means creating an HCURSOR from ARGB bitmap data, typically via CreateIconIndirect, then applying it with SetCursor and possibly the class cursor as well.

One nuance

If the object has an offset from the pointer, that is still fine.
The hotspot marks the true click point.
The rest of the cursor image can include your object drawn at any relative offset.
So yes: “composite cursor” is the correct concept.

If you want, I can implement that next in v6:

replace the current app-drawn target with a real custom OS cursor
show how to build a composite cursor image with an arbitrary offset
optionally keep the D3D rendering path for other objects while the cursor itself becomes hardware-driven
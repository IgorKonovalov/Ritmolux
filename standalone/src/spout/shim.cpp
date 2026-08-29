// The whole C++ surface of the Spout video-out (Plan 0115 Phase 3, ADR-0125).
//
// Four `extern "C"` functions over one `spoutDX` instance. The seam exists so
// the C++ side stays four functions wide and everything above it is Rust: the
// same discipline plugin-foobar/ runs over the core's C ABI, pointing the other
// way.
//
// Ownership: `lmv_spout_create` returns an opaque heap pointer the caller must
// hand back to `lmv_spout_destroy` exactly once. Nothing here is thread-safe -
// `spoutDX` owns a D3D11 device and its immediate context, so one sender
// belongs to one thread. The Rust side holds it behind a raw pointer, which
// makes the wrapper !Send and !Sync without saying so.
//
// No exception can cross this boundary: `spoutDX` throws nothing, and the only
// allocation is a `new (std::nothrow)` that reports failure as a null return.

#include <new>

#include "SpoutDX.h"

// One sender, heap-allocated so the Rust side holds a pointer-sized opaque
// handle rather than mirroring a C++ layout it cannot see.
struct LmvSpout {
    spoutDX sender;
};

extern "C" {

// Claim a sender name and fix its pixel format. Returns null on any failure,
// which the Rust side turns into a named error.
//
// `SetSenderFormat(DXGI_FORMAT_R8G8B8A8_UNORM)` is mandatory, not a preference.
// `spoutDX` defaults `m_dwFormat` to DXGI_FORMAT_B8G8R8A8_UNORM and `SendImage`
// converts nothing - it is one `UpdateSubresource` of the caller's bytes - so a
// sender left on the default publishes our RGBA frames with red and blue
// swapped, on every frame, with nothing to indicate it.
//
// `width` and `height` are validated and no more: the shared texture and the
// sender registration are created by the first `lmv_spout_send`, because the
// SDK's eager path (`spoutDX::CheckSender`) is protected. So a receiver lists
// this sender from the first frame, not from this call.
//
// The NAME, however, is settled here and is worth reading back: `SetSenderName`
// resolves a collision by incrementing (name, name_1, name_2 ...) when an
// earlier run crashed and left its registration behind, so what a receiver
// lists is what `lmv_spout_name` returns and not necessarily what was asked
// for. Registration applies no further increment on top of that.
LmvSpout *lmv_spout_create(const char *sender_name, unsigned int width, unsigned int height) {
    if (sender_name == nullptr || width == 0 || height == 0) {
        return nullptr;
    }
    LmvSpout *spout = new (std::nothrow) LmvSpout();
    if (spout == nullptr) {
        return nullptr;
    }
    if (!spout->sender.SetSenderName(sender_name)) {
        delete spout;
        return nullptr;
    }
    spout->sender.SetSenderFormat(DXGI_FORMAT_R8G8B8A8_UNORM);
    return spout;
}

// Publish one frame. `rgba` is `width * height * 4` bytes of tight, row-major,
// top-to-bottom RGBA8 - the layout `lmv_core`'s CaptureImage already returns.
// Returns 1 on success, 0 on failure.
//
// Rows need no inversion flag and there is none to pass: `SendImage` writes row
// 0 to row 0. A width or height differing from the last call is handled inside
// `SendImage`, which re-creates the shared texture under the same sender name -
// which is why there is no resize entry point here.
int lmv_spout_send(LmvSpout *spout, const unsigned char *rgba, unsigned int width,
                   unsigned int height) {
    if (spout == nullptr || rgba == nullptr || width == 0 || height == 0) {
        return 0;
    }
    return spout->sender.SendImage(rgba, width, height) ? 1 : 0;
}

// The name the sender is actually registered under, which is what a receiver
// lists and is not necessarily what `lmv_spout_create` was asked for. Points
// into the sender's own storage and stays valid until `lmv_spout_destroy`.
const char *lmv_spout_name(LmvSpout *spout) {
    if (spout == nullptr) {
        return nullptr;
    }
    return spout->sender.GetName();
}

// Release the sender and free the handle. Null is a no-op, and a handle must
// not be passed twice. `~spoutDX` already calls ReleaseSender and
// CloseDirectX11, so the delete is the whole teardown.
void lmv_spout_destroy(LmvSpout *spout) { delete spout; }

} // extern "C"

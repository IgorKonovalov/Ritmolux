// The whole C++ surface of the Spout video-out (Plan 0115 Phase 3, ADR-0125).
//
// Six `extern "C"` functions over one `spoutDX` instance. The seam exists so
// the C++ side stays narrow and everything above it is Rust: the same
// discipline plugin-foobar/ runs over the core's C ABI, pointing the other way.
//
// Ownership: `lmv_spout_create` returns an opaque heap pointer the caller must
// hand back to `lmv_spout_destroy` exactly once. Nothing here is thread-safe -
// `spoutDX` owns a D3D11 device and its immediate context, so one sender
// belongs to one thread. The Rust side holds it behind a raw pointer, which
// makes the wrapper !Send and !Sync without saying so.
//
// No exception can cross this boundary: `spoutDX` throws nothing, and the only
// allocation is a `new (std::nothrow)` that reports failure as a null return.
//
// Policy lives in Rust. The adapter is a parameter here, not a decision.

#include <cstdlib>
#include <new>

#include "SpoutDX.h"

// One sender, heap-allocated so the Rust side holds a pointer-sized opaque
// handle rather than mirroring a C++ layout it cannot see.
struct LmvSpout {
    spoutDX sender;
};

extern "C" {

// How many graphics adapters the machine has, or 0 if that cannot be answered.
// Paired with `lmv_spout_adapter_name` so a caller can name them in a message
// rather than printing an index nobody can interpret.
int lmv_spout_adapter_count() {
    spoutDX probe;
    return probe.GetNumAdapters();
}

// Write adapter `index`'s name into `buffer`, NUL-terminated. Returns 1 on
// success, 0 on failure or a bad index.
int lmv_spout_adapter_name(int index, char *buffer, int length) {
    if (buffer == nullptr || length <= 0) {
        return 0;
    }
    buffer[0] = '\0';
    spoutDX probe;
    return probe.GetAdapterName(index, buffer, length) ? 1 : 0;
}

// Claim a sender name, fix its pixel format, and choose the GPU it will live
// on. Returns null on any failure, which the Rust side turns into a named
// error.
//
// `adapter` IS THE ONE ARGUMENT THAT DECIDES WHETHER THIS WORKS AT ALL ON A
// HYBRID LAPTOP, and it is not a performance preference. A Spout sender shares
// a D3D11 texture by handle; the receiver opens that handle on its own device,
// which succeeds only when both devices are the same physical GPU. A machine
// with an integrated and a discrete GPU hands a plain console process the
// integrated one to save power while the receiving application runs on the
// discrete one, and the receiver then reports only that it could not open the
// shared texture - it cannot see why. Measured on a two-adapter laptop: a
// sender on the integrated GPU is invisible to a receiver on the discrete one,
// and pinning it to the discrete one makes the picture appear unchanged
// otherwise. `-1` means whatever D3D11 would pick, which is right on a
// single-GPU machine and a coin toss anywhere else.
//
// `SetSenderFormat(DXGI_FORMAT_R8G8B8A8_UNORM)` matches the engine's readback
// so `SendImage` - which converts nothing, being one `UpdateSubresource` of the
// caller's bytes - carries them across untouched. Leaving it at the SDK default
// of B8G8R8A8_UNORM would publish every frame with red and blue swapped. It has
// no bearing on whether the texture can be OPENED; that is the adapter's job
// alone, and both formats were checked against a real receiver to establish it.
//
// `width` and `height` are validated and no more: the shared texture and the
// sender registration are created by the first `lmv_spout_send`, because the
// SDK's eager path (`spoutDX::CheckSender`) is protected. So a receiver lists
// this sender from the first frame, not from this call.
//
// The NAME, however, is settled here and is worth reading back: `SetSenderName`
// resolves a collision by incrementing (name, name_1, name_2 ...) when an
// earlier run left its registration behind, so what a receiver lists is what
// `lmv_spout_name` returns and not necessarily what was asked for.
// Registration applies no further increment on top of that.
//
// `LMV_SPOUT_LOG` in the environment turns on the SDK's own verbose log. It is
// read here rather than passed in because it configures the SDK's global
// logger rather than this sender, and it is the only thing that reports what
// the D3D11 layer actually did.
LmvSpout *lmv_spout_create(const char *sender_name, unsigned int width, unsigned int height,
                           int adapter) {
    if (sender_name == nullptr || width == 0 || height == 0) {
        return nullptr;
    }
    LmvSpout *spout = new (std::nothrow) LmvSpout();
    if (spout == nullptr) {
        return nullptr;
    }
    if (std::getenv("LMV_SPOUT_LOG") != nullptr) {
        spoututils::EnableSpoutLog();
        spoututils::SetSpoutLogLevel(spoututils::SPOUT_LOG_VERBOSE);
    }
    // Before anything creates the D3D11 device, which the first send does.
    if (adapter >= 0 && !spout->sender.SetAdapter(adapter)) {
        delete spout;
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
// No conversion and no copy of our own: the sender's format matches these
// bytes, so a frame costs the readback and the upload and nothing between them
// - the two copies ADR-0125 budgeted for.
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

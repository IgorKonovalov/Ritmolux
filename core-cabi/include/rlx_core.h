/*
 * rlx_core.h — C ABI of the Ritmolux core (hand-written,
 * kept in lockstep with core-cabi/src/lib.rs).
 *
 * THIS IS A CONTRACT. The C++ host compiles against this header separately
 * from the Rust crate; changing the shape of this surface is an ADR-worthy
 * event. Bump RLX_ABI_VERSION with any such change and check it at runtime
 * via rlx_abi_version().
 *
 * Threading contract:
 *  - rlx_push_samples: at most one calling thread at a time (the host's
 *    audio / visualisation-stream thread). Real-time safe: lock-free, no
 *    allocation, never blocks; excess samples are dropped when the internal
 *    ring is full.
 *  - All other functions: at most one calling thread at a time (the host's
 *    UI/render thread). rlx_create/rlx_free must not race any other call on
 *    the same handle.
 *  - The audio role and the render role may run concurrently.
 */

#ifndef RLX_CORE_H
#define RLX_CORE_H

#include <stddef.h> /* size_t */
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RLX_ABI_VERSION 6u

/* Result codes (0 success, negative failure). */
#define RLX_OK 0
#define RLX_ERR_INVALID_ARG (-1)
#define RLX_ERR_FORMAT (-2)
#define RLX_ERR_RENDER (-3)
#define RLX_ERR_NO_WINDOW (-4)
#define RLX_ERR_PANIC (-5)
#define RLX_ERR_UNSUPPORTED (-6)

/* Debug flags for rlx_set_debug (ADR-0008). Higher bits reserved, ignored. */
#define RLX_DEBUG_OFF 0u
#define RLX_DEBUG_OVERLAY (1u << 0) /* draw the on-screen diagnostics overlay */

/* Opaque visualizer instance. */
typedef struct RlxHandle RlxHandle;

/*
 * Diagnostics snapshot filled by rlx_get_metrics (ADR-0008). Plain data,
 * caller-allocated - no allocation crosses the ABI. Leads with struct_size +
 * abi_version so later fields append without a version bump: the caller sets
 * struct_size = sizeof(RlxMetrics), the core writes at most that many bytes and
 * stamps what it wrote. Process RSS is deliberately NOT here (host-process
 * owned; each shell reads its own). Layout mirrors the Rust #[repr(C)] struct in
 * core-cabi/src/lib.rs - keep the two in lockstep. Added in ABI v3.
 */
typedef struct RlxMetrics {
    uint32_t struct_size;   /* caller sets sizeof; core stamps what it wrote */
    uint32_t abi_version;   /* == rlx_abi_version() */
    float fps;
    float frame_ms_avg;
    float frame_ms_p99;
    uint64_t frames_total;
    uint64_t frames_dropped;
    uint64_t gpu_bytes;     /* core-tracked GPU bytes (approx; no device mem) */
    uint32_t draw_calls;    /* last frame */
    uint32_t reserved;      /* always 0 */
} RlxMetrics;

#ifdef __cplusplus
/* A layout mismatch with the Rust struct is a silent memory bug, not a compile
 * error (no cbindgen, per ADR-0003); guard it where the C++ shim compiles. */
static_assert(sizeof(RlxMetrics) == 56, "RlxMetrics layout must match core-cabi/src/lib.rs");
#endif

/* Runtime ABI version of the linked core; compare with RLX_ABI_VERSION. */
uint32_t rlx_abi_version(void);

/*
 * Create a visualizer for one PCM stream. Accepted bounds: sample_rate in
 * [8000, 384000], channels in [1, 8]. Returns NULL on rejection or failure.
 */
RlxHandle *rlx_create(uint32_t sample_rate, uint16_t channels);

/* Destroy. The handle must not be used afterwards. NULL is a no-op. */
void rlx_free(RlxHandle *handle);

/*
 * Push interleaved 32-bit float samples. sample_count is the number of
 * floats and must be a whole number of frames (multiple of channels).
 */
int32_t rlx_push_samples(RlxHandle *handle, const float *samples,
                         uint32_t sample_count);

/*
 * Attach the native window to render into, with its current client size in
 * physical pixels. On Windows pass the HWND. The window must outlive the
 * handle (or be detached by freeing the handle first).
 */
int32_t rlx_attach_window(RlxHandle *handle, void *hwnd, uint32_t width,
                          uint32_t height);

/* Analyze pending audio and draw one frame. Call at display cadence. Exactly
 * equivalent to rlx_render_dt(handle, 1.0f / 60.0f) - the fixed-step wrapper for
 * a host that has no real elapsed time to supply. */
int32_t rlx_render(RlxHandle *handle);

/*
 * Analyze pending audio and draw one frame, advancing the simulation by
 * dt_seconds of real time. Call at display cadence with the measured elapsed
 * time since the previous frame, so a feedback simulation runs at the same
 * wall-clock rate on any refresh; core never reads a clock. rlx_render is the
 * 1/60 s wrapper over this. Added in ABI v4.
 */
int32_t rlx_render_dt(RlxHandle *handle, float dt_seconds);

/* Notify of a window client-size change (physical pixels). */
int32_t rlx_resize(RlxHandle *handle, uint32_t width, uint32_t height);

/* Advance to the next built-in scene (same roster as the standalone). */
int32_t rlx_cycle_scene(RlxHandle *handle);

/*
 * Seed `path_utf8` (a directory, `path_len` bytes of UTF-8, not
 * NUL-terminated) with the embedded curated presets, writing only files that
 * are absent (never overwriting user edits), then load every valid preset
 * found there and install it as this handle's preset set. rlx_cycle_scene then
 * cycles the loaded set. Returns the number of presets loaded (>= 0), or a
 * negative RLX_ERR_* (invalid arg on a null handle/path or non-UTF-8 path). A
 * directory with no valid presets keeps the current set. The seed step is
 * idempotent - safe to call once on every host start. Added in ABI v2.
 */
int32_t rlx_load_presets(RlxHandle *handle, const uint8_t *path_utf8,
                         size_t path_len);

/*
 * Snapshot the installed preset roster: every name, in roster order, written
 * into `buf` as UTF-8 with a single 0x00 after each. Returns the total byte
 * count the full list needs (>= 0), or a negative RLX_ERR_*.
 *
 * CALL TWICE TO SIZE IT: pass buf_len = 0 (or buf = NULL) to learn the count,
 * allocate, call again. If buf_len is smaller than the needed count NOTHING is
 * written - a short buffer never yields a partial list. No allocation crosses
 * the ABI.
 *
 * out_current_index may be NULL (then skipped); otherwise it receives the index
 * the show is GOING to - the dissolve's target while a transition is in flight,
 * matching rlx_cycle_scene's convention - or -1 on an empty roster. It is filled
 * on every successful call, sizing calls included.
 *
 * Returns RLX_ERR_NO_WINDOW before rlx_attach_window: the roster installs at
 * attach. Indices are SNAPSHOT-SCOPED - meaningful only against this same
 * handle's roster with no rlx_load_presets in between. Added in ABI v6.
 */
int32_t rlx_get_presets(RlxHandle *handle, uint8_t *buf, size_t buf_len,
                        int32_t *out_current_index);

/*
 * Switch to the preset at `index` - an absolute position in the list this same
 * handle's rlx_get_presets reported - dissolving rather than cutting, exactly as
 * rlx_cycle_scene does.
 *
 * Returns RLX_OK; RLX_ERR_INVALID_ARG on a null handle or an index that is
 * negative or past the end of the roster (nothing changes); RLX_ERR_NO_WINDOW
 * before rlx_attach_window. Added in ABI v6.
 */
int32_t rlx_select_preset(RlxHandle *handle, int32_t index);

/*
 * Announce the currently playing track: the core fades a banner in over the
 * visuals, holds it a few seconds, and fades it out (ADR-0110). The host says
 * what is playing and never when to stop.
 *
 * `utf8` is `len` bytes of UTF-8 text, NOT NUL-terminated, conventionally
 * "artist - title" - the core splits on the first " - ". THE CORE COPIES THE
 * BYTES BEFORE RETURNING and never retains the pointer, so the caller may free
 * or reuse the buffer immediately.
 *
 * Returns RLX_OK, or RLX_ERR_INVALID_ARG on a null handle, a null pointer, a
 * zero length, or non-UTF-8 bytes; RLX_ERR_NO_WINDOW before a window is
 * attached. Setting the string that is already set does nothing, so a host may
 * call this on every metadata notification. There is no clear call and none is
 * needed - the banner removes itself.
 *
 * NEVER call this from the visualisation_stream thread: the copy allocates,
 * which that thread must never do. The host's playback/UI callback is the right
 * caller. Added in ABI v5.
 */
int32_t rlx_set_now_playing(RlxHandle *handle, const uint8_t *utf8, size_t len);

/*
 * Set the debug flag set on the handle (RLX_DEBUG_*). Idempotent and cheap;
 * callable at any time from the render-thread role, including before a window is
 * attached (the flags apply when the renderer is created). RLX_DEBUG_OVERLAY at
 * create time can also be seeded from the RLX_DEBUG_OVERLAY environment
 * variable. Added in ABI v3.
 */
int32_t rlx_set_debug(RlxHandle *handle, uint32_t flags);

/*
 * Fill *out (caller-allocated) with the current diagnostics snapshot. Set
 * out->struct_size = sizeof(RlxMetrics) before calling. Returns RLX_OK, or
 * RLX_ERR_INVALID_ARG on a null handle/out. No allocation crosses the ABI; safe
 * to poll every frame or once a second. Added in ABI v3.
 */
int32_t rlx_get_metrics(RlxHandle *handle, RlxMetrics *out);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* RLX_CORE_H */

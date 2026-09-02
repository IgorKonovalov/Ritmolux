// The shared surface of the foobar2000 component: the one visualizer session,
// the two window globals the Win32 callback model needs, and the free functions
// the four translation units call across their boundaries.
//
// Everything here lives in `namespace rlx` rather than an anonymous one. An
// anonymous namespace gives internal linkage, which is right for a single
// translation unit and impossible across four -- so the scoping stays and the
// linkage does not.

#pragma once

#include "SDK/foobar2000.h"

#include <string>
#include <vector>

#include <windows.h>

#include "rlx_core.h"

namespace rlx {

constexpr UINT_PTR kRenderTimer = 1;
// ~66 fps pump; actual pacing is vsync inside the core's present.
constexpr UINT kRenderTimerMs = 15;
// Reduced cadence (~6-7 fps) while paused/stopped: keeps scenes alive without
// pegging the GPU on idle playback.
constexpr UINT kIdleTimerMs = 150;
// A non-owning host polls on this timer to take over once the session frees
// (owning panel removed, pop-out closed) and to keep its placeholder painted.
constexpr UINT_PTR kArbitrationTimer = 2;
constexpr UINT kArbitrationMs = 400;
// The owner's self-heal tick. sync_render_timer() can legitimately decide to
// KILL the render timer (hidden host), and once it has, the render timer is no
// longer there to re-arm itself - a missed or spurious "shown" notification
// therefore left the panel frozen on its last presented frame FOREVER, with
// foobar still playing. This timer is the only one that always runs while a
// host owns the session, so cadence can always recover. Measured cost: one
// wakeup every half second that usually returns without touching anything.
constexpr UINT_PTR kWatchdogTimer = 3;
constexpr UINT kWatchdogMs = 500;
// Context-menu command ids (window-local; not foobar menu GUIDs).
constexpr UINT kMenuNextScene = 1001;
constexpr UINT kMenuToggleOverlay = 1002;
constexpr UINT kMenuReloadPresets = 1003;
constexpr UINT kMenuOpenPresetDir = 1004;
// The Preset submenu's ids: base + roster index. A reserved RANGE rather than a
// handful of constants, so the ids stay disjoint from the fixed items above
// however far the library grows. The cap bounds the range (and a menu nobody
// could use); the roster is ~40 presets today.
constexpr UINT kMenuPresetBase = 2000;
constexpr size_t kMenuPresetMax = 900;
// Read this far behind "now": visualisation data close to the playback head
// may not be decoded yet.
constexpr double kReadBehindSec = 0.05;
constexpr wchar_t kWindowClass[] = L"rlx_foobar_window";

// The single shared visualizer session (main thread only). Exactly one host
// window (pop-out or a panel) owns it at a time; only the owner holds the
// RlxHandle, the stream and the render timer, so there is only ever one wgpu
// surface.
struct VizSession {
    HWND owner = nullptr; // host window currently driving the core
    RlxHandle *handle = nullptr;
    visualisation_stream::ptr stream;
    double cursor = 0.0;
    uint32_t rate = 0;
    uint16_t channels = 0;
    bool visible = true;   // is the owner host currently shown?
    // The surface was attached before the host had a real (non-zero) client
    // size - a Default UI panel is created 0x0, then sized by the layout. Such
    // a surface never presents; recreate it once the first real WM_SIZE lands.
    // Without this, a panel added mid-playback stays blank until an audio-format
    // change happens to recreate the handle (see reattach_at_current_size).
    bool needs_reattach = false;
    UINT timer_ms = 0;     // current render-timer interval (0 = not running)
    ULONGLONG last_log_ms = 0;    // last diagnostics-log write (GetTickCount64)
    LONGLONG last_render_qpc = 0; // QPC at the previous render (0 = first frame)
    // Set once at component init from the runtime ABI handshake
    // (`rlx_abi_version`). Preset loading (v2, ADR-0006) and diagnostics (v3,
    // ADR-0008) are skipped when the linked core is older than the version this
    // shim was built against.
    //
    // A member rather than a file-scope global: it is a fact about the session
    // the shim runs, every reader already has the session in hand, and two of
    // the four files would otherwise need it declared across the seam.
    bool abi_ok = false;
    // Current debug flags (ADR-0008), seeded from RLX_DEBUG_OVERLAY at init and
    // flipped by the context-menu toggle. Applied to each handle on creation and
    // live via `rlx_set_debug`, so the plugin -- not the core's env read -- is
    // the authority once running.
    uint32_t debug_flags = RLX_DEBUG_OFF;
    // The diagnostics log, opened on the first sample and kept.
    //
    // **Opened once, not once per sample.** The header is then written where it
    // belongs -- on the open that created the file, the only moment the file is
    // known to be new. A sampler that reopens cannot tell a file it just
    // created from one it appended to a second ago without stat'ing it again.
    // Closed by `release`, so a session that ends releases it.
    FILE *log = nullptr;

    void destroy_handle();
    void ensure_handle(uint32_t rate, uint16_t channels);
    // Recreate the handle so its wgpu surface re-attaches at the owner's current
    // (now real) client size. Used once when the initial attach happened at a
    // degenerate size (needs_reattach); preserves the current stream format.
    void reattach_at_current_size();
    void push_converted(const audio_sample *data, size_t total, unsigned channels);
    void pump();
    // Real seconds since the previous render, for the frame-rate-independent
    // simulation (C ABI v4 rlx_render_dt). Measured with QueryPerformanceCounter
    // on the render thread (main-thread only); the core never reads a clock. The
    // first frame and any long stall clamp to a small step so a hitch can't jump
    // the simulation.
    float measure_dt();
    // Append a diagnostics sample (rlx_get_metrics) to the plugin log at ~1 Hz.
    // Main-thread only (render timer), never the audio path. No-op pre-v3 core.
    void maybe_log_metrics();
    // Re-arm (or stop) the render timer to match visibility and playback: full
    // rate while playing and visible, reduced when paused/stopped, off when
    // hidden. Idempotent - only touches the timer when the cadence changes.
    void sync_render_timer();

    // Take ownership for `host` if the session is free. On success the core
    // handle + stream + render timer are live on `host`; returns false (no
    // core created) if another host already owns the session.
    bool claim(HWND host);
    // Release ownership held by `host` (no-op if `host` is not the owner),
    // freeing the handle + stream and stopping the timer.
    void release(HWND host);
    // Close the diagnostics log if it is open. Called by `release`.
    void close_log();
};

// The single shared visualizer session (main thread only).
extern VizSession g_session;

// Tracks the pop-out window independently of session ownership, so the View
// command can bump an existing pop-out and `on_quit` can tear it down.
extern HWND g_popup_hwnd;

// %APPDATA%\Ritmolux - the app dir the standalone shares. Empty on
// failure. The diagnostics log lives here, next to the shared presets dir.
std::wstring plugin_app_dir_w();

// One reading of the core's installed roster (C ABI v6, ADR-0117).
//
// The names are the CORE's, not the folder's, and that is the whole point: a
// malformed .toml a user dropped in the directory is absent here, so the menu
// cannot offer a preset that would not load. `current` is the index the show is
// going to - the dissolve's target - so a checkmark follows the click rather
// than the fade.
struct PresetSnapshot {
    std::vector<std::string> names; // UTF-8, roster order
    int32_t current = -1;           // -1 = empty roster / not read
};

// ---- presets.cpp -------------------------------------------------------

std::wstring menu_label(const std::string &name);
void load_presets_into(RlxHandle *h);
bool read_preset_snapshot(RlxHandle *h, PresetSnapshot &out);
bool select_preset_named(RlxHandle *h, const std::string &name);
void remember_current_preset(RlxHandle *h);
void restore_remembered_preset(RlxHandle *h);
void reload_presets_keeping_selection(RlxHandle *h);
void open_preset_folder();
bool env_overlay_on();

// ---- viz_session.cpp ---------------------------------------------------

bool host_is_showing(HWND host);
void set_host_visibility(HWND host, bool vis);
void announce(const metadb_handle_ptr &track);
void announce_current();

// ---- host_window.cpp ---------------------------------------------------

void ensure_window_class();
void open_window();

} // namespace rlx

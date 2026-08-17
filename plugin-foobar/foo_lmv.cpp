// foo_lmv — foobar2000 visualization component for light-music-visualizer.
//
// A thin shim per ADR-0001: pulls PCM from foobar's visualisation_stream,
// forwards it across lmv-core's C ABI, and hosts the core's wgpu output in a
// plain Win32 window. All logic lives in the Rust core; this file only
// bridges foobar2000 conventions to the ABI in core-cabi/include/lmv_core.h.
//
// Two entry points share ONE core instance: a View-menu pop-out window and a
// Default UI panel (ui_element). Both are "host windows" that claim a single
// global VizSession; the session holds the sole LmvHandle + visualisation
// stream + render timer, and only its current owner drives the core. A second
// host cannot create a second wgpu surface (the "lightweight / one surface"
// value). Placeholder painting for non-owners lands in a later phase.
//
// Threading: everything here runs on the foobar2000 main thread (menu
// command, window messages, render timer), which trivially satisfies the
// ABI's two-role threading contract.

#include "SDK/foobar2000.h"
#include "SDK/ui_element.h"
#include "SDK/console.h"

#include <cstdio>
#include <string>
#include <vector>

#include <windows.h>
#include <windowsx.h> // GET_X_LPARAM / GET_Y_LPARAM
#include <shellapi.h> // ShellExecuteW (Open presets folder)

#include "lmv_core.h"

// Component version, single-sourced from the workspace Cargo version (ADR-0025).
// build.ps1 generates foo_lmv_version.h into build/ (on the include path) from
// root Cargo.toml's [workspace.package] version; the fallback keeps a compile
// outside build.ps1 (editor tooling, a stray direct cl) building as 0.0.0-dev.
#if __has_include("foo_lmv_version.h")
#  include "foo_lmv_version.h"
#endif
#ifndef FOO_LMV_VERSION
#  define FOO_LMV_VERSION "0.0.0-dev"
#endif

// foobar2000 x64 uses 64-bit audio_sample; lmv-core takes f32, so chunks are
// converted through a fixed buffer on the way in (see push_converted).

DECLARE_COMPONENT_VERSION(
    "Light Music Visualizer", FOO_LMV_VERSION,
    "Light Music Visualizer\n"
    "Audio-reactive scenes - fragment fields, particle swarm, line geometry "
    "(curves / L-systems / star patterns), reaction-diffusion and attractor "
    "flows - rendered by the shared lmv-core Rust engine (wgpu).\n"
    "Dockable as a Default UI panel or opened from the View menu. "
    "Space cycles scenes; right-click to pick one by name, reload the preset "
    "folder, or open it.");
VALIDATE_COMPONENT_FILENAME("foo_lmv.dll");

namespace {

// GUIDs owned by this component - never reuse.
constexpr GUID kGuidLmvMenu = {
    0x8f7c2a1e, 0x94d3, 0x4b6a, {0x9c, 0x1f, 0x27, 0x5e, 0x88, 0x3a, 0xd4, 0x61}};
constexpr GUID kGuidLmvElement = {
    0x2d9b4f7c, 0x6e21, 0x4a83, {0xb5, 0x0c, 0x1a, 0x77, 0x3e, 0x08, 0xc2, 0x54}};

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
constexpr wchar_t kWindowClass[] = L"lmv_foobar_window";

// The single shared visualizer session (main thread only). Exactly one host
// window (pop-out or a panel) owns it at a time; only the owner holds the
// LmvHandle, stream and render timer, so there is only ever one wgpu surface.
struct VizSession {
    HWND owner = nullptr; // host window currently driving the core
    LmvHandle *handle = nullptr;
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

    void destroy_handle();
    void ensure_handle(uint32_t rate, uint16_t channels);
    // Recreate the handle so its wgpu surface re-attaches at the owner's current
    // (now real) client size. Used once when the initial attach happened at a
    // degenerate size (needs_reattach); preserves the current stream format.
    void reattach_at_current_size();
    void push_converted(const audio_sample *data, size_t total, unsigned channels);
    void pump();
    // Real seconds since the previous render, for the frame-rate-independent
    // simulation (C ABI v4 lmv_render_dt). Measured with QueryPerformanceCounter
    // on the render thread (main-thread only); the core never reads a clock. The
    // first frame and any long stall clamp to a small step so a hitch can't jump
    // the simulation.
    float measure_dt();
    // Append a diagnostics sample (lmv_get_metrics) to the plugin log at ~1 Hz.
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
};

VizSession g_session;

// Tracks the pop-out window independently of session ownership, so the View
// command can bump an existing pop-out and on_quit can tear it down.
HWND g_popup_hwnd = nullptr;

// Set once at component init from the runtime ABI handshake (lmv_abi_version).
// Preset loading (v2, ADR-0006) and diagnostics (v3, ADR-0008) are skipped when
// the linked core is older than the version this shim was built against.
bool g_abi_ok = false;

// Current debug flags (ADR-0008), seeded from LMV_DEBUG_OVERLAY at init and
// flipped by the context-menu toggle. Applied to each handle on creation and
// live via lmv_set_debug, so the plugin - not the core's env read - is the
// authority once running.
uint32_t g_debug_flags = LMV_DEBUG_OFF;

// %APPDATA%\light-music-visualizer - the app dir the standalone shares. Empty on
// failure. The diagnostics log lives here, next to the shared presets dir.
std::wstring plugin_app_dir_w() {
    const DWORD need = GetEnvironmentVariableW(L"APPDATA", nullptr, 0);
    if (need == 0) return {};
    std::wstring wide(need, L'\0');
    const DWORD got = GetEnvironmentVariableW(L"APPDATA", wide.data(), need);
    if (got == 0 || got >= need) return {};
    wide.resize(got);
    wide += L"\\light-music-visualizer";
    return wide;
}

// The shared per-user preset directory: %APPDATA%\light-music-visualizer\presets
// - the exact path the standalone seeds and watches, so both frontends share one
// library. Empty on failure (the core then keeps its embedded defaults).
//
// This is the last independent copy of that path. The two Rust frontends (the
// app and the shot CLI) now share one resolver in standalone/src/lib.rs, which
// also honors the LMV_PRESET_DIR override (ADR-0014); this shim deliberately
// does not - it resolves the same %APPDATA% directory on its own, and honoring
// the override here is a documented followup. Keep the literals above in step
// with APP_DIR_NAME in that module.
std::wstring preset_dir_w() {
    const std::wstring app = plugin_app_dir_w();
    if (app.empty()) return {};
    return app + L"\\presets";
}

// UTF-16 -> UTF-8, for the path the ABI takes as bytes.
std::string narrow(const std::wstring &wide) {
    if (wide.empty()) return {};
    const int len =
        WideCharToMultiByte(CP_UTF8, 0, wide.c_str(),
                            static_cast<int>(wide.size()), nullptr, 0, nullptr,
                            nullptr);
    if (len <= 0) return {};
    std::string out(static_cast<size_t>(len), '\0');
    WideCharToMultiByte(CP_UTF8, 0, wide.c_str(), static_cast<int>(wide.size()),
                        out.data(), len, nullptr, nullptr);
    return out;
}

// UTF-8 -> UTF-16, for the roster names the menu draws. Preset names are
// author-supplied text and need not be ASCII, so this is not decoration.
std::wstring widen(const std::string &utf8) {
    if (utf8.empty()) return {};
    const int len = MultiByteToWideChar(CP_UTF8, 0, utf8.data(),
                                        static_cast<int>(utf8.size()), nullptr, 0);
    if (len <= 0) return {};
    std::wstring out(static_cast<size_t>(len), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, utf8.data(), static_cast<int>(utf8.size()),
                        out.data(), len);
    return out;
}

// A menu label from a preset name. The only transformation is doubling '&',
// which AppendMenuW would otherwise eat as an accelerator prefix - a preset
// called "black & white" must not display as "black  white" with a underlined W.
std::wstring menu_label(const std::string &name) {
    std::wstring out;
    for (const wchar_t c : widen(name)) {
        out.push_back(c);
        if (c == L'&') out.push_back(c);
    }
    return out;
}

std::string resolve_preset_dir_utf8() { return narrow(preset_dir_w()); }

// Seed + load the shared preset library into `h` over the C ABI. No-op if the
// ABI handshake failed or the directory can't be resolved. Runs on the main
// thread (menu/timer), never the audio callback, so its disk I/O is fine.
void load_presets_into(LmvHandle *h) {
    if (!g_abi_ok || h == nullptr) return;
    const std::string dir = resolve_preset_dir_utf8();
    if (dir.empty()) return;
    lmv_load_presets(h, reinterpret_cast<const uint8_t *>(dir.data()),
                     dir.size());
}

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

// Fill `out` from `h`. False (and an empty snapshot) when the ABI is too old,
// no window is attached yet, or the roster is empty - all cases where the menu
// simply omits the Preset submenu rather than showing an empty one.
bool read_preset_snapshot(LmvHandle *h, PresetSnapshot &out) {
    out.names.clear();
    out.current = -1;
    if (!g_abi_ok || h == nullptr) return false;
    // Call twice: size, then fill. Nothing is written when the buffer is short,
    // so a roster that grew between the two calls yields no names rather than a
    // truncated list - and it cannot, since both run on this thread.
    const int32_t needed = lmv_get_presets(h, nullptr, 0, &out.current);
    if (needed <= 0) return false;
    std::vector<uint8_t> buf(static_cast<size_t>(needed));
    if (lmv_get_presets(h, buf.data(), buf.size(), &out.current) != needed) {
        return false;
    }
    size_t start = 0;
    for (size_t i = 0; i < buf.size(); ++i) {
        if (buf[i] != 0) continue;
        out.names.emplace_back(reinterpret_cast<const char *>(buf.data()) + start,
                               i - start);
        start = i + 1;
    }
    return !out.names.empty();
}

// Select the preset called `name` against a FRESH snapshot; returns whether it
// was found. Indices are snapshot-scoped (ADR-0117), so the lookup and the
// select must have nothing between them - which is exactly why this reads the
// roster itself instead of taking an index from a caller who read it earlier.
bool select_preset_named(LmvHandle *h, const std::string &name) {
    PresetSnapshot snap;
    if (name.empty() || !read_preset_snapshot(h, snap)) return false;
    for (size_t i = 0; i < snap.names.size(); ++i) {
        if (snap.names[i] != name) continue;
        return lmv_select_preset(h, static_cast<int32_t>(i)) == LMV_OK;
    }
    return false; // a name that is gone leaves the roster where it is
}

// Re-scan the shared folder so a file dropped into it appears, without
// restarting foobar - the explicit alternative to a file watcher (Plan 0107's
// interview decision).
//
// The re-selection is not a nicety: the core's set_presets keeps the roster
// INDEX, not the name, so a new file sorting before the current one would
// silently move the show to a different look. Reloading also re-seeds the
// running scene's simulation state (set_presets reconfigures the active scene),
// which is accepted here because reload is something the user asked for.
void reload_presets_keeping_selection(LmvHandle *h) {
    if (!g_abi_ok || h == nullptr) return;
    PresetSnapshot before;
    std::string keep;
    if (read_preset_snapshot(h, before) && before.current >= 0 &&
        static_cast<size_t>(before.current) < before.names.size()) {
        keep = before.names[static_cast<size_t>(before.current)];
    }
    load_presets_into(h);
    select_preset_named(h, keep);
}

// Show the shared preset folder in Explorer - the only thing that makes the
// drop-a-file loop discoverable, since the seeding is silent.
void open_preset_folder() {
    const std::wstring dir = preset_dir_w();
    if (dir.empty()) return;
    // Seeding creates it, but a session that never got that far should still
    // land in a real folder rather than an error box. Both levels, because
    // CreateDirectoryW does not create parents.
    CreateDirectoryW(plugin_app_dir_w().c_str(), nullptr);
    CreateDirectoryW(dir.c_str(), nullptr);
    ShellExecuteW(nullptr, L"open", dir.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
}

// True when LMV_DEBUG_OVERLAY is set to a truthy value (1/true/on/yes). Seeds
// the overlay default; the core reads the same var at lmv_create, but the plugin
// tracks it too so the menu toggle and handle re-creation stay consistent.
bool env_overlay_on() {
    wchar_t buf[16] = {};
    const DWORD got = GetEnvironmentVariableW(L"LMV_DEBUG_OVERLAY", buf, 16);
    if (got == 0 || got >= 16) return false;
    return _wcsicmp(buf, L"1") == 0 || _wcsicmp(buf, L"true") == 0 ||
           _wcsicmp(buf, L"on") == 0 || _wcsicmp(buf, L"yes") == 0;
}

void VizSession::destroy_handle() {
    if (handle) {
        lmv_free(handle);
        handle = nullptr;
    }
    rate = 0;
    channels = 0;
    needs_reattach = false;
}

// (Re)create the core handle for a stream format and attach the owner window.
// Called with the default format on claim so scenes render even in silence,
// then again whenever the track's format differs. Requires `owner` set.
void VizSession::ensure_handle(uint32_t new_rate, uint16_t new_channels) {
    if (handle != nullptr && new_rate == rate && new_channels == channels) return;
    destroy_handle();
    LmvHandle *h = lmv_create(new_rate, new_channels);
    if (h == nullptr) return; // format outside core bounds - skip
    RECT rc = {};
    GetClientRect(owner, &rc);
    const uint32_t w = static_cast<uint32_t>(rc.right - rc.left);
    const uint32_t ht = static_cast<uint32_t>(rc.bottom - rc.top);
    if (lmv_attach_window(h, owner, w ? w : 1, ht ? ht : 1) != LMV_OK) {
        lmv_free(h);
        return;
    }
    handle = h;
    rate = new_rate;
    channels = new_channels;
    // If the owner had no real client area yet (a panel is created 0x0 then
    // sized), this surface was attached at the 1x1 fallback and will not
    // present. Flag it so the first real WM_SIZE recreates the handle at the
    // correct size instead of merely resizing the dead surface.
    needs_reattach = (w == 0 || ht == 0);
    // Every freshly created handle loads the shared curated + user library so
    // Next-scene cycles it. Called here (not only on claim) so a mid-playback
    // format change, which recreates the handle, does not drop the presets.
    load_presets_into(h);
    // Re-apply the current debug flags so a menu-toggled overlay survives a
    // handle re-creation (mid-playback format change); the core otherwise
    // re-seeds from the env at create.
    if (g_abi_ok) lmv_set_debug(h, g_debug_flags);
}

void VizSession::reattach_at_current_size() {
    if (owner == nullptr || handle == nullptr) return;
    const uint32_t r = rate;
    const uint16_t c = channels;
    destroy_handle();    // clears rate/channels so ensure_handle re-attaches
    ensure_handle(r, c); // fresh lmv_create + lmv_attach_window at the real size
}

// Append a diagnostics sample to %APPDATA%\light-music-visualizer\
// plugin-diagnostics.log at ~1 Hz. RSS is not logged: it is host-process-owned
// (ADR-0008) and would mean "all of foobar", not "us".
void VizSession::maybe_log_metrics() {
    if (!g_abi_ok || handle == nullptr) return;
    const ULONGLONG now = GetTickCount64();
    if (last_log_ms != 0 && now - last_log_ms < 1000) return;
    last_log_ms = now;

    LmvMetrics m = {};
    m.struct_size = sizeof(LmvMetrics);
    if (lmv_get_metrics(handle, &m) != LMV_OK) return;

    const std::wstring dir = plugin_app_dir_w();
    if (dir.empty()) return;
    CreateDirectoryW(dir.c_str(), nullptr); // parent (%APPDATA%) already exists
    const std::wstring path = dir + L"\\plugin-diagnostics.log";

    const bool is_new = GetFileAttributesW(path.c_str()) == INVALID_FILE_ATTRIBUTES;
    FILE *f = nullptr;
    if (_wfopen_s(&f, path.c_str(), L"a") != 0 || f == nullptr) return;
    if (is_new) {
        fprintf(f, "unix_ms\tfps\tframe_ms_avg\tframe_ms_p99\tframes_total"
                   "\tframes_dropped\tgpu_bytes\tdraw_calls\n");
    }
    FILETIME ft = {};
    GetSystemTimeAsFileTime(&ft);
    const ULONGLONG ft100 =
        (static_cast<ULONGLONG>(ft.dwHighDateTime) << 32) | ft.dwLowDateTime;
    // FILETIME is 100 ns ticks since 1601; convert to Unix milliseconds.
    const ULONGLONG unix_ms = (ft100 - 116444736000000000ULL) / 10000ULL;
    fprintf(f, "%llu\t%.1f\t%.3f\t%.3f\t%llu\t%llu\t%llu\t%u\n",
            static_cast<unsigned long long>(unix_ms), m.fps, m.frame_ms_avg,
            m.frame_ms_p99, static_cast<unsigned long long>(m.frames_total),
            static_cast<unsigned long long>(m.frames_dropped),
            static_cast<unsigned long long>(m.gpu_bytes),
            static_cast<unsigned>(m.draw_calls));
    fclose(f);
}

// Convert audio_sample (double on x64 builds) to the f32 the ABI takes,
// through a fixed buffer - no per-tick allocation.
void VizSession::push_converted(const audio_sample *data, size_t total,
                                unsigned chans) {
    static float conv[8192];
    const size_t cap = (sizeof(conv) / sizeof(float)) / chans * chans;
    size_t off = 0;
    while (off < total && cap != 0) {
        const size_t n = (total - off < cap) ? (total - off) : cap;
        for (size_t i = 0; i < n; ++i) {
            conv[i] = static_cast<float>(data[off + i]);
        }
        lmv_push_samples(handle, conv, static_cast<uint32_t>(n));
        off += n;
    }
}

float VizSession::measure_dt() {
    LARGE_INTEGER now;
    LARGE_INTEGER freq;
    QueryPerformanceCounter(&now);
    QueryPerformanceFrequency(&freq);
    float dt = 1.0f / 60.0f; // first-frame fallback (matches the fixed step)
    if (last_render_qpc != 0 && freq.QuadPart > 0) {
        dt = static_cast<float>(static_cast<double>(now.QuadPart - last_render_qpc) /
                                static_cast<double>(freq.QuadPart));
        // Clamp a long stall so the simulation steps forward, never leaps.
        if (dt > 0.25f) dt = 0.25f;
        if (dt < 0.0f) dt = 1.0f / 60.0f;
    }
    last_render_qpc = now.QuadPart;
    return dt;
}

void VizSession::pump() {
    if (stream.is_empty()) return;
    double now = 0.0;
    if (!stream->get_absolute_time(now)) return;
    const double end = now - kReadBehindSec;
    // Resync after open, seek, or a long stall - never chase a huge backlog.
    if (cursor <= 0.0 || cursor > end || end - cursor > 0.5) {
        cursor = end;
        return;
    }
    if (end <= cursor) return;

    audio_chunk_impl chunk;
    if (stream->get_chunk_absolute(chunk, cursor, end - cursor)) {
        const unsigned chunk_rate = chunk.get_sample_rate();
        const unsigned chunk_channels = chunk.get_channels();
        const t_size samples = chunk.get_sample_count() * chunk_channels;
        if (chunk_rate != 0 && chunk_channels != 0 && samples != 0) {
            ensure_handle(static_cast<uint32_t>(chunk_rate),
                          static_cast<uint16_t>(chunk_channels));
            if (handle != nullptr) {
                push_converted(chunk.get_data(), samples, chunk_channels);
            }
        }
        cursor += chunk.get_duration();
    } else {
        cursor = end; // paused/stopped - keep the cursor near the head
    }
}

// Whether `host` is really on screen with a drawable client area.
//
// GROUND TRUTH, asked of the window rather than accumulated from messages.
// `visible` used to be a latch driven only by WM_SIZE / WM_SHOWWINDOW edges, and
// an edge that never arrived (or arrived while another host owned the session)
// left it stuck - with a killed render timer and no way back. Deriving the
// answer means a wrong value can only survive until the next watchdog tick.
bool host_is_showing(HWND host) {
    if (host == nullptr || IsWindowVisible(host) == FALSE) return false;
    if (IsIconic(host) != FALSE) return false;
    RECT rc = {};
    if (GetClientRect(host, &rc) == FALSE) return false;
    return (rc.right - rc.left) > 0 && (rc.bottom - rc.top) > 0;
}

// ---- Now-playing banner (Plan 0097 Phase 5, ADR-0110) ------------------
//
// foobar hands a component the exact metadata, so this side does no guessing:
// it renders one string through titleformat and pushes it over the C ABI. The
// core owns everything after that - the fade, the layout, the truncation - and
// learns nothing about where the string came from.

// The script is a CONTRACT, not a display choice: the core takes one string and
// splits it at the first " - ", so this has to produce that shape. The square
// brackets drop the whole "artist - " chunk when the field is missing, which is
// what stops a title-only track reading as "? - Title".
constexpr char kNowPlayingScript[] = "[%artist% - ]%title%";

// Compiled once, on first use. titleformat compilation is not free and this
// runs on every track change.
const titleformat_object::ptr &now_playing_script() {
    static titleformat_object::ptr script;
    if (script.is_empty()) {
        titleformat_compiler::get()->compile_safe(script, kNowPlayingScript);
    }
    return script;
}

// Push `track` into the core's banner.
//
// MAIN THREAD ONLY, and that is the real-time rule here rather than a style
// note: lmv_set_now_playing COPIES the string, and an allocation on the
// visualisation_stream thread is exactly what the ABI's threading contract
// forbids. Every caller below is a play_callback or a window message - foobar
// delivers both on the main thread, the same one the render timer runs on.
void announce(const metadb_handle_ptr &track) {
    // Gated on the ABI handshake: lmv_set_now_playing is v5, so a core older
    // than this shim does not have it.
    if (!g_abi_ok || g_session.handle == nullptr || track.is_empty()) return;
    pfc::string8 text;
    track->format_title(nullptr, text, now_playing_script(), nullptr);
    if (text.is_empty()) return; // nothing to announce is not an error
    lmv_set_now_playing(g_session.handle,
                        reinterpret_cast<const uint8_t *>(text.get_ptr()),
                        text.get_length());
}

// Announce whatever is playing right now, if anything.
//
// A statically registered play_callback gets no replay of the current state, so
// a panel added or a window opened *during* a track would otherwise show
// nothing until the next one. The core ignores a string it already holds, so
// this cannot double-trigger against a real track change.
void announce_current() {
    metadb_handle_ptr track;
    if (playback_control::get()->get_now_playing(track)) announce(track);
}

class play_callback_lmv : public play_callback_static {
public:
    // Track changes drive the banner; start/stop/pause drive the render
    // cadence, which is otherwise only noticed on the render timer's own next
    // tick - and not at all once that timer has been killed.
    unsigned get_flags() override {
        return flag_on_playback_new_track | flag_on_playback_starting |
               flag_on_playback_stop | flag_on_playback_pause;
    }

    void on_playback_new_track(metadb_handle_ptr p_track) override {
        announce(p_track);
        g_session.sync_render_timer();
    }

    // Cadence only - `playing_at_full_rate()` re-reads the transport, so these
    // just tell the session to look again rather than passing a state along.
    void on_playback_starting(play_control::t_track_command, bool) override {
        g_session.sync_render_timer();
    }
    void on_playback_stop(play_control::t_stop_reason) override {
        g_session.sync_render_timer();
    }
    void on_playback_pause(bool) override { g_session.sync_render_timer(); }

    // The rest of the interface, deliberately empty.
    void on_playback_seek(double) override {}
    void on_playback_edited(metadb_handle_ptr) override {}
    void on_playback_dynamic_info(const file_info &) override {}
    void on_playback_dynamic_info_track(const file_info &) override {}
    void on_playback_time(double) override {}
    void on_volume_change(float) override {}
};

play_callback_static_factory_t<play_callback_lmv> g_play_callback_factory;

bool VizSession::claim(HWND host) {
    if (owner != nullptr) return false; // another host drives the core
    if (stream.is_empty()) {
        visualisation_manager::get()->create_stream(stream, 0);
    }
    cursor = 0.0;
    owner = host; // ensure_handle attaches to the owner window
    // Default format so visuals run before (or without) playback; swapped
    // out automatically when the first chunk reports the real format.
    ensure_handle(48000, 2);
    if (handle == nullptr) {
        owner = nullptr; // create failed - stay free so another host may try
        stream.release();
        return false;
    }
    visible = true;
    timer_ms = 0;
    sync_render_timer(); // starts the render timer at the right cadence
    // The self-heal tick, for as long as this host owns the session. Armed even
    // though the render timer was just started: the whole point is that it
    // outlives a KillTimer the render timer cannot come back from.
    SetTimer(host, kWatchdogTimer, kWatchdogMs, nullptr);
    // A host that claims mid-track fires no play_callback of its own, so ask.
    announce_current();
    return true;
}

void VizSession::release(HWND host) {
    if (owner != host) return;
    KillTimer(host, kRenderTimer);
    KillTimer(host, kWatchdogTimer);
    timer_ms = 0;
    visible = true;
    destroy_handle();
    stream.release();
    cursor = 0.0;
    owner = nullptr;
}

// True only while a track is actively playing (not paused, not stopped).
bool playing_at_full_rate() {
    playback_control::ptr pc = playback_control::get();
    return pc->is_playing() && !pc->is_paused();
}

void VizSession::sync_render_timer() {
    if (owner == nullptr) return;
    const UINT target =
        !visible ? 0 : (playing_at_full_rate() ? kRenderTimerMs : kIdleTimerMs);
    if (target == timer_ms) return;
    if (target == 0) {
        KillTimer(owner, kRenderTimer);
    } else {
        SetTimer(owner, kRenderTimer, target, nullptr); // re-arms same id
    }
    timer_ms = target;
}

// Apply a visibility change reported for `host` (Default UI notify, or a
// pop-out show/hide/minimise). Only the owner's timer is affected.
void set_host_visibility(HWND host, bool vis) {
    if (g_session.owner != host || g_session.visible == vis) return;
    g_session.visible = vis;
    g_session.sync_render_timer();
}

// Paint the "someone else owns the core" placeholder for a non-owning host.
void paint_placeholder(HWND wnd, HDC hdc) {
    RECT rc = {};
    GetClientRect(wnd, &rc);
    FillRect(hdc, &rc, static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
    const wchar_t *msg = L"Light Music Visualizer is active in another window";
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, RGB(180, 180, 180));
    // Word-wrap, then vertically centre the wrapped block within the client.
    RECT measure = rc;
    DrawTextW(hdc, msg, -1, &measure, DT_CENTER | DT_WORDBREAK | DT_CALCRECT);
    RECT draw = rc;
    const LONG text_h = measure.bottom - measure.top;
    if (text_h < rc.bottom - rc.top) {
        draw.top = rc.top + ((rc.bottom - rc.top) - text_h) / 2;
    }
    DrawTextW(hdc, msg, -1, &draw, DT_CENTER | DT_WORDBREAK);
}

// Shared window procedure for both host kinds (pop-out top-level and panel
// child). The owner check gates every core call so a non-owning host never
// touches the handle; a non-owner runs an arbitration timer to claim the
// session once it frees and paints the placeholder meanwhile.
LRESULT CALLBACK wnd_proc(HWND wnd, UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
        case WM_CREATE:
            if (!g_session.claim(wnd)) {
                // Another host owns the core - wait for it to free the session.
                SetTimer(wnd, kArbitrationTimer, kArbitrationMs, nullptr);
            }
            return 0;
        case WM_TIMER:
            if (wp == kRenderTimer) {
                if (g_session.owner == wnd) {
                    g_session.pump();
                    if (g_session.handle != nullptr)
                        lmv_render_dt(g_session.handle, g_session.measure_dt());
                    g_session.maybe_log_metrics(); // ~1 Hz, gated internally
                    // Follow play/pause transitions between full and idle rate.
                    g_session.sync_render_timer();
                }
                return 0;
            }
            if (wp == kWatchdogTimer) {
                // Re-derive visibility and re-sync the cadence. This is what
                // makes a missed show/hide edge survivable: whatever `visible`
                // latched to, the window itself decides here, so a killed
                // render timer is re-armed within one tick.
                if (g_session.owner == wnd) {
                    const bool showing = host_is_showing(wnd);
                    if (g_session.visible != showing) g_session.visible = showing;
                    g_session.sync_render_timer();
                }
                return 0;
            }
            if (wp == kArbitrationTimer) {
                // Session free? Take it over (claim starts the render timer)
                // and repaint to clear the placeholder.
                if (g_session.owner == nullptr && g_session.claim(wnd)) {
                    KillTimer(wnd, kArbitrationTimer);
                    InvalidateRect(wnd, nullptr, FALSE);
                }
                return 0;
            }
            break;
        case WM_SIZE: {
            // Zero size or minimise counts as hidden (stops rendering); a real
            // size means shown and drives the core resize.
            const bool hidden = (wp == SIZE_MINIMIZED) ||
                                 (LOWORD(lp) == 0) || (HIWORD(lp) == 0);
            set_host_visibility(wnd, !hidden);
            if (g_session.owner == wnd) {
                if (!hidden && g_session.handle != nullptr) {
                    if (g_session.needs_reattach) {
                        // First real size for a surface attached while the host
                        // was still 0-sized: recreate it so it actually presents
                        // (a plain resize can't revive the dead 1x1 surface).
                        g_session.reattach_at_current_size();
                    } else {
                        lmv_resize(g_session.handle, LOWORD(lp), HIWORD(lp));
                    }
                }
            } else {
                InvalidateRect(wnd, nullptr, FALSE); // re-centre the placeholder
            }
            return 0;
        }
        case WM_SHOWWINDOW:
            set_host_visibility(wnd, wp != FALSE);
            break;
        case WM_KEYDOWN:
            if (wp == VK_SPACE && g_session.owner == wnd &&
                g_session.handle != nullptr) {
                lmv_cycle_scene(g_session.handle);
                return 0;
            }
            break;
        case WM_LBUTTONDOWN:
            SetFocus(wnd); // so a subsequent Space reaches this panel/window
            return 0;
        case WM_CONTEXTMENU: {
            // Owner-only: the right-click "Next scene" works without keyboard
            // focus; a placeholder (non-owner) host offers nothing.
            if (g_session.owner != wnd || g_session.handle == nullptr) break;
            POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
            if (pt.x == -1 && pt.y == -1) { // keyboard-invoked: centre on window
                RECT rc = {};
                GetWindowRect(wnd, &rc);
                pt.x = (rc.left + rc.right) / 2;
                pt.y = (rc.top + rc.bottom) / 2;
            }
            HMENU menu = CreatePopupMenu();
            if (menu == nullptr) return 0;

            // The roster is read ONCE, here, and the ids the items carry are
            // positions in this snapshot. That is safe because the menu is
            // modal: nothing on this thread can reload presets between the
            // build and the click, and no other thread may touch the handle.
            PresetSnapshot snap;
            size_t listed = 0;
            if (read_preset_snapshot(g_session.handle, snap)) {
                HMENU presets = CreatePopupMenu();
                if (presets != nullptr) {
                    listed = snap.names.size() < kMenuPresetMax ? snap.names.size()
                                                               : kMenuPresetMax;
                    for (size_t i = 0; i < listed; ++i) {
                        AppendMenuW(presets, MF_STRING,
                                    kMenuPresetBase + static_cast<UINT>(i),
                                    menu_label(snap.names[i]).c_str());
                    }
                    if (snap.current >= 0 &&
                        static_cast<size_t>(snap.current) < listed) {
                        // A radio bullet rather than a tick: exactly one preset
                        // is showing, and this also clears the previous mark.
                        CheckMenuRadioItem(
                            presets, kMenuPresetBase,
                            kMenuPresetBase + static_cast<UINT>(listed) - 1,
                            kMenuPresetBase + static_cast<UINT>(snap.current),
                            MF_BYCOMMAND);
                    }
                    // DestroyMenu(menu) below tears the submenu down with it.
                    AppendMenuW(menu, MF_POPUP,
                                reinterpret_cast<UINT_PTR>(presets), L"Preset");
                }
            }

            AppendMenuW(menu, MF_STRING, kMenuNextScene, L"Next scene");
            if (g_abi_ok) {
                AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
                AppendMenuW(menu, MF_STRING, kMenuReloadPresets,
                            L"Reload presets");
                AppendMenuW(menu, MF_STRING, kMenuOpenPresetDir,
                            L"Open presets folder");
                AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
                const UINT check =
                    (g_debug_flags & LMV_DEBUG_OVERLAY) ? MF_CHECKED : MF_UNCHECKED;
                AppendMenuW(menu, MF_STRING | check, kMenuToggleOverlay,
                            L"Diagnostics overlay");
            }
            const int cmd =
                TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, pt.x, pt.y,
                               0, wnd, nullptr);
            DestroyMenu(menu);
            // The menu is modal, so ownership can have changed while it was up.
            if (g_session.owner != wnd || g_session.handle == nullptr) return 0;
            const UINT ucmd = static_cast<UINT>(cmd);
            if (cmd == kMenuNextScene) {
                lmv_cycle_scene(g_session.handle);
            } else if (cmd == kMenuToggleOverlay && g_abi_ok) {
                // Flip the overlay bit and push it live over the ABI.
                g_debug_flags ^= LMV_DEBUG_OVERLAY;
                lmv_set_debug(g_session.handle, g_debug_flags);
            } else if (cmd == kMenuReloadPresets && g_abi_ok) {
                reload_presets_keeping_selection(g_session.handle);
            } else if (cmd == kMenuOpenPresetDir) {
                open_preset_folder();
            } else if (listed != 0 && ucmd >= kMenuPresetBase &&
                       ucmd < kMenuPresetBase + listed) {
                lmv_select_preset(g_session.handle,
                                  static_cast<int32_t>(ucmd - kMenuPresetBase));
            }
            return 0;
        }
        case WM_PAINT:
            if (g_session.owner != wnd) {
                PAINTSTRUCT ps = {};
                HDC hdc = BeginPaint(wnd, &ps);
                paint_placeholder(wnd, hdc);
                EndPaint(wnd, &ps);
                return 0;
            }
            break; // owner: the core presents on its timer; DefWindowProc validates
        case WM_ERASEBKGND:
            return 1; // owner: core repaints; non-owner: WM_PAINT fills fully
        case WM_CLOSE:
            DestroyWindow(wnd); // pop-out only; panels are destroyed by the host
            return 0;
        case WM_DESTROY:
            KillTimer(wnd, kArbitrationTimer); // no-op if this host was the owner
            g_session.release(wnd); // frees the handle iff this host owned it
            if (wnd == g_popup_hwnd) g_popup_hwnd = nullptr; // allow reopening
            return 0;
        default:
            break;
    }
    return DefWindowProcW(wnd, msg, wp, lp);
}

// Register the shared window class once.
void ensure_window_class() {
    static bool registered = false;
    if (registered) return;
    WNDCLASSW wc = {};
    wc.lpfnWndProc = wnd_proc;
    wc.hInstance = core_api::get_my_instance();
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.lpszClassName = kWindowClass;
    if (RegisterClassW(&wc) != 0) registered = true;
}

// ---- Pop-out window (View menu) ----------------------------------------

void open_window() {
    if (g_popup_hwnd != nullptr) {
        SetForegroundWindow(g_popup_hwnd);
        return;
    }
    ensure_window_class();
    g_popup_hwnd = CreateWindowExW(
        0, kWindowClass, L"Light Music Visualizer",
        WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT, 1024, 640,
        core_api::get_main_window(), nullptr, core_api::get_my_instance(),
        nullptr);
}

class mainmenu_commands_lmv : public mainmenu_commands {
public:
    t_uint32 get_command_count() override { return 1; }
    GUID get_command(t_uint32) override { return kGuidLmvMenu; }
    void get_name(t_uint32, pfc::string_base &out) override {
        out = "Light Music Visualizer";
    }
    bool get_description(t_uint32, pfc::string_base &out) override {
        out = "Opens the Light Music Visualizer window (Space cycles scenes).";
        return true;
    }
    GUID get_parent() override { return mainmenu_groups::view; }
    void execute(t_uint32, service_ptr_t<service_base>) override { open_window(); }
};

mainmenu_commands_factory_t<mainmenu_commands_lmv> g_mainmenu_factory;

// Tear the pop-out down before the app finishes shutting down. Panels are
// destroyed by the Default UI host; whichever owned the session releases it
// via WM_DESTROY, so the handle is freed exactly once.
class initquit_lmv : public initquit {
public:
    // Runtime ABI handshake: the shim links the core's C ABI compiled
    // separately, so a version mismatch is caught here rather than by calling a
    // function whose contract has shifted. Preset loading (v2) is gated on it.
    void on_init() override {
        const uint32_t core_abi = lmv_abi_version();
        // v3 is forward-compatible: get_metrics is size-guarded and the older
        // functions are stable, so a newer core is fine - require >= built ABI.
        g_abi_ok = (core_abi >= LMV_ABI_VERSION);
        if (!g_abi_ok) {
            console::printf("foo_lmv: lmv-core ABI too old (core reports %u, "
                            "shim needs >= %u); preset loading and diagnostics "
                            "disabled",
                            static_cast<unsigned>(core_abi),
                            static_cast<unsigned>(LMV_ABI_VERSION));
        }
        // Seed the overlay default from the environment (a boundary read).
        g_debug_flags = env_overlay_on() ? LMV_DEBUG_OVERLAY : LMV_DEBUG_OFF;
    }
    void on_quit() override {
        if (g_popup_hwnd != nullptr) DestroyWindow(g_popup_hwnd);
    }
};

initquit_factory_t<initquit_lmv> g_initquit_factory;

// ---- Default UI panel (ui_element) -------------------------------------

// One embedded panel instance: owns a WS_CHILD window parented into the
// layout. The window claims the shared session on WM_CREATE and releases it on
// WM_DESTROY, exactly like the pop-out, so no panel-specific core logic exists.
class lmv_ui_element_instance : public ui_element_instance {
public:
    lmv_ui_element_instance(HWND parent, ui_element_instance_callback_ptr callback)
        : m_callback(callback) {
        ensure_window_class();
        m_wnd = CreateWindowExW(0, kWindowClass, L"", WS_CHILD | WS_VISIBLE, 0, 0,
                                0, 0, parent, nullptr,
                                core_api::get_my_instance(), nullptr);
    }
    ~lmv_ui_element_instance() {
        if (m_wnd != nullptr) DestroyWindow(m_wnd);
    }

    fb2k::hwnd_t get_wnd() override { return m_wnd; }
    void set_configuration(ui_element_config::ptr) override {}
    ui_element_config::ptr get_configuration() override {
        return ui_element_config::g_create_empty(kGuidLmvElement);
    }
    GUID get_guid() override { return kGuidLmvElement; }
    GUID get_subclass() override {
        return ui_element_subclass_playback_visualisation;
    }
    void notify(const GUID &what, t_size param1, const void *,
                t_size) override {
        // Default UI's authoritative show/hide for a layout tab; param1 is the
        // new-visible bool. Stops/resumes rendering when the panel is a
        // background tab.
        if (what == ui_element_notify_visibility_changed) {
            set_host_visibility(m_wnd, param1 != 0);
        }
    }

private:
    HWND m_wnd = nullptr;
    ui_element_instance_callback_ptr m_callback;
};

class lmv_ui_element : public ui_element {
public:
    GUID get_guid() override { return kGuidLmvElement; }
    GUID get_subclass() override {
        return ui_element_subclass_playback_visualisation;
    }
    void get_name(pfc::string_base &out) override {
        out = "Light Music Visualizer";
    }
    ui_element_instance_ptr instantiate(
        fb2k::hwnd_t parent, ui_element_config::ptr,
        ui_element_instance_callback_ptr callback) override {
        return new service_impl_t<lmv_ui_element_instance>(
            static_cast<HWND>(parent), callback);
    }
    ui_element_config::ptr get_default_configuration() override {
        return ui_element_config::g_create_empty(kGuidLmvElement);
    }
    ui_element_children_enumerator_ptr enumerate_children(
        ui_element_config::ptr) override {
        return nullptr;
    }
    bool get_description(pfc::string_base &out) override {
        out = "Audio-reactive visuals - fragment fields, particle swarm, line "
              "geometry, reaction-diffusion and attractor scenes - from "
              "lmv-core. Space cycles scenes.";
        return true;
    }
};

service_factory_single_t<lmv_ui_element> g_lmv_ui_element_factory;

} // namespace

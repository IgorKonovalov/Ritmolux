// The visualizer session: the core handle, the visualisation stream, the render
// timer's cadence, and the diagnostics log. One instance, main thread only.
//
// Exactly one host window (pop-out or Default UI panel) owns it at a time; only
// the owner holds the `RlxHandle`, the stream and the render timer, so there is
// only ever one wgpu surface.

#include "viz_session.h"

#include "SDK/console.h"

#include <cstdio>
#include <share.h> // _SH_DENYWR

namespace rlx {

VizSession g_session;

// %APPDATA%\Ritmolux - the app dir the standalone shares. Empty on
// failure. The diagnostics log lives here, next to the shared presets dir.
std::wstring plugin_app_dir_w() {
    const DWORD need = GetEnvironmentVariableW(L"APPDATA", nullptr, 0);
    if (need == 0) return {};
    std::wstring wide(need, L'\0');
    const DWORD got = GetEnvironmentVariableW(L"APPDATA", wide.data(), need);
    if (got == 0 || got >= need) return {};
    wide.resize(got);
    wide += L"\\Ritmolux";
    return wide;
}

void VizSession::destroy_handle() {
    if (handle) {
        rlx_free(handle);
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
    RlxHandle *h = rlx_create(new_rate, new_channels);
    if (h == nullptr) return; // format outside core bounds - skip
    RECT rc = {};
    GetClientRect(owner, &rc);
    const uint32_t w = static_cast<uint32_t>(rc.right - rc.left);
    const uint32_t ht = static_cast<uint32_t>(rc.bottom - rc.top);
    if (rlx_attach_window(h, owner, w ? w : 1, ht ? ht : 1) != RLX_OK) {
        rlx_free(h);
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
    // ...and comes up on the preset the user last chose. After the attach above
    // on purpose: the roster installs there, so the snapshot this reads against
    // does not exist any earlier. A format change mid-playback therefore keeps
    // the look on screen instead of snapping back to the roster's first entry.
    restore_remembered_preset(h);
    // Re-apply the current debug flags so a menu-toggled overlay survives a
    // handle re-creation (mid-playback format change); the core otherwise
    // re-seeds from the env at create.
    if (g_session.abi_ok) rlx_set_debug(h, g_session.debug_flags);
}

void VizSession::reattach_at_current_size() {
    if (owner == nullptr || handle == nullptr) return;
    const uint32_t r = rate;
    const uint16_t c = channels;
    destroy_handle();    // clears rate/channels so ensure_handle re-attaches
    ensure_handle(r, c); // fresh rlx_create + rlx_attach_window at the real size
}

// Append a diagnostics sample to %APPDATA%\Ritmolux\
// plugin-diagnostics.log at ~1 Hz. RSS is not logged: it is host-process-owned
// (ADR-0008) and would mean "all of foobar", not "us".
void VizSession::maybe_log_metrics() {
    if (!g_session.abi_ok || handle == nullptr) return;
    const ULONGLONG now = GetTickCount64();
    if (last_log_ms != 0 && now - last_log_ms < 1000) return;
    last_log_ms = now;

    RlxMetrics m = {};
    m.struct_size = sizeof(RlxMetrics);
    if (rlx_get_metrics(handle, &m) != RLX_OK) return;

    // Opened once and kept (`VizSession::log`). The header is written on the
    // open that created the file, which is the only moment the file is known to
    // be new: a reopen cannot tell a file it just created from one it appended
    // to a second ago without stat'ing it again.
    if (log == nullptr) {
        const std::wstring dir = plugin_app_dir_w();
        if (dir.empty()) return;
        CreateDirectoryW(dir.c_str(), nullptr); // parent (%APPDATA%) already exists
        const std::wstring path = dir + L"\\plugin-diagnostics.log";
        const bool is_new =
            GetFileAttributesW(path.c_str()) == INVALID_FILE_ATTRIBUTES;
        // `_wfsopen` with `_SH_DENYWR`, not `_wfopen_s`. The CRT's default
        // share mode is exclusive, and a handle held for the whole session
        // would then lock every other reader out of the file for as long as the
        // visualisation runs -- which is precisely when someone tails it. This
        // denies other WRITERS (there is one writer by construction) and lets
        // readers in.
        log = _wfsopen(path.c_str(), L"a", _SH_DENYWR);
        if (log == nullptr) return;
        if (is_new) {
            fprintf(log, "unix_ms\tfps\tframe_ms_avg\tframe_ms_p99\tframes_total"
                         "\tframes_dropped\tgpu_bytes\tdraw_calls\n");
        }
    }
    FILE *const f = log;
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
    // Flushed, not closed: a crash mid-show must still leave the samples that
    // led up to it on disk, which is the whole point of a 1 Hz log.
    fflush(f);
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
        rlx_push_samples(handle, conv, static_cast<uint32_t>(n));
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
// A latch driven only by WM_SIZE / WM_SHOWWINDOW edges sticks: an edge that
// never arrives (or arrives while another host owns the session) leaves it
// wrong - with a killed render timer and no way back. Deriving the answer means
// a wrong value can only survive until the next watchdog tick.
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
// note: rlx_set_now_playing COPIES the string, and an allocation on the
// visualisation_stream thread is exactly what the ABI's threading contract
// forbids. Every caller below is a play_callback or a window message - foobar
// delivers both on the main thread, the same one the render timer runs on.
void announce(const metadb_handle_ptr &track) {
    // Gated on the ABI handshake: rlx_set_now_playing is v5, so a core older
    // than this shim does not have it.
    if (!g_session.abi_ok || g_session.handle == nullptr || track.is_empty()) return;
    pfc::string8 text;
    track->format_title(nullptr, text, now_playing_script(), nullptr);
    if (text.is_empty()) return; // nothing to announce is not an error
    rlx_set_now_playing(g_session.handle,
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
    close_log();
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

void VizSession::close_log() {
    if (log != nullptr) {
        fclose(log);
        log = nullptr;
    }
}

} // namespace rlx

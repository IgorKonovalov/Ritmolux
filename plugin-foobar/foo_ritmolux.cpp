// foo_ritmolux — foobar2000 visualization component for Ritmolux.
//
// A thin shim per ADR-0001: pulls PCM from foobar's visualisation_stream,
// forwards it across rlx-core's C ABI, and hosts the core's wgpu output in a
// plain Win32 window. All logic lives in the Rust core; this file only
// bridges foobar2000 conventions to the ABI in core-cabi/include/rlx_core.h.
//
// Two entry points share ONE core instance: a View-menu pop-out window and a
// Default UI panel (ui_element). Both are "host windows" that claim a single
// global VizSession; the session holds the sole RlxHandle + visualisation
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

#include "rlx_core.h"

// Component version, single-sourced from the workspace Cargo version (ADR-0025).
// build.ps1 generates foo_ritmolux_version.h into build/ (on the include path) from
// root Cargo.toml's [workspace.package] version; the fallback keeps a compile
// outside build.ps1 (editor tooling, a stray direct cl) building as 0.0.0-dev.
#if __has_include("foo_ritmolux_version.h")
#  include "foo_ritmolux_version.h"
#endif
#ifndef FOO_RITMOLUX_VERSION
#  define FOO_RITMOLUX_VERSION "0.0.0-dev"
#endif

// foobar2000 x64 uses 64-bit audio_sample; rlx-core takes f32, so chunks are
// converted through a fixed buffer on the way in (see push_converted).

DECLARE_COMPONENT_VERSION(
    "Ritmolux", FOO_RITMOLUX_VERSION,
    "Ritmolux\n"
    "Audio-reactive scenes - fragment fields, particle swarm, line geometry "
    "(curves / L-systems / star patterns), reaction-diffusion and attractor "
    "flows - rendered by the shared rlx-core Rust engine (wgpu).\n"
    "Dockable as a Default UI panel or opened from the View menu. "
    "Space cycles scenes; right-click to pick one by name, reload the preset "
    "folder, or open it.");
VALIDATE_COMPONENT_FILENAME("foo_ritmolux.dll");

#include "viz_session.h"

using rlx::announce;
using rlx::announce_current;
using rlx::ensure_window_class;
using rlx::env_overlay_on;
using rlx::g_popup_hwnd;
using rlx::g_session;
using rlx::kWindowClass;
using rlx::open_window;
using rlx::set_host_visibility;

namespace {

// GUIDs owned by this component - never reuse.
constexpr GUID kGuidLmvMenu = {
    0x8f7c2a1e, 0x94d3, 0x4b6a, {0x9c, 0x1f, 0x27, 0x5e, 0x88, 0x3a, 0xd4, 0x61}};
constexpr GUID kGuidLmvElement = {
    0x2d9b4f7c, 0x6e21, 0x4a83, {0xb5, 0x0c, 0x1a, 0x77, 0x3e, 0x08, 0xc2, 0x54}};

class play_callback_rlx : public play_callback_static {
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

play_callback_static_factory_t<play_callback_rlx> g_play_callback_factory;

class mainmenu_commands_rlx : public mainmenu_commands {
public:
    t_uint32 get_command_count() override { return 1; }
    GUID get_command(t_uint32) override { return kGuidLmvMenu; }
    void get_name(t_uint32, pfc::string_base &out) override {
        out = "Ritmolux";
    }
    bool get_description(t_uint32, pfc::string_base &out) override {
        out = "Opens the Ritmolux window (Space cycles scenes).";
        return true;
    }
    GUID get_parent() override { return mainmenu_groups::view; }
    void execute(t_uint32, service_ptr_t<service_base>) override { open_window(); }
};

mainmenu_commands_factory_t<mainmenu_commands_rlx> g_mainmenu_factory;

// Tear the pop-out down before the app finishes shutting down. Panels are
// destroyed by the Default UI host; whichever owned the session releases it
// via WM_DESTROY, so the handle is freed exactly once.
class initquit_rlx : public initquit {
public:
    // Runtime ABI handshake: the shim links the core's C ABI compiled
    // separately, so a version mismatch is caught here rather than by calling a
    // function whose contract has shifted. Preset loading (v2) is gated on it.
    void on_init() override {
        const uint32_t core_abi = rlx_abi_version();
        // v3 is forward-compatible: get_metrics is size-guarded and the older
        // functions are stable, so a newer core is fine - require >= built ABI.
        g_session.abi_ok = (core_abi >= RLX_ABI_VERSION);
        if (!g_session.abi_ok) {
            console::printf("foo_ritmolux: rlx-core ABI too old (core reports %u, "
                            "shim needs >= %u); preset loading and diagnostics "
                            "disabled",
                            static_cast<unsigned>(core_abi),
                            static_cast<unsigned>(RLX_ABI_VERSION));
        }
        // Seed the overlay default from the environment (a boundary read).
        g_session.debug_flags = env_overlay_on() ? RLX_DEBUG_OVERLAY : RLX_DEBUG_OFF;
    }
    void on_quit() override {
        if (g_popup_hwnd != nullptr) DestroyWindow(g_popup_hwnd);
    }
};

initquit_factory_t<initquit_rlx> g_initquit_factory;

// ---- Default UI panel (ui_element) -------------------------------------

// One embedded panel instance: owns a WS_CHILD window parented into the
// layout. The window claims the shared session on WM_CREATE and releases it on
// WM_DESTROY, exactly like the pop-out, so no panel-specific core logic exists.
class rlx_ui_element_instance : public ui_element_instance {
public:
    rlx_ui_element_instance(HWND parent, ui_element_instance_callback_ptr callback)
        : m_callback(callback) {
        ensure_window_class();
        m_wnd = CreateWindowExW(0, kWindowClass, L"", WS_CHILD | WS_VISIBLE, 0, 0,
                                0, 0, parent, nullptr,
                                core_api::get_my_instance(), nullptr);
    }
    ~rlx_ui_element_instance() {
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

class rlx_ui_element : public ui_element {
public:
    GUID get_guid() override { return kGuidLmvElement; }
    GUID get_subclass() override {
        return ui_element_subclass_playback_visualisation;
    }
    void get_name(pfc::string_base &out) override {
        out = "Ritmolux";
    }
    ui_element_instance_ptr instantiate(
        fb2k::hwnd_t parent, ui_element_config::ptr,
        ui_element_instance_callback_ptr callback) override {
        return new service_impl_t<rlx_ui_element_instance>(
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
              "rlx-core. Space cycles scenes.";
        return true;
    }
};

service_factory_single_t<rlx_ui_element> g_rlx_ui_element_factory;

} // namespace

// The preset roster, over the C ABI.
//
// The names are the CORE's, never the folder's: a malformed `.toml` a user
// dropped into the shared directory is absent from every list here, so the menu
// cannot offer a preset that would not load. Everything runs on the main thread
// (menu, timer), never the audio path, so the disk I/O is fine.

#include "viz_session.h"

#include <shellapi.h> // ShellExecuteW (Open presets folder)

namespace rlx {

namespace {

// The GUID this component stores its one persisted setting under - never reuse.
constexpr GUID kGuidCfgPreset = {
    0x5b3ad1c7, 0x0f4e, 0x4d92, {0xa6, 0x18, 0x3c, 0x71, 0x9e, 0x2d, 0x84, 0x0b}};

// The component's only persisted setting: the preset the user last chose, BY
// NAME. A name and not an index, because the roster is a directory listing -
// dropping a file in reorders it, and an index would then restore a different
// look than the one that was showing. An empty value, or a name whose file is
// gone, means "whatever the roster opens on"; nothing is surfaced to the user,
// since a preset they deleted is not an error.
cfg_string g_cfg_preset(kGuidCfgPreset, "");

// The shared per-user preset directory: %APPDATA%\Ritmolux\presets
// - the exact path the standalone seeds and watches, so both frontends share one
// library. Empty on failure (the core then keeps its embedded defaults).
//
// This is the last independent copy of that path. The two Rust frontends (the
// app and the shot CLI) now share one resolver in standalone/src/lib.rs, which
// also honors the RLX_PRESET_DIR override (ADR-0014); this shim deliberately
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

std::string resolve_preset_dir_utf8() { return narrow(preset_dir_w()); }

} // namespace

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

// Seed + load the shared preset library into `h` over the C ABI. No-op if the
// ABI handshake failed or the directory can't be resolved. Runs on the main
// thread (menu/timer), never the audio callback, so its disk I/O is fine.
void load_presets_into(RlxHandle *h) {
    if (!g_session.abi_ok || h == nullptr) return;
    const std::string dir = resolve_preset_dir_utf8();
    if (dir.empty()) return;
    rlx_load_presets(h, reinterpret_cast<const uint8_t *>(dir.data()),
                     dir.size());
}

// Fill `out` from `h`. False (and an empty snapshot) when the ABI is too old,
// no window is attached yet, or the roster is empty - all cases where the menu
// simply omits the Preset submenu rather than showing an empty one.
bool read_preset_snapshot(RlxHandle *h, PresetSnapshot &out) {
    out.names.clear();
    out.current = -1;
    if (!g_session.abi_ok || h == nullptr) return false;
    // Call twice: size, then fill. Nothing is written when the buffer is short,
    // so a roster that grew between the two calls yields no names rather than a
    // truncated list - and it cannot, since both run on this thread.
    const int32_t needed = rlx_get_presets(h, nullptr, 0, &out.current);
    if (needed <= 0) return false;
    std::vector<uint8_t> buf(static_cast<size_t>(needed));
    if (rlx_get_presets(h, buf.data(), buf.size(), &out.current) != needed) {
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
bool select_preset_named(RlxHandle *h, const std::string &name) {
    PresetSnapshot snap;
    if (name.empty() || !read_preset_snapshot(h, snap)) return false;
    for (size_t i = 0; i < snap.names.size(); ++i) {
        if (snap.names[i] != name) continue;
        return rlx_select_preset(h, static_cast<int32_t>(i)) == RLX_OK;
    }
    return false; // a name that is gone leaves the roster where it is
}

// Store what is showing now, so the next foobar start comes up on it. Reads the
// name back from the core rather than trusting the caller's index: the snapshot
// reports the dissolve's target, which is the preset the user just asked for.
void remember_current_preset(RlxHandle *h) {
    PresetSnapshot snap;
    if (!read_preset_snapshot(h, snap)) return;
    if (snap.current < 0 ||
        static_cast<size_t>(snap.current) >= snap.names.size()) {
        return;
    }
    g_cfg_preset.set(snap.names[static_cast<size_t>(snap.current)].c_str());
}

// Restore the remembered preset onto a freshly loaded roster. Called after the
// window is attached and the library is loaded - the roster does not exist
// before either. A name that fails to resolve leaves the roster's own default
// showing, which is the documented degrade.
void restore_remembered_preset(RlxHandle *h) {
    const pfc::string8 name = g_cfg_preset.get();
    if (name.is_empty()) return;
    select_preset_named(h, std::string(name.get_ptr(), name.get_length()));
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
void reload_presets_keeping_selection(RlxHandle *h) {
    if (!g_session.abi_ok || h == nullptr) return;
    PresetSnapshot before;
    std::string keep;
    if (read_preset_snapshot(h, before) && before.current >= 0 &&
        static_cast<size_t>(before.current) < before.names.size()) {
        keep = before.names[static_cast<size_t>(before.current)];
    }
    load_presets_into(h);
    select_preset_named(h, keep);
    // Whatever the reload landed on is now what a restart should come up on -
    // including the case where `keep`'s file was the one deleted.
    remember_current_preset(h);
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

// True when RLX_DEBUG_OVERLAY is set to a truthy value (1/true/on/yes). Seeds
// the overlay default; the core reads the same var at rlx_create, but the plugin
// tracks it too so the menu toggle and handle re-creation stay consistent.
bool env_overlay_on() {
    wchar_t buf[16] = {};
    const DWORD got = GetEnvironmentVariableW(L"RLX_DEBUG_OVERLAY", buf, 16);
    if (got == 0 || got >= 16) return false;
    return _wcsicmp(buf, L"1") == 0 || _wcsicmp(buf, L"true") == 0 ||
           _wcsicmp(buf, L"on") == 0 || _wcsicmp(buf, L"yes") == 0;
}

} // namespace rlx

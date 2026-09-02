// The host window: one window procedure for both host kinds, the class it is
// registered under, and the pop-out the View menu opens.
//
// The owner check gates every core call, so a non-owning host never touches the
// handle; a non-owner runs an arbitration timer to claim the session once it
// frees, and paints a placeholder meanwhile.

#include "viz_session.h"

#include <windowsx.h> // GET_X_LPARAM / GET_Y_LPARAM

namespace rlx {

HWND g_popup_hwnd = nullptr;

namespace {

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

} // namespace

// Paint the "someone else owns the core" placeholder for a non-owning host.
void paint_placeholder(HWND wnd, HDC hdc) {
    RECT rc = {};
    GetClientRect(wnd, &rc);
    FillRect(hdc, &rc, static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
    const wchar_t *msg = L"Ritmolux is active in another window";
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

namespace {

// The three timers a host window runs, and what each tick does. Returns whether
// the id was one of them -- a `false` falls through to `DefWindowProc`, which is
// what an id this component did not set must do.
bool handle_timer(HWND wnd, WPARAM wp) {
    if (wp == kRenderTimer) {
        if (g_session.owner == wnd) {
            g_session.pump();
            if (g_session.handle != nullptr)
                rlx_render_dt(g_session.handle, g_session.measure_dt());
            g_session.maybe_log_metrics(); // ~1 Hz, gated internally
            // Follow play/pause transitions between full and idle rate.
            g_session.sync_render_timer();
        }
        return true;
    }
    if (wp == kWatchdogTimer) {
        // Re-derive visibility and re-sync the cadence. This is what makes a
        // missed show/hide edge survivable: whatever `visible` latched to, the
        // window itself decides here, so a killed render timer is re-armed
        // within one tick.
        if (g_session.owner == wnd) {
            const bool showing = host_is_showing(wnd);
            if (g_session.visible != showing) g_session.visible = showing;
            g_session.sync_render_timer();
        }
        return true;
    }
    if (wp == kArbitrationTimer) {
        // Session free? Take it over (claim starts the render timer) and repaint
        // to clear the placeholder.
        if (g_session.owner == nullptr && g_session.claim(wnd)) {
            KillTimer(wnd, kArbitrationTimer);
            InvalidateRect(wnd, nullptr, FALSE);
        }
        return true;
    }
    return false;
}

// Build the right-click menu, filling `snap` and `listed` with the roster
// reading its Preset submenu was built from.
//
// **The roster is read ONCE, here**, and the ids the items carry are positions
// in this snapshot. The snapshot can go stale while the menu is up:
// `TrackPopupMenu` runs its OWN message loop and keeps dispatching WM_TIMER --
// which is what keeps the visualisation animating -- and that handler reaches
// `ensure_handle`, which on a stream-format change destroys the handle, builds a
// new one and reloads the roster. So these ids address menu items and nothing
// else; `dispatch_menu_command` re-resolves the chosen NAME against a fresh
// roster, because indices are snapshot-scoped (ADR-0117).
HMENU build_context_menu(PresetSnapshot &snap, size_t &listed) {
    HMENU menu = CreatePopupMenu();
    if (menu == nullptr) return nullptr;

    listed = 0;
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
                // A radio bullet rather than a tick: exactly one preset is
                // showing, and this also clears the previous mark.
                CheckMenuRadioItem(
                    presets, kMenuPresetBase,
                    kMenuPresetBase + static_cast<UINT>(listed) - 1,
                    kMenuPresetBase + static_cast<UINT>(snap.current),
                    MF_BYCOMMAND);
            }
            // DestroyMenu(menu) at the call site tears the submenu down with it.
            AppendMenuW(menu, MF_POPUP, reinterpret_cast<UINT_PTR>(presets),
                        L"Preset");
        }
    }

    AppendMenuW(menu, MF_STRING, kMenuNextScene, L"Next scene");
    if (g_session.abi_ok) {
        AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
        AppendMenuW(menu, MF_STRING, kMenuReloadPresets, L"Reload presets");
        AppendMenuW(menu, MF_STRING, kMenuOpenPresetDir, L"Open presets folder");
        AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
        const UINT check = (g_session.debug_flags & RLX_DEBUG_OVERLAY)
                               ? MF_CHECKED
                               : MF_UNCHECKED;
        AppendMenuW(menu, MF_STRING | check, kMenuToggleOverlay,
                    L"Diagnostics overlay");
    }
    return menu;
}

// Act on what the menu returned. `snap` and `listed` are the reading
// `build_context_menu` built the ids from; a preset id is turned back into a
// NAME here and re-resolved against a fresh roster, which is what covers a
// handle REPLACED while the menu was up -- that case keeps this owner and a
// non-null pointer, so the caller's guard passes it through carrying a roster
// that need not match `snap`.
void dispatch_menu_command(UINT cmd, const PresetSnapshot &snap, size_t listed) {
    if (cmd == kMenuNextScene) {
        rlx_cycle_scene(g_session.handle);
        remember_current_preset(g_session.handle);
    } else if (cmd == kMenuToggleOverlay && g_session.abi_ok) {
        // Flip the overlay bit and push it live over the ABI.
        g_session.debug_flags ^= RLX_DEBUG_OVERLAY;
        rlx_set_debug(g_session.handle, g_session.debug_flags);
    } else if (cmd == kMenuReloadPresets && g_session.abi_ok) {
        reload_presets_keeping_selection(g_session.handle);
    } else if (cmd == kMenuOpenPresetDir) {
        open_preset_folder();
    } else if (listed != 0 && cmd >= kMenuPresetBase &&
               cmd < kMenuPresetBase + listed) {
        if (select_preset_named(
                g_session.handle,
                snap.names[static_cast<size_t>(cmd - kMenuPresetBase)])) {
            remember_current_preset(g_session.handle);
        }
    }
}

} // namespace


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
            if (handle_timer(wnd, wp)) return 0;
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
                        rlx_resize(g_session.handle, LOWORD(lp), HIWORD(lp));
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
                rlx_cycle_scene(g_session.handle);
                remember_current_preset(g_session.handle);
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
            PresetSnapshot snap;
            size_t listed = 0;
            HMENU menu = build_context_menu(snap, listed);
            if (menu == nullptr) return 0;
            const int cmd =
                TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, pt.x, pt.y,
                               0, wnd, nullptr);
            DestroyMenu(menu);
            // The menu pumped messages while it was up, so ownership can have
            // changed. This guard catches a dropped handle and a window this
            // session does not own - but NOT a handle that was REPLACED, which
            // keeps this owner and a non-null pointer and so passes here
            // carrying a roster that need not match `snap`. Re-resolving by
            // name inside `dispatch_menu_command` is what covers that case.
            if (g_session.owner != wnd || g_session.handle == nullptr) return 0;
            dispatch_menu_command(static_cast<UINT>(cmd), snap, listed);
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
        0, kWindowClass, L"Ritmolux",
        WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT, 1024, 640,
        core_api::get_main_window(), nullptr, core_api::get_my_instance(),
        nullptr);
}

} // namespace rlx

//! The standalone shell's entry point, and nothing else.
//!
//! Every part of the shell lives in its own module: [`cli`] judges the argument
//! list before a window exists, [`run`] opens the window and drives the winit
//! loop, [`app_state`] holds the running show, and [`hud`] and [`input`] carry
//! the two halves of what the operator sees and presses. [`show`] is what the
//! windowed and headless paths manage identically around the renderer.

mod app_state;
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "only the Linux capture backend reads bytes; built everywhere so its tests run"
    )
)]
mod capture_frames;
#[cfg(target_os = "linux")]
mod capture_linux;
#[cfg(target_os = "macos")]
mod capture_mac;
mod capture_start;
mod capture_verdict;
#[cfg(windows)]
mod capture_win;
mod cli;
mod console;
mod diaglog;
mod director;
mod downbeatlog;
mod hud;
mod input;
// Windows-only: the standalone's now-playing source (Plan 0097 / ADR-0110).
// macOS has no supported equivalent, so the banner exists there and is simply
// never fed — the same asymmetry loopback capture already has.
#[cfg(windows)]
mod nowplaying_win;
mod overlay;
mod preset_dir;
mod run;
mod settings;
mod show;
mod soak;
mod stream;
mod thumbs;

fn main() {
    // The thumbnail child (ADR-0230), before the launch path: this mode renders
    // one preset's still into the cache and exits, and the process asking for it
    // is the player itself rather than an operator. It opens no window, binds no
    // socket and starts no capture client, so it must not travel through a path
    // that does.
    if let Some(code) = thumbs::child_mode() {
        std::process::exit(code);
    }
    run::run();
}

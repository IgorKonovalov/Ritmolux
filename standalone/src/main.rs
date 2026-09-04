//! The standalone shell's entry point, and nothing else.
//!
//! Every part of the shell lives in its own module: [`cli`] judges the argument
//! list before a window exists, [`run`] opens the window and drives the winit
//! loop, [`app_state`] holds the running show, and [`hud`] and [`input`] carry
//! the two halves of what the operator sees and presses.

mod app_state;
#[cfg(target_os = "macos")]
mod capture_mac;
mod capture_start;
mod capture_verdict;
#[cfg(windows)]
mod capture_win;
mod cli;
mod config;
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
mod soak;
mod stream;

fn main() {
    run::run();
}

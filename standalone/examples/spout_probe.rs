//! Publish a known reference image as a Spout sender, and write the same image
//! to a PNG (Plan 0115 Phase 3).
//!
//! This is the instrument for the colour question ADR-0125 names as the
//! likeliest way the live video-out ships looking wrong, and it is the first
//! thing in this repository able to ask it: Spout's own demo sender publishes
//! its own animated content and takes no image, so nothing before a sender we
//! control could put *known bytes* on the wire.
//!
//! It answers two failures at a glance, which is why the pattern is shaped the
//! way it is:
//!
//! - **Channel order.** Four saturated patches — red, green, blue, mid-grey. A
//!   red/blue swap (the failure a sender left on `spoutDX`'s BGRA default
//!   produces) turns the red patch blue and is unmissable.
//! - **Transfer function.** A black-to-white ramp plus an sRGB mid-grey patch at
//!   exactly 128. The engine reads back `Rgba8UnormSrgb` — display-referred
//!   bytes — and Spout publishes an unlabelled 8-bit texture, so whether the
//!   receiver treats those bytes as sRGB or as linear is a receiver-side
//!   setting. Wrong, and the ramp bows and the grey patch lands visibly light or
//!   dark against the PNG beside it.
//!
//! **How to use it.** Run it, then in TouchDesigner put a `Syphon Spout In` TOP
//! next to a `Movie File In` TOP pointed at the PNG this wrote. They are the
//! same bytes, so they must look the same. If they do not, the receiving TOP
//! needs a colour setting, and finding that out here is the whole point.
//!
//! ```text
//! cargo run -p standalone --features spout --example spout_probe -- --gpu "RTX 3080"
//! cargo run -p standalone --features spout --example spout_probe -- --gpu 1
//! ```
//!
//! `--gpu` takes a name from the roster the probe prints on startup, or an
//! index, and is resolved by the same `standalone::gpu` code the stream mode
//! uses. **On a machine with two GPUs it is not optional in practice**: a Spout
//! receiver can only open a sender that lives on the GPU it renders with, and a
//! console process is handed the power-saving one by default.
//!
//! Requires the staged SDK (`packaging/spout/fetch-sdk.ps1`) and is gated on
//! the `spout` feature by `required-features`, so an ordinary build never
//! compiles it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use standalone::gpu::{SenderAdapter, sender_adapter};
use standalone::spout::{SpoutSender, adapters};

/// 1280x720: inside the 1280x1280 cap a TouchDesigner Non-Commercial key
/// imposes on the receiving TOP, and the size Plan 0115 Phase 4 streams at.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// A plausible live cadence, so the sender behaves like the stream mode will.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// Height of the liveness band along the bottom edge.
///
/// It eats the bottom 40 rows **of the patch row**, not of some spare margin:
/// the patches already run from mid-height to the bottom edge, so there is no
/// unused space to put it in. That leaves 320 rows of each patch, which is what
/// the colour and transfer-function comparison is read from, and the ramp above
/// is untouched.
const BAND_H: u32 = 40;

/// The sender name a receiver lists. Not necessarily the one it gets — see the
/// increment note in `standalone/src/spout/shim.cpp`.
const SENDER_NAME: &str = "lmv-probe";

fn main() {
    let rgba = reference_image();

    let png = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("spout-probe-reference.png");
    match image::save_buffer(&png, &rgba, WIDTH, HEIGHT, image::ExtendedColorType::Rgba8) {
        Ok(()) => eprintln!("reference written to {}", png.display()),
        Err(e) => eprintln!("could not write the reference PNG ({e}) — the sender still runs"),
    }

    // On a machine with two GPUs the receiver can only open a sender that lives
    // on the same one it renders with, so the roster is printed whatever
    // happens: a probe that fails silently on the wrong adapter is the exact
    // confusion this exists to prevent.
    let roster = adapters();
    for (index, name) in roster.iter().enumerate() {
        eprintln!("adapter [{index}] {name}");
    }
    let wanted = gpu_argument();
    // The probe has no renderer to follow, so with no `--gpu` it takes the
    // D3D11 default and says so; the stream mode's no-flag path follows its own
    // renderer's adapter instead.
    let adapter = match wanted.as_deref() {
        None if roster.len() > 1 => {
            eprintln!(
                "using the default adapter, but this machine has {} — if the receiver cannot \
                 open the sender, re-run with --gpu naming the one it renders on",
                roster.len()
            );
            None
        }
        None => None,
        Some(raw) => match sender_adapter(Some(raw), "", &roster) {
            Ok(SenderAdapter::Pinned(index)) => {
                eprintln!(
                    "using adapter [{index}] {}",
                    roster.get(index as usize).map_or("?", String::as_str)
                );
                Some(index)
            }
            Ok(SenderAdapter::Default { reason }) => {
                eprintln!("spout_probe: {reason}");
                None
            }
            Err(message) => {
                eprintln!("spout_probe: {message}");
                std::process::exit(2);
            }
        },
    };

    let mut sender = match SpoutSender::new(SENDER_NAME, WIDTH, HEIGHT, adapter) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("spout_probe: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "publishing {WIDTH}x{HEIGHT} as Spout sender '{}' — open a Syphon Spout In TOP. Ctrl-C to stop.",
        sender.name()
    );

    let mut frames: u64 = 0;
    let mut frame = rgba.clone();
    loop {
        // Sender pacing is a shell concern; the core stays clock-free.
        #[allow(
            clippy::disallowed_methods,
            reason = "probe frame pacing reads the wall clock; core analysis stays clock-free"
        )]
        let started = Instant::now();
        stamp_liveness(&mut frame, &rgba, frames);
        if let Err(e) = sender.send(&frame, WIDTH, HEIGHT) {
            eprintln!("spout_probe: frame {frames}: {e}");
            std::process::exit(1);
        }
        frames += 1;
        if frames.is_multiple_of(300) {
            eprintln!("{frames} frames sent");
        }
        #[allow(
            clippy::disallowed_methods,
            reason = "probe frame pacing reads the wall clock; core analysis stays clock-free"
        )]
        let spent = started.elapsed();
        if let Some(rest) = FRAME.checked_sub(spent) {
            std::thread::sleep(rest);
        }
    }
}

/// The `--gpu` argument, or a bare first argument, if there is one.
///
/// An argument rather than a constant because which adapter is right is a
/// property of the machine and of what is receiving, and neither is knowable
/// here. A bare argument is still accepted so the index form Phase 3 used keeps
/// working.
fn gpu_argument() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--gpu" {
            return match args.next() {
                Some(value) => Some(value),
                None => {
                    eprintln!("spout_probe: --gpu expects an adapter name or index");
                    std::process::exit(2);
                }
            };
        }
        if !arg.starts_with("--") {
            return Some(arg);
        }
    }
    None
}

/// Overwrite the reference's liveness marker for frame `frames`: a white block
/// that steps left to right across the bottom edge, one cell per 15 frames.
///
/// **A static test pattern cannot answer the question this probe exists to
/// ask.** A Spout receiver that loses its sender may keep presenting the last
/// texture it received, so a frozen frame and a live feed are the same picture.
/// The control that matters here is whether a sender on the *other* GPU
/// arrives, and its negative result looks exactly like a receiver still showing
/// the previous, working run. The marker separates them at a glance: moving
/// means live, parked means frozen, absent means nothing was ever received.
///
/// `base` is the untouched reference, so each call restores the row before
/// drawing - the marker never accumulates and the rest of the frame stays
/// byte-identical to the PNG written beside it.
fn stamp_liveness(frame: &mut [u8], base: &[u8], frames: u64) {
    /// Marker cells across the width. 16 keeps each cell wide enough to read
    /// from a thumbnail.
    const CELLS: u32 = 16;
    /// Frames each cell is lit for. 15 at 60 fps is four steps a second -
    /// unmistakably moving without strobing.
    const HOLD: u64 = 15;

    let band = HEIGHT.saturating_sub(BAND_H);
    let cell_w = WIDTH / CELLS;
    let lit = ((frames / HOLD) % u64::from(CELLS)) as u32;
    for y in band..HEIGHT {
        for x in 0..WIDTH {
            let at = (y as usize * WIDTH as usize + x as usize) * 4;
            let Some(slot) = frame.get_mut(at..at + 4) else {
                continue;
            };
            let inside = cell_w > 0 && x / cell_w == lit;
            if inside {
                slot.copy_from_slice(&[255, 255, 255, 255]);
            } else if let Some(original) = base.get(at..at + 4) {
                slot.copy_from_slice(original);
            }
        }
    }
}

/// The reference: a black-to-white ramp across the top half, four patches
/// across the bottom.
///
/// Tight, row-major, top-to-bottom RGBA8 — the same layout `CaptureImage`
/// returns, so this exercises the seam exactly as the stream mode will.
fn reference_image() -> Vec<u8> {
    /// sRGB mid-grey. 128 rather than a "perceptual" middle on purpose: it is
    /// the byte value, and the byte value is what has to survive the wire.
    const MID_GREY: [u8; 4] = [128, 128, 128, 255];
    const PATCHES: [[u8; 4]; 4] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        MID_GREY,
    ];

    let mut rgba = vec![0u8; WIDTH as usize * HEIGHT as usize * 4];
    let half = HEIGHT / 2;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let pixel = if y < half {
                // The ramp spans the full byte range end to end, so a receiver
                // that crushes blacks or clips whites shows it at the edges.
                let level = ((x * 255) / (WIDTH - 1)) as u8;
                [level, level, level, 255]
            } else {
                let column = (x * PATCHES.len() as u32 / WIDTH) as usize;
                PATCHES.get(column).copied().unwrap_or(MID_GREY)
            };
            let at = (y as usize * WIDTH as usize + x as usize) * 4;
            if let Some(slot) = rgba.get_mut(at..at + 4) {
                slot.copy_from_slice(&pixel);
            }
        }
    }
    rgba
}

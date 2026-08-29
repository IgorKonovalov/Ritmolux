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
//! cargo run -p standalone --features spout --example spout_probe
//! ```
//!
//! Requires the staged SDK (`packaging/spout/fetch-sdk.ps1`) and is gated on
//! the `spout` feature by `required-features`, so an ordinary build never
//! compiles it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use standalone::spout::SpoutSender;

/// 1280x720: inside the 1280x1280 cap a TouchDesigner Non-Commercial key
/// imposes on the receiving TOP, and the size Plan 0115 Phase 4 streams at.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// A plausible live cadence, so the sender behaves like the stream mode will.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

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

    let mut sender = match SpoutSender::new(SENDER_NAME, WIDTH, HEIGHT) {
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
    loop {
        // Sender pacing is a shell concern; the core stays clock-free.
        #[allow(
            clippy::disallowed_methods,
            reason = "probe frame pacing reads the wall clock; core analysis stays clock-free"
        )]
        let started = Instant::now();
        if let Err(e) = sender.send(&rgba, WIDTH, HEIGHT) {
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

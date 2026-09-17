//! The viewer: listen for Art-Net, reassemble frames, write one PNG each.
//!
//! **Nothing runs this automatically.** It is an instrument an author points at
//! a sender by hand, the same renderer-not-a-gate distinction `scripts/` draws
//! between the documentation renderers and the link checkers. What CI runs is
//! the library, through this crate's own tests and through `standalone`'s
//! integration tests.
//!
//! ```text
//! cargo run -p rlx-artnet-sim -- --listen 0.0.0.0:6454 --out target/rig/
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use rlx_artnet_sim::{RIG_PIXELS, RIG_UNIVERSES, Receiver, UdpCapture};

const USAGE: &str = "\
rlx-artnet-sim - decode Art-Net into the rig's raster and write a PNG per frame

USAGE:
    rlx-artnet-sim [OPTIONS]

OPTIONS:
    --listen <ADDR>     address to bind [default: 0.0.0.0:6454]
    --out <DIR>         directory for the PNG sequence [default: target/rig]
    --scale <N>         nearest-neighbour upscale factor [default: 4]
    --frames <N>        stop after N frames, 0 for no limit [default: 0]
    --universes <N>     universes in the rig [default: 24]
    --pixels <N>        pixels per universe [default: 170]
    --idle <SECONDS>    give up after this much silence [default: 5]
    -h, --help          print this
";

struct Options {
    listen: String,
    out: PathBuf,
    scale: u32,
    frames: usize,
    universes: usize,
    pixels: usize,
    idle: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:6454".to_string(),
            out: PathBuf::from("target/rig"),
            scale: 4,
            frames: 0,
            universes: RIG_UNIVERSES,
            pixels: RIG_PIXELS,
            idle: Duration::from_secs(5),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("rlx-artnet-sim: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let Some(options) = parse(std::env::args().skip(1))? else {
        print!("{USAGE}");
        return Ok(());
    };

    std::fs::create_dir_all(&options.out)
        .map_err(|err| format!("{}: {err}", options.out.display()))?;

    let addr = std::net::ToSocketAddrs::to_socket_addrs(&options.listen.as_str())
        .map_err(|err| format!("--listen `{}`: {err}", options.listen))?
        .next()
        .ok_or_else(|| format!("--listen `{}`: resolved to no address", options.listen))?;
    let mut capture = UdpCapture::bind(addr).map_err(|err| format!("bind {addr}: {err}"))?;
    capture
        .set_timeout(Some(options.idle))
        .map_err(|err| format!("read timeout: {err}"))?;
    let mut receiver = Receiver::new(options.universes, options.pixels);

    println!(
        "listening on {addr}, writing {} x {} frames to {}",
        options.pixels,
        options.universes,
        options.out.display()
    );

    let mut written = 0usize;
    let mut refused = 0u64;
    loop {
        let Some(datagram) = capture.recv().map_err(|err| format!("recv: {err}"))? else {
            // Silence for the whole idle window: close whatever is in flight so
            // the last frame of a run is not lost, then stop.
            if let Some(frame) = receiver.flush() {
                written += 1;
                write_frame(&options, written, &frame)?;
            }
            println!(
                "idle for {:?}; stopping after {written} frame(s)",
                options.idle
            );
            break;
        };
        match receiver.accept(&datagram) {
            Ok(Some(frame)) => {
                written += 1;
                write_frame(&options, written, &frame)?;
                if options.frames != 0 && written >= options.frames {
                    break;
                }
            }
            Ok(None) => {}
            Err(err) => {
                refused += 1;
                // One line per distinct failure would need a latch; one line per
                // refusal is right here because a viewer is watched, not logged.
                eprintln!("refused a datagram: {err}");
            }
        }
    }

    if refused > 0 {
        println!("{refused} datagram(s) refused");
    }
    Ok(())
}

fn write_frame(
    options: &Options,
    index: usize,
    frame: &rlx_artnet_sim::Frame,
) -> Result<(), String> {
    let path = options.out.join(format!("frame-{index:05}.png"));
    frame
        .raster
        .write_png(&path, options.scale)
        .map_err(|err| err.to_string())?;
    println!(
        "{}  {}/{} universes",
        path.display(),
        frame.seen_count(),
        frame.seen.len()
    );
    Ok(())
}

/// `Ok(None)` means `--help` was asked for.
fn parse(args: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--listen" => options.listen = value()?,
            "--out" => options.out = PathBuf::from(value()?),
            "--scale" => options.scale = number(&value()?, "--scale")?,
            "--frames" => options.frames = number(&value()?, "--frames")?,
            "--universes" => options.universes = number(&value()?, "--universes")?,
            "--pixels" => options.pixels = number(&value()?, "--pixels")?,
            "--idle" => options.idle = Duration::from_secs(number(&value()?, "--idle")?),
            other => return Err(format!("unknown argument `{other}`\n\n{USAGE}")),
        }
    }
    if options.universes == 0 || options.pixels == 0 {
        return Err("--universes and --pixels must both be above zero".to_string());
    }
    Ok(Some(options))
}

fn number<T: std::str::FromStr>(raw: &str, flag: &str) -> Result<T, String> {
    raw.parse()
        .map_err(|_| format!("{flag} `{raw}`: not a number"))
}

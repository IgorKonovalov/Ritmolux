//! The player's `--render` and `--bars` modes, run the way the studio runs them
//! (ADR-0262), against the `shot` CLI they are the shipped twin of.
//!
//! Each case gives the player and `shot` the same one-preset library through
//! `RLX_PRESET_DIR` and the same WAV, which the test writes because this repo
//! commits no audio. The rendering case needs a real adapter and **skips with a
//! printed reason** where there is none, keyed on the adapter error itself as
//! `shot_cli` does, so any other failure still fails. The bar-grid and refusal
//! cases build no renderer, so they run everywhere.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Output;

/// Substring of the error a run prints when no GPU adapter can be acquired.
const NO_ADAPTER: &str = "no suitable GPU adapter";

/// The library's single preset, bound to the audio so a frame stream that
/// ignored the clip would not pass for one that read it.
const PRESET_SRC: &str = r#"
system = "fragment_field"
name = "Render Probe"
[params]
zoom = "1.1 + 0.6 * bass"
"#;

const PRESET_NAME: &str = "Render Probe";

/// A fresh scratch directory inside the build tree, unique to this process and
/// this call.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("render-cli")
        .join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch dir");
    dir
}

/// A one-preset library under `dir`.
fn scratch_library(dir: &Path) -> PathBuf {
    let presets = dir.join("presets");
    std::fs::create_dir_all(&presets).expect("create the scratch library");
    std::fs::write(presets.join("probe.toml"), PRESET_SRC).expect("write the probe preset");
    presets
}

/// A canonical 44-byte-header WAV with `format` as its format code: 1 is PCM,
/// 3 is IEEE float, which the reader refuses.
fn wav_bytes(format: u16, channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let block_align = channels * 2;
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&format.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
    b.extend_from_slice(&block_align.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

/// A `secs`-long stereo clip with real dynamics at 48 kHz, written into `dir`.
fn clip(dir: &Path, secs: f32) -> PathBuf {
    let format = rlx_core::audio::AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    };
    let samples: Vec<i16> = rlx_core::signal::dynamic_groove(110.0, secs, format)
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * 32_767.0) as i16)
        .collect();
    let path = dir.join("clip.wav");
    std::fs::write(&path, wav_bytes(1, 2, 48_000, &samples)).expect("write the clip");
    path
}

/// The player with a data root of its own and `presets` as its library.
fn player(dir: &Path, presets: &Path, args: &[&str]) -> Output {
    let root = dir.join("player-data");
    std::fs::create_dir_all(&root).expect("create the player's data root");
    common::player_with_data_root(&root)
        .env("RLX_PRESET_DIR", presets)
        .args(args)
        .output()
        .expect("spawning the ritmolux binary")
}

/// `shot` with `presets` as its library.
fn shot(presets: &Path, args: &[&str]) -> Output {
    common::shot()
        .env("RLX_PRESET_DIR", presets)
        .args(args)
        .output()
        .expect("spawning the shot binary")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn skipped_for_no_adapter(out: &Output) -> bool {
    if !out.status.success() && stderr(out).contains(NO_ADAPTER) {
        eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
        return true;
    }
    false
}

/// **The player's `--render` is `shot --render`.** The same preset, clip, rate,
/// size and tier through both produce byte-identical Y4M streams. The tier is
/// `rich`, off both binaries' default, so a mode that dropped `--tier` would
/// render a different file rather than the same one by accident.
#[test]
fn the_players_render_is_byte_identical_to_shots() {
    let dir = scratch("identical");
    let presets = scratch_library(&dir);
    let wav = clip(&dir, 1.0);
    let wav = wav.to_string_lossy();
    let request = [
        "--render",
        &wav,
        "--preset",
        PRESET_NAME,
        "--fps",
        "30",
        "--size",
        "48x32",
        "--tier",
        "rich",
    ];

    let ours = player(&dir, &presets, &request);
    if skipped_for_no_adapter(&ours) {
        return;
    }
    assert!(
        ours.status.success(),
        "ritmolux --render failed\nstderr: {}",
        stderr(&ours)
    );
    let theirs = shot(&presets, &request);
    assert!(
        theirs.status.success(),
        "shot --render failed\nstderr: {}",
        stderr(&theirs)
    );

    let header = b"YUV4MPEG2 W48 H32 F30:1 Ip A1:1 C444 XCOLORRANGE=FULL\n";
    assert!(
        ours.stdout.starts_with(header),
        "the player's stream does not open with the declared header"
    );
    // ceil(1.0 s x 30 fps) = 30 complete 4:4:4 frames.
    assert_eq!(
        ours.stdout.len(),
        header.len() + 30 * (b"FRAME\n".len() + 48 * 32 * 3),
        "the player's stream is not header + 30 complete frames"
    );
    assert_eq!(
        ours.stdout.len(),
        theirs.stdout.len(),
        "the two streams differ in length"
    );
    assert!(
        ours.stdout == theirs.stdout,
        "ritmolux --render and shot --render wrote different bytes for the same request"
    );
}

/// **`--bars` writes the file `shot --render --bar-grid` writes.** `shot`
/// writes its grid before it builds a renderer, so the comparison holds on a
/// runner with no adapter too: there `shot` fails after the file is written,
/// and only for that reason.
#[test]
fn the_players_bars_match_shots_bar_grid() {
    let dir = scratch("bars");
    let presets = scratch_library(&dir);
    let wav = clip(&dir, 6.0);
    let wav = wav.to_string_lossy();
    let ours_path = dir.join("ours.json");
    let theirs_path = dir.join("theirs.json");

    let ours = player(
        &dir,
        &presets,
        &[
            "--bars",
            &wav,
            "--fps",
            "30",
            "--out",
            &ours_path.to_string_lossy(),
        ],
    );
    assert!(
        ours.status.success(),
        "ritmolux --bars failed\nstderr: {}",
        stderr(&ours)
    );
    assert!(
        ours.stdout.is_empty(),
        "--bars wrote to stdout, which belongs to no stream here"
    );

    let theirs = shot(
        &presets,
        &[
            "--render",
            &wav,
            "--preset",
            PRESET_NAME,
            "--fps",
            "30",
            "--size",
            "16x16",
            "--bar-grid",
            &theirs_path.to_string_lossy(),
        ],
    );
    assert!(
        theirs.status.success() || stderr(&theirs).contains(NO_ADAPTER),
        "shot --render --bar-grid failed for a reason other than the adapter\nstderr: {}",
        stderr(&theirs)
    );

    let ours = std::fs::read_to_string(&ours_path).expect("--bars wrote its file");
    let theirs = std::fs::read_to_string(&theirs_path).expect("shot wrote its grid");
    assert!(
        ours.starts_with("{\"fps\":\"30:1\",\"frames\":180,\"bar_starts\":[0"),
        "the grid is not six seconds at 30 fps from frame 0: {ours}"
    );
    assert_eq!(ours, theirs, "the two bar grids differ");
}

/// **A wrong request is refused with one line, and the exit code says which
/// kind.** A preset no library holds is the spelling of the request (2); a
/// clip that is not 16-bit PCM is its effect (1). Neither builds a renderer.
#[test]
fn an_unknown_preset_exits_2_and_a_non_pcm_wav_exits_1() {
    let dir = scratch("refusals");
    let presets = scratch_library(&dir);
    let wav = clip(&dir, 0.5);
    let wav = wav.to_string_lossy();

    let out = player(
        &dir,
        &presets,
        &["--render", &wav, "--preset", "No Such Preset"],
    );
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    let said = stderr(&out);
    assert_eq!(said.lines().count(), 1, "not one line: {said}");
    assert!(said.contains("No Such Preset"), "{said}");
    assert!(out.stdout.is_empty(), "a refused render wrote a stream");

    let float = dir.join("float.wav");
    std::fs::write(&float, wav_bytes(3, 2, 48_000, &[0; 960])).expect("write the float clip");
    let float = float.to_string_lossy();
    let grid = dir.join("grid.json");
    for args in [
        &["--render", &float, "--preset", PRESET_NAME][..],
        &["--bars", &float, "--out", &grid.to_string_lossy()],
    ] {
        let out = player(&dir, &presets, args);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
        let said = stderr(&out);
        assert_eq!(said.lines().count(), 1, "{args:?}: not one line: {said}");
        assert!(said.contains("PCM"), "{args:?}: {said}");
        assert!(
            out.stdout.is_empty(),
            "{args:?}: a refused run wrote a stream"
        );
    }
    assert!(!grid.exists(), "a refused --bars wrote its file");

    // A flag the mode does not read is the shape of the request, too.
    let out = player(
        &dir,
        &presets,
        &["--render", &wav, "--preset", PRESET_NAME, "--stream"],
    );
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("--stream"), "{}", stderr(&out));
}

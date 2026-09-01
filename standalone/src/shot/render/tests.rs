//! Unit tests for the offline render mode's pure halves (Plan 0101).
//!
//! The two clocks and the wire format are exactly the parts that can be wrong in
//! a way nobody notices: a frame-count off-by-one is a picture that ends a
//! sixtieth of a second before its soundtrack, and a mis-declared colour range is
//! a file that is washed out against the app for a reason that reads as an engine
//! bug. The GPU half is asserted from outside, in `standalone/tests/`.

use super::*;

fn stereo(rate: u32) -> AudioFormat {
    AudioFormat {
        sample_rate: rate,
        channels: 2,
    }
}

/// Interleaved sample count for `seconds` of `format`.
fn samples_for(seconds: f32, format: AudioFormat) -> usize {
    (seconds * format.sample_rate as f32) as usize * format.channels as usize
}

// ---------------------------------------------------------------------------
// --fps
// ---------------------------------------------------------------------------

#[test]
fn fps_parses_a_whole_rate_and_an_exact_rational() {
    assert_eq!(parse_fps("60"), Ok(Fps { num: 60, den: 1 }));
    assert_eq!(parse_fps("24"), Ok(Fps { num: 24, den: 1 }));
    assert_eq!(
        parse_fps("30000/1001"),
        Ok(Fps {
            num: 30_000,
            den: 1_001
        })
    );
    assert_eq!(parse_fps(" 30 / 1 "), Ok(Fps { num: 30, den: 1 }));

    // A decimal is rejected rather than approximated: 29.97 is 30000/1001, and
    // 2997/100 drifts a frame against the soundtrack every few minutes.
    let err = parse_fps("29.97").expect_err("a decimal rate");
    assert!(
        err.contains("30000/1001"),
        "the error names the form: {err}"
    );
    assert!(parse_fps("0").is_err(), "zero frames a second");
    assert!(parse_fps("60/0").is_err(), "zero denominator");
    assert!(parse_fps("-30").is_err(), "negative");
    assert!(parse_fps("fast").is_err(), "not a number");
}

/// The step is the rate, and at 60 fps it is bit-for-bit the step every other
/// capture path in this repo takes — which is the *only* reason a rendered frame
/// and a `--frame-at` PNG can be compared byte for byte at all.
#[test]
fn the_step_at_sixty_is_the_capture_paths_own_step() {
    assert_eq!(DEFAULT_FPS.dt(), 1.0 / 60.0);
    assert_eq!(Fps { num: 30, den: 1 }.dt(), 1.0 / 30.0);
    // The rational rate is not the same number as its decimal, which is the
    // whole reason `Fps` is not an `f32`.
    let ntsc = Fps {
        num: 30_000,
        den: 1_001,
    };
    assert!(ntsc.dt() > 1.0 / 30.0, "29.97 fps steps slightly longer");
    assert_eq!(ntsc.as_header_field(), "30000:1001");
}

// ---------------------------------------------------------------------------
// The two clocks
// ---------------------------------------------------------------------------

/// `ceil(clip_seconds x fps)`, which is the done-when Phase 1 is asserted on.
#[test]
fn the_frame_count_is_the_ceiling_of_the_clips_length() {
    let format = stereo(48_000);
    // Four seconds, the length `--signal` synthesizes.
    let four = samples_for(4.0, format);
    assert_eq!(frame_count(four, format, DEFAULT_FPS), Ok(240));
    assert_eq!(frame_count(four, format, Fps { num: 30, den: 1 }), Ok(120));
    assert_eq!(frame_count(four, format, Fps { num: 24, den: 1 }), Ok(96));

    // A clip that does not divide evenly gets its trailing partial frame, so the
    // picture is never shorter than the audio it is muxed against.
    let ragged = samples_for(4.0, format) + format.channels as usize;
    assert_eq!(
        frame_count(ragged, format, DEFAULT_FPS),
        Ok(241),
        "one sample past four seconds still needs a 241st frame"
    );

    // Mono halves the interleaved samples per audio frame, so the same count is
    // twice the clip.
    let mono = AudioFormat {
        sample_rate: 48_000,
        channels: 1,
    };
    assert_eq!(frame_count(four, mono, DEFAULT_FPS), Ok(480));

    // A four-minute track at 1080p/60 — the length Phase 4 is asserted on.
    assert_eq!(
        frame_count(samples_for(240.0, format), format, DEFAULT_FPS),
        Ok(14_400)
    );

    // Nothing to render is an error, not a zero-frame stream nobody can play.
    let err = frame_count(0, format, DEFAULT_FPS).expect_err("an empty clip");
    assert!(err.contains("shorter than one frame"), "got {err}");
}

/// The hop clock and the frame clock are different clocks, and this is the map.
///
/// At 48 kHz a hop is 512 samples, so hops arrive at 93.75 Hz against a 60 Hz
/// frame clock: most frames take one new hop and some take two. Rendering one
/// hop per frame — the mistake this function exists to prevent — would run the
/// picture at 64% speed against its own soundtrack.
#[test]
fn the_hop_clock_and_the_frame_clock_advance_independently() {
    let format = stereo(48_000);

    // 93.75 hops a second against 60 frames a second: by the end of frame 0
    // (1/60 s) one hop has completed, and by the end of frame 59 (one second)
    // ninety-three have.
    assert_eq!(hops_through(0, DEFAULT_FPS, format), 1);
    assert_eq!(hops_through(59, DEFAULT_FPS, format), 93);
    assert_eq!(hops_through(119, DEFAULT_FPS, format), 187);

    // Some frames take two hops and some take one — that unevenness *is* the
    // two clocks, and a schedule where every gap were 1 would be the bug.
    let steps: Vec<usize> = (1..60)
        .map(|i| hops_through(i, DEFAULT_FPS, format) - hops_through(i - 1, DEFAULT_FPS, format))
        .collect();
    assert!(steps.contains(&1) && steps.contains(&2), "{steps:?}");
    assert!(
        steps.iter().all(|s| *s <= 2),
        "no frame skips a hop: {steps:?}"
    );

    // A frame rate *above* the hop rate leaves frames with no new hop at all,
    // which the loop resolves by repeating the last published frame.
    let fast = Fps { num: 240, den: 1 };
    let repeats = (1..12)
        .filter(|i| hops_through(*i, fast, format) == hops_through(i - 1, fast, format))
        .count();
    assert!(repeats > 0, "240 fps must outrun a 93.75 Hz hop clock");

    // 30,720 Hz is the one rate where the two clocks coincide — 60 hops a
    // second — and it is what the tap-identity fixture is built at.
    let locked = stereo(60 * HOP_SIZE as u32);
    for frame in 0..120 {
        assert_eq!(
            hops_through(frame, DEFAULT_FPS, locked),
            frame as usize + 1,
            "at 60 x HOP_SIZE Hz each frame takes exactly one hop"
        );
    }
}

/// The schedule is monotone and never runs ahead of the audio — a hop cannot be
/// consumed before the samples in it have played.
#[test]
fn the_hop_schedule_never_runs_ahead_of_the_clip() {
    let format = stereo(44_100);
    let fps = Fps { num: 25, den: 1 };
    let mut previous = 0;
    for frame in 0..500u32 {
        let due = hops_through(frame, fps, format);
        assert!(due >= previous, "the schedule went backwards at {frame}");
        // Every hop counted has genuinely completed: its last sample plays at or
        // before the end of this frame.
        let hop_end_samples = due as u64 * HOP_SIZE as u64;
        let frame_end_samples =
            (u64::from(frame) + 1) * u64::from(fps.den) * u64::from(format.sample_rate)
                / u64::from(fps.num);
        assert!(
            hop_end_samples <= frame_end_samples,
            "frame {frame} claimed hop {due}, which has not played yet"
        );
        previous = due;
    }
}

// ---------------------------------------------------------------------------
// The wire format
// ---------------------------------------------------------------------------

/// The header carries everything a consumer needs to decode the stream without
/// being told anything on the command line — the self-describing requirement
/// ADR-0114 exists to state.
#[test]
fn the_stream_header_declares_its_own_geometry_rate_and_range() {
    let header = y4m_header(1920, 1080, DEFAULT_FPS);
    assert_eq!(
        header, "YUV4MPEG2 W1920 H1080 F60:1 Ip A1:1 C444 XCOLORRANGE=FULL\n",
        "the header is a wire format, so it is asserted verbatim"
    );
    // The magic and the terminator are what make it parseable at all.
    assert!(header.starts_with("YUV4MPEG2 "));
    assert!(header.ends_with('\n'));

    // The rate is the rational, not a rounded decimal.
    let ntsc = y4m_header(
        1280,
        720,
        Fps {
            num: 30_000,
            den: 1_001,
        },
    );
    assert!(ntsc.contains(" F30000:1001 "), "got {ntsc}");
    assert!(!ntsc.contains("29.97"));

    // Full range is the half most likely to ship wrong: without it every player
    // expands 16-235 to 0-255 and the file is visibly darker than the app.
    assert!(header.contains("XCOLORRANGE=FULL"));
    // 4:4:4 — chroma is not subsampled, so the conversion loses only rounding.
    assert!(header.contains(" C444 "));
}

/// The colour conversion is near-identity, and "near" is measured rather than
/// asserted by hand-wave (Plan 0101 Phase 3).
///
/// An 8-bit RGB->YUV->RGB round trip cannot be exact — that is precisely why the
/// tap-placement assertion is made on the RGB frame and not on the wire bytes —
/// but it must be tight enough that no visible shift is hiding in it.
#[test]
fn the_colour_conversion_round_trips_to_within_a_level() {
    /// Largest per-channel error over the swept colours. Two levels out of 256
    /// is under 1%, and is what an exact-in-float, rounded-twice conversion
    /// costs; anything larger would be a wrong matrix rather than rounding.
    const TOLERANCE: i32 = 2;

    let mut worst = 0i32;
    // A coarse sweep of the whole cube plus the corners and the greys, which is
    // where a swapped coefficient shows up first.
    for r in (0..=255u16).step_by(17) {
        for g in (0..=255u16).step_by(17) {
            for b in (0..=255u16).step_by(17) {
                let (r, g, b) = (r as u8, g as u8, b as u8);
                let (y, u, v) = rgb_to_yuv(r, g, b);
                let (r2, g2, b2) = yuv_to_rgb(y, u, v);
                for (before, after) in [(r, r2), (g, g2), (b, b2)] {
                    let error = (i32::from(before) - i32::from(after)).abs();
                    worst = worst.max(error);
                    assert!(
                        error <= TOLERANCE,
                        "({r},{g},{b}) round-tripped to ({r2},{g2},{b2}) — \
                         channel error {error} past {TOLERANCE}"
                    );
                }
            }
        }
    }
    eprintln!("worst round-trip channel error: {worst}");

    // Black and white are exact, and their chroma is neutral: a full-range
    // conversion must not clip either end, which is the studio-swing bug.
    assert_eq!(rgb_to_yuv(0, 0, 0), (0, 128, 128));
    assert_eq!(rgb_to_yuv(255, 255, 255), (255, 128, 128));
    // ...and grey stays grey rather than acquiring a cast.
    assert_eq!(rgb_to_yuv(128, 128, 128), (128, 128, 128));
    // A primary lands where BT.709 says it does — luma weights, not BT.601's.
    let (y, _, _) = rgb_to_yuv(0, 255, 0);
    assert_eq!(y, 182, "0.7152 x 255 = 182.4; BT.601 would read 150");
}

/// The frozen colour table, asserted here and in `tools/sd-filter/test_sd_filter.py`.
///
/// THE TWIN OF THIS TEST is the `RGB_TO_YUV` / `YUV_TO_RGB` tables in
/// `tools/sd-filter/test_sd_filter.py`, which assert these exact numbers against
/// that tool's `rgb_to_yuv444` / `yuv444_to_rgb`. Neither file may be edited
/// alone: a one-sided edit reddens the side it was made on, and that is the
/// whole mechanism.
///
/// Why it exists (Plan 0106 Phase 7b). The diffusion filter re-implements this
/// conversion in Python because it has to decode the stream to diffuse it, and
/// nothing checked the pair. The filter's pass-through never converts and its
/// diffused output is not reproducible across machines, so no test on either
/// side of the seam touched it — a constant edited in one language would ship as
/// a colour cast across every diffused frame, and no instrument in this repo
/// could see it.
///
/// What this pin is sensitive to, measured rather than assumed: it reddens on any
/// single-coefficient edit of ±0.0005 or larger, in either direction, to any of
/// the five forward constants. Below that it starts to miss — the worst case a
/// ±0.0002 luma edit can shift a channel is 0.05 of one 8-bit level, which is
/// under the quantization floor of the format itself and cannot produce a cast
/// the pin exists to catch. Structural errors — swapped Cb/Cr, BT.601 weights, a
/// missing +128, a wrap where the clamp belongs — move these rows by tens of
/// levels and are caught outright.
///
/// This is a property, not a measurement (ADR-0071): the output is exact 8-bit
/// integers on every machine, so the table names no configuration and carries no
/// tolerance. It is deliberately separate from the round-trip sweep above, which
/// asserts a *tolerance* on the pair composed — a different question, and one
/// that cannot catch a matching error made in both directions.
#[test]
fn the_colour_table_is_pinned_to_its_python_twin() {
    // RGB -> planar C444. Pure red's Cr computes to 255.5 and pure cyan's to
    // 0.5, so those two rows are the whole forward-direction clamp: across the
    // entire 8-bit cube the chroma terms reach exactly half a level past each
    // end and no further.
    const RGB_TO_YUV: &[([u8; 3], [u8; 3])] = &[
        ([0, 0, 0], [0, 128, 128]),
        ([255, 255, 255], [255, 128, 128]),
        ([128, 128, 128], [128, 128, 128]),
        ([255, 0, 0], [54, 99, 255]), // Cr 255.5 -> clamped, not wrapped
        ([0, 255, 0], [182, 30, 12]), // luma 182: BT.709; BT.601 would read 150
        ([0, 0, 255], [18, 255, 116]),
        ([0, 255, 255], [201, 157, 1]), // Cr 0.5, the other end of the same edge
        ([255, 0, 255], [73, 226, 244]),
        ([255, 255, 0], [237, 1, 140]),
        ([250, 7, 7], [59, 100, 250]),
        ([7, 7, 250], [25, 250, 117]),
        ([18, 52, 86], [47, 149, 109]),
    ];

    // Planar C444 -> RGB. This is where the clamp genuinely bites: an arbitrary
    // YUV triple is not the image of any RGB one, so the terms leave `0..=255`
    // by a wide margin. Five of these seven rows clamp at one end or the other,
    // which is why the inverse direction carries its own table rather than being
    // asserted as a round-trip of the one above.
    const YUV_TO_RGB: &[([u8; 3], [u8; 3])] = &[
        ([0, 0, 0], [0, 84, 0]),            // R -201.6 and B -237.5, both clamped low
        ([255, 255, 255], [255, 172, 255]), // R 455 and B 490, both clamped high
        ([128, 128, 128], [128, 128, 128]),
        ([16, 240, 16], [0, 47, 224]),
        ([240, 16, 240], [255, 209, 32]),
        ([200, 20, 235], [255, 170, 0]),
        ([54, 128, 255], [254, 0, 54]),
    ];

    for &([r, g, b], want) in RGB_TO_YUV {
        let (y, u, v) = rgb_to_yuv(r, g, b);
        assert_eq!(
            [y, u, v],
            want,
            "rgb({r},{g},{b}) — moved; so must the twin table in tools/sd-filter/test_sd_filter.py"
        );
    }

    for &([y, u, v], want) in YUV_TO_RGB {
        let (r, g, b) = yuv_to_rgb(y, u, v);
        assert_eq!(
            [r, g, b],
            want,
            "yuv({y},{u},{v}) — moved; so must the twin table in tools/sd-filter/test_sd_filter.py"
        );
    }
}

/// A written frame is the marker plus three full-resolution planes, in the order
/// the header's `C444` promises — asserted on bytes, since a plane-order slip
/// swaps the red and blue of every exported video.
#[test]
fn a_written_frame_is_a_marker_and_three_planes() {
    // Two pixels: pure red then pure blue.
    let img = CaptureImage {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
    };
    let mut planes = Vec::new();
    let mut out: Vec<u8> = Vec::new();
    write_y4m_frame(&img, &mut planes, &mut out).expect("a Vec never fails to write");

    assert_eq!(&out[..6], b"FRAME\n");
    let body = &out[6..];
    assert_eq!(body.len(), 2 * 3, "one Y, one Cb and one Cr per pixel");

    let (red, blue) = (rgb_to_yuv(255, 0, 0), rgb_to_yuv(0, 0, 255));
    assert_eq!(&body[0..2], &[red.0, blue.0], "the luma plane comes first");
    assert_eq!(&body[2..4], &[red.1, blue.1], "then Cb for both pixels");
    assert_eq!(&body[4..6], &[red.2, blue.2], "then Cr");

    // Planar, not interleaved: red's three samples must not be adjacent.
    assert_ne!(&body[0..3], &[red.0, red.1, red.2], "packed, not planar");

    // The scratch buffer is reused across frames without growing.
    let before = planes.capacity();
    write_y4m_frame(&img, &mut planes, &mut out).expect("second frame");
    assert_eq!(planes.len(), 6);
    assert_eq!(
        planes.capacity(),
        before,
        "a long render reallocates nothing"
    );
}

// ---------------------------------------------------------------------------
// Which preset, decided before anything is spent
// ---------------------------------------------------------------------------

/// A roster of empty presets under the given names — enough for a membership
/// test, which reads nothing but `name`.
fn roster(names: &[&str]) -> Vec<Preset> {
    names
        .iter()
        .map(|n| {
            Preset::from_toml_str(&format!("system = \"attractor\"\nname = \"{n}\"\n"))
                .expect("a minimal preset parses")
        })
        .collect()
}

/// The rejection is the whole point of the check: it happens **before** the
/// encoder is spawned and before a device is built, and it hands back the keys
/// the roster is actually keyed on. `attractor_leviathan` is the reproduction —
/// the *filename* of a preset whose `name` is `Leviathan`.
#[test]
fn an_unknown_preset_is_rejected_and_the_error_names_the_rosters_keys() {
    let presets = roster(&["Leviathan", "Drift Field"]);

    let err = resolve_preset(Some("attractor_leviathan"), &presets)
        .expect_err("a filename is not a roster key");
    assert!(
        err.contains("attractor_leviathan"),
        "the rejected name is quoted back: {err}"
    );
    assert!(
        err.contains("Leviathan") && err.contains("Drift Field"),
        "every roster key is offered: {err}"
    );

    // Exact equality, the same comparison the renderer makes when it selects: a
    // near-miss is a miss, or the check would pass a name the renderer rejects.
    assert!(resolve_preset(Some("leviathan"), &presets).is_err());
    assert!(resolve_preset(Some("Leviathan "), &presets).is_err());
    assert_eq!(
        resolve_preset(Some("Leviathan"), &presets),
        Ok("Leviathan".to_string())
    );
}

/// The two `--preset`-less arms, unchanged by the membership test: a one-entry
/// roster names itself, and a longer one has to be told which.
#[test]
fn an_unnamed_preset_resolves_only_against_a_one_entry_roster() {
    assert_eq!(
        resolve_preset(None, &roster(&["Leviathan"])),
        Ok("Leviathan".to_string())
    );

    let err = resolve_preset(None, &roster(&["Leviathan", "Drift Field"]))
        .expect_err("two presets, no name");
    assert!(err.contains("--preset"), "the error names the flag: {err}");
    assert!(resolve_preset(None, &roster(&[])).is_err());
}

// ---------------------------------------------------------------------------
// The one canonical encoder invocation
// ---------------------------------------------------------------------------

/// The `ffmpeg` command line is a **support surface**: it will be wrong on
/// somebody's build, and the whole point of generating it is that there is one
/// of it to fix. This pins the parts that are load-bearing rather than
/// stylistic, so a well-meant tidy-up cannot quietly drop the colour tags or the
/// audio mapping.
#[test]
fn the_generated_ffmpeg_command_carries_its_inputs_mapping_and_colour() {
    let args = ffmpeg_args(
        std::path::Path::new("track.wav"),
        std::path::Path::new("out.mp4"),
        DEFAULT_CRF,
    );
    let line = args.join(" ");

    // Two inputs: the frame stream on stdin, then the source WAV untouched.
    assert!(line.contains("-f yuv4mpegpipe -i pipe:0"), "{line}");
    assert!(line.contains("-i track.wav"), "{line}");
    // ...explicitly mapped, so a clip with a video stream of its own (album art)
    // cannot displace the rendered picture.
    assert!(line.contains("-map 0:v:0 -map 1:a:0"), "{line}");

    // No geometry is passed: it is on the wire, which is the point of a
    // self-describing stream. `-s`/`-pix_fmt` *input* flags reappearing here
    // would reintroduce exactly the mistyping ADR-0114 refuses.
    assert!(
        !line.contains("-s "),
        "geometry must come off the stream: {line}"
    );

    // The colour declaration, which is the half most likely to ship wrong: an
    // untagged file gets expanded from studio swing and shows washed out.
    for tag in [
        "-color_range pc",
        "-colorspace bt709",
        "-color_primaries bt709",
        "-color_trc bt709",
    ] {
        assert!(line.contains(tag), "missing `{tag}` in: {line}");
    }

    // Audio is encoded rather than dropped — a music video without the music is
    // the one failure nobody would need a test to notice, and every test needs
    // to notice it before that.
    assert!(line.contains("-c:a aac"), "{line}");
    assert!(line.contains("-shortest"), "{line}");

    // The destination is last and unquoted — the caller passes it as one argv
    // entry, so a path with spaces needs no escaping here.
    assert_eq!(args.last().map(String::as_str), Some("out.mp4"));
    // Overwrite without prompting: an encoder waiting on a y/n at the far end of
    // a pipe is a render that hangs with no explanation.
    assert!(args.iter().any(|a| a == "-y"), "{line}");

    // The default is archival and stays archival: `--crf` adds a lever beside it
    // and does not move it.
    assert!(line.contains("-crf 18"), "{line}");
}

/// `--crf` moves exactly one argument. Everything the previous test pins
/// *describes the stream* — geometry, colour, mapping — and a size lever that
/// dropped one of those would be a worse defect than the archival default it was
/// added to work around.
#[test]
fn a_moved_crf_changes_the_rate_and_nothing_that_describes_the_stream() {
    let clip = std::path::Path::new("track.wav");
    let out = std::path::Path::new("out.mp4");

    let default = ffmpeg_args(clip, out, DEFAULT_CRF);
    let moved = ffmpeg_args(clip, out, 23);

    assert!(moved.join(" ").contains("-crf 23"));
    assert_eq!(
        default.len(),
        moved.len(),
        "a rate change adds and removes no argument"
    );

    // The one differing position is the value after `-crf`, and every colour tag
    // and mapping argument is byte-identical across the two.
    let differing: Vec<usize> = (0..default.len())
        .filter(|&i| default[i] != moved[i])
        .collect();
    assert_eq!(
        differing,
        vec![
            default
                .iter()
                .position(|a| a == "-crf")
                .expect("a -crf flag")
                + 1
        ]
    );
}

/// The range is x264's own. Outside it the encoder rejects the whole command
/// line, and the failure arrives as the encoder's diagnostics through a pipe
/// rather than as a named flag error.
#[test]
fn crf_parses_its_range_and_refuses_what_the_encoder_would() {
    assert_eq!(parse_crf("23"), Ok(23));
    assert_eq!(parse_crf(" 0 "), Ok(0));
    assert_eq!(parse_crf("51"), Ok(51));

    for bad in ["52", "-1", "18.5", "", "high"] {
        let err = parse_crf(bad).expect_err("{bad} is not a crf");
        assert!(err.contains("--crf"), "the error names the flag: {err}");
    }
}

// ---------------------------------------------------------------------------
// The resident set across a long render
// ---------------------------------------------------------------------------

/// **Growth is measured from the warm reading, not from the baseline** — which is
/// the difference between an instrument and a false alarm (Plan 0101 Phase 4).
///
/// The measured series on the Windows dev box steps once, by 76 MB, at the first
/// sampled frame and is then flat to the end: pipelines compiled and GPU
/// resources built on the first draw. Charged against the baseline that prints as
/// "+76 MB" and reads exactly like the linear per-frame retention Plan 0099
/// found. This case is that shape, and it must read flat.
#[test]
fn the_resident_set_line_separates_warm_up_from_growth_across_the_run() {
    const MB: u64 = 1024 * 1024;

    // The measured shape: one step, then twenty flat readings.
    let mut warmed = vec![357 * MB];
    warmed.extend(std::iter::repeat_n(434 * MB, 20));
    let line = ResidentSet { samples: warmed }.summary(600);
    assert!(
        line.contains("growth +0.0 MB across 600 frames"),
        "a one-step warm-up is not growth: {line}"
    );
    assert!(
        line.contains("a +77.0 MB warm-up"),
        "the startup step is reported rather than hidden: {line}"
    );
    assert!(line.contains("resident set 434 MB"), "got {line}");
    assert!(line.contains("peak 434 MB"), "got {line}");
    assert!(line.contains("21 samples"), "got {line}");

    // Real growth, charged from the warm reading: 330 warm, 346 at the end.
    let leaked = ResidentSet {
        samples: vec![300 * MB, 330 * MB, 338 * MB, 346 * MB],
    };
    let line = leaked.summary(14_400);
    assert!(line.contains("growth +16.0 MB"), "got {line}");
    assert!(line.contains("a +30.0 MB warm-up"), "got {line}");

    // An excursion that was reclaimed reads flat end to end, and only the peak
    // tells it apart from a run that never grew.
    let spiked = ResidentSet {
        samples: vec![300 * MB, 330 * MB, 400 * MB, 330 * MB],
    };
    let line = spiked.summary(600);
    assert!(line.contains("growth +0.0 MB"), "got {line}");
    assert!(
        line.contains("peak 400 MB"),
        "the excursion is visible: {line}"
    );

    // Reclaimed below the warm reading: negative, not an underflow. Both
    // subtractions are signed for exactly this — `u64` here would print
    // 17 exabytes and read as a catastrophic leak.
    let shrank = ResidentSet {
        samples: vec![300 * MB, 340 * MB, 330 * MB],
    };
    assert!(
        shrank.summary(1).contains("growth -10.0 MB"),
        "got {}",
        shrank.summary(1)
    );

    // A run with only a baseline has no warm reading to charge against, and
    // reports zero of each rather than dividing by a sample it does not have.
    let one = ResidentSet {
        samples: vec![300 * MB],
    };
    let line = one.summary(1);
    assert!(line.contains("growth +0.0 MB"), "got {line}");
    assert!(line.contains("a +0.0 MB warm-up"), "got {line}");

    // No reading at all is said rather than shown as a flat zero, which would be
    // a leak-free verdict nobody measured.
    let none = ResidentSet::default();
    assert_eq!(
        none.summary(14_400),
        "render: resident set unavailable on this platform"
    );
}

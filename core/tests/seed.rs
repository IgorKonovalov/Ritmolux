//! Plan 0047 Phase 2 / ADR-0051: `seed = "random"`, and the pin that keeps it
//! out of the harness.
//!
//! The feature is a deliberate collision — a preset may look different every time
//! the app launches, while the capture harness's entire value is that a frame
//! reproduces byte for byte. It is resolved the way ADR-0045 resolves quality
//! tiers: the live app varies, every capture path pins. So there are two claims
//! to defend and they pull in opposite directions:
//!
//! 1. two loads of a `seed = "random"` preset really do draw different salts
//!    (otherwise the feature does nothing), and
//! 2. a capture of that same preset is byte-identical anyway (otherwise every
//!    golden, gate and `--report` run becomes a coin toss).
//!
//! The second test asserts both at once, which is what makes it non-vacuous:
//! if the pin were dropped, the salts it checks differ *are* what the pixels
//! would then be drawn from.

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;

mod common;

const SIZE: u32 = 64;
/// Enough frames for the fragment field to be fully established; the fixture
/// binds nothing time-varying, so this only clears any first-frame transient.
const FRAMES: u32 = 8;

/// A fragment-field preset whose **colour** is a pure function of the salt, so a
/// changed salt is a changed picture rather than a subtle one. `hue` is bound to
/// a constant argument on purpose: with no `time` or band term, the only thing
/// that can move this preset's pixels between two runs is the seed.
///
/// No `trails` and no `[palette]`: one scene pipeline and nothing accumulating,
/// which is the configuration WARP is faithful on (see `composite.rs`).
fn source(seed: &str) -> String {
    format!(
        r#"
system = "fragment_field"
name = "seed_probe"

[params]
hue = "hash(1)"
warp = "0.35"
zoom = "1.0"

[generator]
seed = {seed}
"#
    )
}

fn load(seed: &str) -> Preset {
    Preset::from_toml_str(&source(seed)).unwrap_or_else(|e| panic!("seed fixture parses: {e}"))
}

/// A quiet frame — this fixture binds no band, so the audio is irrelevant and
/// saying so with silence keeps the capture's inputs minimal.
fn silent() -> AnalysisFrame {
    AnalysisFrame::default()
}

/// The two salts a preset carries, and what each of the three declaration forms
/// resolves them to (ADR-0051).
#[test]
fn a_random_seed_redraws_each_load_while_the_pinned_salt_stays_put() {
    // Declared nothing: both salts `0`, which is every preset shipped today.
    let bare = load("0");
    assert_eq!(bare.salt, 0);
    assert_eq!(bare.pinned_salt, 0);

    // Declared a number: one salt, used live and in a capture alike — there is
    // nothing to vary, so the two must not diverge.
    let fixed = load("7");
    assert_ne!(fixed.salt, 0, "a declared seed reaches the salt");
    assert_eq!(
        fixed.salt, fixed.pinned_salt,
        "a numeric seed captures under exactly the salt it renders under"
    );
    assert_eq!(load("7").salt, fixed.salt, "a number is reproducible");

    // Declared `"random"`: a fresh salt per load, and a pinned twin that is the
    // numeric fallback whatever the draw was.
    let a = load("\"random\"");
    let b = load("\"random\"");
    assert_ne!(
        a.salt, b.salt,
        "two loads of a random-seeded preset must draw different salts"
    );
    assert_eq!(a.pinned_salt, 0, "a capture sees the numeric fallback");
    assert_eq!(b.pinned_salt, 0);
}

/// A `seed` value that is neither a non-negative integer nor `"random"` is a
/// surfaced load error — never a panic, and never a silent fallback to `0`, which
/// would hide the typo behind a preset that renders perfectly well.
#[test]
fn a_malformed_seed_is_a_surfaced_load_error() {
    for bad in ["\"randmo\"", "\"\"", "-1", "true", "1.5", "[1]"] {
        let result = Preset::from_toml_str(&source(bad));
        assert!(
            result.is_err(),
            "`seed = {bad}` must be rejected, not accepted as a salt"
        );
    }
}

/// The pin, end to end: the same `seed = "random"` preset loaded twice — drawing
/// two different live salts, asserted here — captures to identical bytes.
///
/// This is the property every golden baseline, every behavioral gate and every
/// `--report` run rests on. Without the pin the two captures would be drawn from
/// the two salts this test has just shown to differ.
#[test]
fn a_random_seeded_preset_captures_byte_identically() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let frame = silent();

    let first = load("\"random\"");
    let second = load("\"random\"");
    assert_ne!(
        first.salt, second.salt,
        "sanity: the two loads drew different live salts, so the capture below \
         has something to pin"
    );

    renderer.set_presets(vec![first]);
    let a = renderer
        .capture_preset("seed_probe", &frame, FRAMES)
        .expect("first capture");

    renderer.set_presets(vec![second]);
    let b = renderer
        .capture_preset("seed_probe", &frame, FRAMES)
        .expect("second capture");

    assert_eq!(
        a.rgba, b.rgba,
        "a capture must not depend on the salt a load happened to draw"
    );

    // ...and the capture is not simply blind to the salt: the same fixture under
    // a *declared* seed renders differently, so the equality above is a pin
    // rather than a preset whose pixels ignore `hash` altogether.
    renderer.set_presets(vec![load("7")]);
    let seeded = renderer
        .capture_preset("seed_probe", &frame, FRAMES)
        .expect("third capture");
    assert_ne!(
        a.rgba, seeded.rgba,
        "the fixture's pixels must actually depend on its salt, or this file \
         proves nothing"
    );
}

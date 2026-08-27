//! **The sample sheet Plan 0113 Phase 5 judges**, plus the two claims about the
//! grammar that are only true at the *rendered* level (Plan 0113 Phase 4).
//!
//! Phase 5 is a human gate: a seeded layout can satisfy every statistic and
//! still fail to read as a composition, and no test can tell us which one does.
//! So this file's job is to make the candidates **comparable** — one image per
//! `(grammar, seed)` cell, same count, same palette, same everything else — and
//! then to get out of the way.
//!
//! # Where the images go, and why they are not committed
//!
//! `LMV_SAMPLE_DIR=<dir> cargo test -p lmv-core --test collage_layout` writes the
//! sheet there and prints the path. Without the variable the sweep still runs
//! and asserts, and writes nothing: these are **samples for a human to look at
//! once**, not baselines. Committing twenty PNGs that no test compares against
//! would be twenty files nothing can ever invalidate — the golden suite is where
//! a pinned picture belongs, and `core/tests/golden/shape_collage.png` is the
//! one this system has.
//!
//! # The four columns
//!
//! Three are the generator's grammars and the fourth is the **control**: the
//! hand-authored canvas Phase 1 shipped, which is the one composition a human
//! has already approved. Judging three generated candidates with nothing to
//! judge them against is how a gate ends up picking the best of three bad
//! options; `presets/README.md` carries what each `layout` number means.
//!
//! # What is asserted here, and what is asserted elsewhere
//!
//! The **list-level** claims — bit-identical output for an equal recipe, no
//! reallocation across a thousand recompositions, the three grammars being
//! distinct — are in `core/src/render/scenes/shape_collage/tests.rs`, where the
//! element array is reachable and a difference cannot hide under a rasterizer's
//! tolerance. An integration test sees the public API only. What it can say that
//! the unit tests cannot is that the whole path — preset, param, quantizer,
//! generator, buffer, shader — carries a seed to the screen, and that is what it
//! says.

use std::path::PathBuf;

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Sample-sheet render size. 1080p, because these are judged by eye at
/// full size, and a composition read at thumbnail scale is a different
/// question from the one Phase 5 is asking.
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
/// Size the *assertions* render at. They compare frames rather than look at
/// them, and the software adapter is slow.
const PROBE: u32 = 128;
/// Frames advanced before capture. The canvas is static under a fixed frame, so
/// two is enough to be past the first-frame build.
const FRAMES: u32 = 2;

/// The `layout` values, with the names `presets/README.md` documents.
const GRAMMARS: [(u32, &str); 4] = [
    (0, "authored-control"),
    (1, "anchor-and-satellites"),
    (2, "diagonal-axis"),
    (3, "size-hierarchy"),
];

/// The five seeds the sheet sweeps. Arbitrary and fixed: what matters
/// is that they are the *same* five in every column, so a row is one
/// seed across four strategies.
const SEEDS: [u32; 5] = [1, 7, 19, 43, 101];

/// Elements per cell.
///
/// **Fourteen, which is the density the Phase 3 gate chose**, not the tier cap.
/// The user judged 8-14 the right canvas from rendered samples, so a sample
/// sheet built at the cap would be asking Phase 5 to pick a grammar at a density
/// already rejected.
const COUNT: usize = 14;

/// One cell's preset. Everything but `layout` and `seed` is held.
fn cell(layout: u32, seed: u32) -> Preset {
    let toml = format!(
        "system = \"shape_collage\"\nname = \"L{layout}_S{seed}\"\n\
         [palette]\nstops = [\n\
         {{ at = 0.0000, color = \"#111111\" }},\n\
         {{ at = 0.1249, color = \"#111111\" }},\n\
         {{ at = 0.1251, color = \"#8a1420\" }},\n\
         {{ at = 0.2499, color = \"#8a1420\" }},\n\
         {{ at = 0.2501, color = \"#96751e\" }},\n\
         {{ at = 0.3749, color = \"#96751e\" }},\n\
         {{ at = 0.3751, color = \"#1e3a8a\" }},\n\
         {{ at = 0.4999, color = \"#1e3a8a\" }},\n\
         {{ at = 0.5001, color = \"#1d5c34\" }},\n\
         {{ at = 0.6249, color = \"#1d5c34\" }},\n\
         {{ at = 0.6251, color = \"#4a4a4a\" }},\n\
         {{ at = 0.7499, color = \"#4a4a4a\" }},\n\
         {{ at = 0.7501, color = \"#5a1f4a\" }},\n\
         {{ at = 0.8749, color = \"#5a1f4a\" }},\n\
         {{ at = 0.8751, color = \"#d9d5c8\" }},\n\
         {{ at = 1.0000, color = \"#d9d5c8\" }},\n\
         ]\n\
         [params]\npaper = \"0.9375\"\nscale = \"1.0\"\ncount = \"{COUNT}\"\n\
         layout = \"{layout}\"\nseed = \"{seed}\"\nsize_hierarchy = \"0.5\"\n\
         angle_bias = \"-22\"\n"
    );
    Preset::from_toml_str(&toml).expect("the sample-sheet cell parses")
}

/// A headless renderer at `size`, or `None` (a logged skip) on a runner with no
/// GPU — macOS has no software Metal fallback (ADR-0016).
fn headless(width: u32, height: u32, software: bool) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width,
        height,
        prefer_software: software,
    }) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// Capture one cell.
fn capture(renderer: &mut Renderer, layout: u32, seed: u32) -> CaptureImage {
    let preset = cell(layout, seed);
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(&name, &AnalysisFrame::default(), FRAMES)
        .expect("capture the sample cell")
}

/// **The seed reaches the screen, and so does the grammar.**
///
/// Two claims, both about the whole path rather than about the generator: a
/// preset naming a seed renders the same canvas every time, a preset naming a
/// different seed renders a different one, and two grammars at one seed differ.
/// The control is asserted the other way — it is a fixed list, so its seed must
/// be inert, and saying so is what keeps the sweep honest about which arm it is
/// testing.
#[test]
fn a_seed_and_a_grammar_both_reach_the_frame() {
    let Some(mut renderer) = headless(PROBE, PROBE, true) else {
        return;
    };

    for (layout, name) in GRAMMARS {
        let a = capture(&mut renderer, layout, SEEDS[0]);
        let again = capture(&mut renderer, layout, SEEDS[0]);
        assert_eq!(
            a.rgba, again.rgba,
            "{name}: the same preset rendered two different canvases — a layout \
             must be a pure function of its seed (the determinism rule)"
        );

        let other = capture(&mut renderer, layout, SEEDS[1]);
        let differing = a
            .rgba
            .chunks_exact(4)
            .zip(other.rgba.chunks_exact(4))
            .filter(|(x, y)| x[..3] != y[..3])
            .count();
        let total = (PROBE * PROBE) as usize;
        eprintln!(
            "{name}: seeds {} and {} differ in {differing} of {total} px",
            SEEDS[0], SEEDS[1]
        );
        if layout == 0 {
            assert_eq!(
                differing, 0,
                "the authored control is a fixed list and must ignore its seed"
            );
        } else {
            assert!(
                differing * 100 > total,
                "{name}: two seeds drew nearly the same canvas ({differing} of \
                 {total} px) — the seed is not reaching the generator"
            );
        }
    }

    // Two grammars at one seed are different pictures, or the sheet Phase 5
    // judges is one strategy rendered four times.
    let base = capture(&mut renderer, 1, SEEDS[0]);
    for (layout, name) in GRAMMARS.into_iter().filter(|(l, _)| *l != 1) {
        let other = capture(&mut renderer, layout, SEEDS[0]);
        let differing = base
            .rgba
            .chunks_exact(4)
            .zip(other.rgba.chunks_exact(4))
            .filter(|(x, y)| x[..3] != y[..3])
            .count();
        assert!(
            differing * 100 > (PROBE * PROBE) as usize,
            "anchor-and-satellites and {name} drew nearly the same canvas"
        );
    }
}

/// **The sample sheet.** Writes one PNG per `(grammar, seed)` cell at 1080p into
/// `$LMV_SAMPLE_DIR` and prints the path. Without that variable it renders
/// nothing and says so — see the module docs on why these are not committed.
///
/// Hardware if there is any: these are looked at, and the software rasterizer
/// would take minutes to draw twenty 1080p frames for no gain in what they show.
#[test]
fn the_sample_sheet_renders() {
    let Some(dir) = std::env::var_os("LMV_SAMPLE_DIR") else {
        eprintln!(
            "skipped: set LMV_SAMPLE_DIR=<dir> to write the Phase 5 sample sheet \
             ({} grammars x {} seeds at {WIDTH}x{HEIGHT})",
            GRAMMARS.len(),
            SEEDS.len(),
        );
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the sample directory");

    let Some(mut renderer) = headless(WIDTH, HEIGHT, false) else {
        return;
    };
    eprintln!(
        "sample sheet: {} cells at {WIDTH}x{HEIGHT} -> {}",
        GRAMMARS.len() * SEEDS.len(),
        dir.display()
    );
    for (layout, name) in GRAMMARS {
        for seed in SEEDS {
            let img = capture(&mut renderer, layout, seed);
            let path = dir.join(format!("{layout}-{name}-seed{seed:03}.png"));
            let buf = image::RgbaImage::from_raw(img.width, img.height, img.rgba)
                .expect("the capture is a well-formed image");
            buf.save(&path)
                .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
            eprintln!("  {}", path.display());
        }
    }
}

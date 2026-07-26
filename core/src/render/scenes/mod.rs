//! Built-in scenes and the thin trait the renderer cycles through.
//!
//! Per ADR-0002 this stays crate-internal and minimal: it is the vocabulary
//! the future preset engine will drive, not a public extension point — no
//! plugin registration, no dynamic dispatch beyond what cycling needs.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Scene update/render run every displayed frame; a panic here
// is a visible crash mid-show.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub mod fragment_field;
pub mod lines;
pub mod particles;
pub mod reaction_diffusion;
pub mod swarm;

use std::cell::RefCell;
use std::rc::Rc;

use crate::dsp::AnalysisFrame;
use crate::preset::SystemKind;
use crate::render::palette::Palette;

/// The `dt` (seconds) the C ABI's legacy `lmv_render` and the headless capture
/// primitives inject when a caller has no real elapsed time to supply — the
/// former fixed scene step, now demoted to a fallback (Plan 0014 Phase 2, ADR-0012).
/// The live frontends measure and inject real `dt` instead, so animation is
/// frame-rate-independent; capture uses this fixed value so a render is a pure
/// function of its inputs.
pub(crate) const FALLBACK_DT: f32 = 1.0 / 60.0;

/// Declarative structural config a scene consumes once at preset load
/// (ADR-0007): **not** expressions — the family / grammar / tiling the sampler
/// or generator builds from. Delivered through the optional
/// `Scene::configure` hook, off the hot path. This is
/// the shared structural-config enum for every scene that has one: the line
/// scenes' curve/L-system/star variants, plus the compute-particle attractor
/// family (Plan 0016) — it lives here rather than in `lines/` so `lines` never has to name a
/// `particles` type (Plan 0031 Phase 6).
#[derive(Debug, Clone)]
pub enum GeneratorConfig {
    /// A parametric curve: which family to sample.
    Curve {
        /// The curve family (Maurer rose, ...).
        family: lines::CurveFamily,
    },
    /// An L-system: a grammar the generator expands and turtle-walks at load,
    /// caching one segment buffer per depth.
    LSystem {
        /// The starting string.
        axiom: String,
        /// Production rules `(predecessor, successor)`.
        rules: Vec<(char, String)>,
        /// Turn angle in degrees for `+`/`-`.
        angle_deg: f32,
        /// Iterations to precompute (`1..=max_depth`), clamped to
        /// [`lines::MAX_LSYSTEM_DEPTH`] at load.
        max_depth: u32,
        /// Reserved seed for future stochastic rules; deterministic today.
        seed: u64,
    },
    /// A Hankin star pattern: an `n`-fold star rosette built at load, with a few
    /// contact-angle variants a beat can switch between.
    Star {
        /// Star order `n` (from the tiling), e.g. 6 or 12.
        order: u32,
        /// Contact angle in degrees; variants are precomputed around it.
        contact_angle_deg: f32,
    },
    /// A GPU compute-particle attractor (Plan 0016): which strange-attractor map
    /// the compute step iterates. Not a line scene — reuses this shared enum so
    /// the family rides the existing `configure` hook (no new trait method).
    Particles {
        /// The attractor family (De Jong, Clifford, Thomas, Lorenz).
        family: particles::AttractorFamily,
    },
}

/// Which construction hit the [`lines::MAX_SEGMENTS`] cap, for the surfaced message.
///
/// An enum rather than a `String` because one of the two producers is **per
/// frame**: an audio-driven `mirror_order` sitting over the cap used to build a
/// fresh `format!("mirror x{order}")` on every single frame for as long as it
/// stayed there — a heap allocation on the hot path (Plan 0031 Phase 4). The
/// formatting now happens only in [`Display`](std::fmt::Display), i.e. only when
/// something actually prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowContext {
    /// An N-fold geometry mirror replicated past the cap — per frame, from the
    /// `mirror_order` param (Plan 0018 Phase 4).
    Mirror(u32),
    /// An L-system depth expanded past the cap — once, at preset load.
    Depth(u32),
}

impl std::fmt::Display for OverflowContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // These two renderings are the user-visible text ADR-0007 requires stay
        // informative; the shell prints them verbatim. Do not reword them
        // without meaning to change what an operator sees.
        match self {
            OverflowContext::Mirror(order) => write!(f, "mirror x{order}"),
            OverflowContext::Depth(depth) => write!(f, "depth {depth}"),
        }
    }
}

/// Reported when building a line scene's geometry hit the fixed [`lines::MAX_SEGMENTS`]
/// cap and truncated. The cap must never be a silent cut (ADR-0007 Risks), so it
/// travels to the frontend two ways: out of
/// `Scene::configure` at preset load, and off
/// `Scene::mirror_overflow` for the
/// per-frame mirror. `None` is the normal case where geometry fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapOverflow {
    /// How many draw segments were dropped at the cap.
    pub dropped: usize,
    /// Where the drop happened, for the surfaced message.
    pub context: OverflowContext,
}

impl std::fmt::Display for CapOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "geometry exceeded the {}-segment cap at {} (dropped {} segment(s)); \
             reduce the structure or its depth",
            lines::MAX_SEGMENTS,
            self.context,
            self.dropped
        )
    }
}

/// One visual. `update` advances state from the analysis frame; `render` draws
/// with the state it has.
///
/// Both built-in systems (fragment field, swarm) are preset-driven and
/// implement the named-parameter surface — `set_time`, `reset_params`,
/// `set_param` — that the preset layer evaluates into per frame (ADR-0002). The
/// trait carries no-op defaults so a future non-parametric scene need not.
pub(crate) trait Scene {
    fn name(&self) -> &'static str;
    fn update(&mut self, frame: &AnalysisFrame);
    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    );

    /// The pixel size of the target **this scene renders into this frame**
    /// (ADR-0030). That is *not* always the surface: the composite chain routes
    /// the scene into the first active post stage's input, which is a fixed
    /// internal grid for the trails and kaleidoscope stages and the surface
    /// otherwise, so the only correct value is the one the chain reports back
    /// (`PostChain::begin`).
    /// A scene that accumulates into an internal offscreen field sizes that field
    /// from here, so it matches its target instead of upscaling from a fixed grid
    /// or supersampling into a smaller offscreen; every other scene ignores it.
    ///
    /// **Called unconditionally every frame**, immediately before
    /// [`render`](Self::render) — it is named for what it carries, not for an
    /// event, and there is no resize event behind it. So ADR-0030 condition 2
    /// binds every implementor: **compare against what you already built and do
    /// nothing when unchanged**, and never allocate or build GPU resources here.
    /// The attractor records the requested grid and lets the next `render` notice
    /// the difference.
    ///
    /// Default no-op, in the same spirit as [`advance`](Self::advance): the
    /// renderer already holds the size in `draw_frame`, and `Scene` is a `dyn`
    /// trait, so this is the only channel that reaches a scene with it (Plan 0027
    /// Phase 2, the third and first hot-path widening — ADR-0030).
    fn set_target_size(&mut self, _width: u32, _height: u32) {}

    /// Advance simulation state by `dt` real seconds (Plan 0014 Phase 2). The
    /// renderer injects the elapsed time each frame; a feedback scene steps its
    /// fixed-timestep accumulator here and a CPU-integrated scene (the swarm)
    /// scales its motion by `dt`, so both look identical over wall-clock time on
    /// any refresh rate. Stateless, purely `time`-driven scenes ignore it.
    fn advance(&mut self, _dt: f32) {}

    /// Set the shared scene clock (seconds). The renderer owns the single clock
    /// so an expression's `time` and the system's animation never diverge.
    fn set_time(&mut self, _time: f32) {}
    /// Reset every named parameter to its default (called each frame before the
    /// active preset's bindings are applied, so unbound params don't leak).
    fn reset_params(&mut self) {}
    /// Apply one named parameter; unknown names are ignored.
    fn set_param(&mut self, _name: &str, _value: f32) {}

    /// Consume a preset's declarative structural config (ADR-0007). Invoked
    /// **once at preset load, off the hot path** — a generator builds and caches
    /// its geometry here; a parametric scene records its family. Default no-op,
    /// so non-line scenes (fragment field, swarm) never implement it. The one
    /// optional widening of this trait ADR-0007 sanctions — keep it to this.
    ///
    /// Returns [`Some`](lines::CapOverflow) when building the geometry hit the
    /// segment cap and truncated, so the frontend can surface it — the cap is
    /// never a silent cut (ADR-0007 Risks). `None` means it fit (the norm).
    fn configure(&mut self, _cfg: &lines::GeneratorConfig) -> Option<lines::CapOverflow> {
        None
    }

    /// Consume a preset's baked color [`Palette`] (ADR-0021). Invoked **once at
    /// preset load, off the hot path** — a shader-colored scene stores the baked
    /// LUT and uploads it to its 256×1 texture (or samples it on the CPU) on the
    /// next frame; a non-colored scene (the line scenes) ignores it. Default
    /// no-op. The second and last thin off-hot-path widening of this trait after
    /// ADR-0007's [`configure`](Scene::configure).
    fn set_palette(&mut self, _palette: &Palette) {}

    /// The per-frame geometry-mirror cap overflow (Plan 0018 Phase 4), if this
    /// frame's N-fold replication exceeded the segment cap and truncated. Reuses
    /// the ADR-0007 [`CapOverflow`](lines::CapOverflow) so the frontend surfaces
    /// it — the cap is never a silent cut. Default `None`: only the line scenes
    /// mirror, and only when `mirror_order` pushes past the cap.
    fn mirror_overflow(&self) -> Option<&lines::CapOverflow> {
        None
    }
}

/// The registry: every built-in scene, **keyed by the [`SystemKind`] it drives**,
/// in [`SystemKind::ALL`] order. All scenes are created up front so switching
/// mid-show is a lookup, never a hitch.
///
/// The keying is the point: the renderer addresses a scene by the kind its preset
/// names, so a scene can no longer silently end up in the wrong slot. Nothing
/// here is positional — reordering [`SystemKind::ALL`] reorders construction and
/// nothing else.
pub(crate) fn create_all(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> Vec<(SystemKind, Box<dyn Scene>)> {
    // One shared line renderer for every line scene (ADR-0007: "one line
    // renderer"). A single instanced-quad pipeline + segment buffer, borrowed by
    // whichever line scene is active — only one draws per frame. (Two separate
    // line pipelines with byte-identical vertex layouts also mis-render on the
    // DX12 WARP software adapter the capture tests use; one renderer avoids it.)
    let line_renderer = Rc::new(RefCell::new(lines::LineRenderer::new(
        device,
        surface_format,
        lines::MAX_SEGMENTS,
        "lines",
    )));
    SystemKind::ALL
        .iter()
        .map(|&kind| (kind, create(kind, device, surface_format, &line_renderer)))
        .collect()
}

/// Whether two systems' scenes share mutable GPU state, so **one frame must not
/// render both**.
///
/// Two facts make this true, and only one of them is obvious. The roster is keyed
/// by kind, so the same kind is literally the same `Box<dyn Scene>`. Less
/// obviously, the three **line** scenes deliberately share one `LineRenderer` —
/// "borrowed by whichever line scene is active, only one draws per frame" (see
/// [`create_all`]) — so two *different* line kinds are just as unrenderable in one
/// frame as one kind twice.
///
/// Plan 0023's dual-live dissolve is the first caller: it composites two presets
/// in a single frame, which is exactly what this forbids. A pair that shares
/// resources falls back to the frozen snapshot.
pub(crate) fn shares_resources(a: SystemKind, b: SystemKind) -> bool {
    a == b || (draws_through_shared_line_renderer(a) && draws_through_shared_line_renderer(b))
}

/// Whether a system draws through the shared `LineRenderer`. **Exhaustive** with
/// no wildcard arm, like [`create`] itself: a new scene fails to compile here
/// until someone says which side of the sharing it is on.
fn draws_through_shared_line_renderer(kind: SystemKind) -> bool {
    match kind {
        SystemKind::ParametricCurve
        | SystemKind::LSystem
        | SystemKind::StarPattern
        | SystemKind::Spectrum => true,
        SystemKind::FragmentField
        | SystemKind::Swarm
        | SystemKind::ReactionDiffusion
        | SystemKind::Attractor => false,
    }
}

/// Build the scene a [`SystemKind`] drives.
///
/// An **exhaustive** `match` with no wildcard arm — the same guard the golden
/// drift fixtures use: adding a variant fails to compile here until its scene is
/// constructed, so a new system cannot ship unbuilt or wired to the wrong scene.
fn create(
    kind: SystemKind,
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    line_renderer: &Rc<RefCell<lines::LineRenderer>>,
) -> Box<dyn Scene> {
    match kind {
        SystemKind::FragmentField => Box::new(fragment_field::FragmentFieldScene::new(
            device,
            surface_format,
        )),
        SystemKind::Swarm => Box::new(swarm::SwarmScene::new(device, surface_format)),
        SystemKind::ParametricCurve => {
            Box::new(lines::ParametricCurveScene::new(line_renderer.clone()))
        }
        SystemKind::LSystem => Box::new(lines::LSystemScene::new(line_renderer.clone())),
        SystemKind::StarPattern => Box::new(lines::StarPatternScene::new(line_renderer.clone())),
        SystemKind::ReactionDiffusion => Box::new(reaction_diffusion::ReactionDiffusionScene::new(
            device,
            surface_format,
        )),
        SystemKind::Attractor => Box::new(particles::AttractorScene::new(device, surface_format)),
        SystemKind::Spectrum => Box::new(lines::SpectrumScene::new(line_renderer.clone())),
    }
}

/// Tiny deterministic RNG (splitmix64) so visual randomness is explicitly
/// seeded (NFR 6) without pulling a rand crate.
pub(crate) struct SeededRng(u64);

impl SeededRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub(crate) fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [lo, hi).
    pub(crate) fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

#[cfg(test)]
mod tests {
    //! The scene-keying contract (Plan 0030 Phase 3). Test asserts panic freely;
    //! this is not the render path.
    #![allow(clippy::panic)]

    use super::create_all;
    use crate::preset::SystemKind;
    use crate::render::context::{RenderContext, RenderError};

    /// The scene each system is *supposed* to drive, written independently of the
    /// factory so the two can disagree. This is the mapping the old magic-index
    /// `system_slot` lookup could never assert: it named a position, and nothing
    /// checked the position held the right scene.
    fn expected_scene_name(system: SystemKind) -> &'static str {
        match system {
            SystemKind::FragmentField => "fragment field",
            SystemKind::Swarm => "swarm",
            SystemKind::ParametricCurve => "parametric curve",
            SystemKind::LSystem => "l-system",
            SystemKind::StarPattern => "star pattern",
            SystemKind::ReactionDiffusion => "reaction diffusion",
            SystemKind::Attractor => "attractor",
            SystemKind::Spectrum => "spectrum",
        }
    }

    /// Every `SystemKind::ALL` entry builds the scene that kind is supposed to
    /// drive, and the roster covers exactly the roster — so transposing two
    /// factory arms, which used to silently point every preset of one system at
    /// another's scene, now fails here.
    ///
    /// Needs a GPU adapter to build the scenes, so it skips on runners without
    /// one (ADR-0016).
    #[test]
    fn every_kind_builds_the_scene_it_drives() {
        let ctx = match RenderContext::new_headless(64, 64, true) {
            Ok(ctx) => ctx,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return;
            }
            Err(e) => panic!("headless context build failed: {e}"),
        };

        let scenes = create_all(&ctx.device, ctx.surface_format());

        let kinds: Vec<SystemKind> = scenes.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(
            kinds,
            SystemKind::ALL.to_vec(),
            "the roster is exactly SystemKind::ALL, in its order"
        );

        for (kind, scene) in &scenes {
            assert_eq!(
                scene.name(),
                expected_scene_name(*kind),
                "system {} must drive its own scene",
                kind.as_str()
            );
        }
    }

    /// The **freeze veto** a dual-live dissolve rests on (Plan 0023 Phase 4): a
    /// pair of systems that would have to render one mutable object twice in a
    /// frame must report shared resources, so the governor never upgrades it.
    ///
    /// GPU-free — this is the mapping, not the rendering. It closes the half the
    /// governor's own test has to assume: `dual_live_eligible` is asserted to
    /// refuse a shared pair, and here is what makes a pair shared.
    #[test]
    fn a_pair_that_cannot_render_twice_reports_shared_resources() {
        // Same kind is the same `Box<dyn Scene>` — the same-scene case that must
        // always freeze, whatever the frame budget says.
        for kind in SystemKind::ALL {
            assert!(
                super::shares_resources(kind, kind),
                "{} against itself is one scene object",
                kind.as_str()
            );
        }

        // Two *different* line systems are just as unrenderable together: they
        // borrow one shared `LineRenderer` (see `create_all`).
        let lines = [
            SystemKind::ParametricCurve,
            SystemKind::LSystem,
            SystemKind::StarPattern,
            SystemKind::Spectrum,
        ];
        for a in lines {
            for b in lines {
                assert!(
                    super::shares_resources(a, b),
                    "{} and {} share the line renderer",
                    a.as_str(),
                    b.as_str()
                );
            }
        }

        // Everything else holds independent state, so a dissolve between them may
        // run both sides live.
        let independent = [
            SystemKind::FragmentField,
            SystemKind::Swarm,
            SystemKind::ReactionDiffusion,
            SystemKind::Attractor,
        ];
        for (i, a) in independent.iter().enumerate() {
            for b in independent.iter().skip(i + 1).chain(lines.iter()) {
                assert!(
                    !super::shares_resources(*a, *b),
                    "{} and {} hold independent GPU state",
                    a.as_str(),
                    b.as_str()
                );
            }
        }
    }
}

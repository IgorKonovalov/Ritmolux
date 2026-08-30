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

pub(crate) mod common;
pub mod emitter;
pub mod fragment_field;
pub mod lines;
/// The shared mark-silhouette vocabulary the two particle scenes draw through
/// (ADR-0084). Crate-internal: it is arithmetic and a roster, not a scene.
pub(crate) mod marks;
pub mod particles;
pub mod reaction_diffusion;
pub mod shape_collage;
pub mod shape_field;
pub mod swarm;
pub mod warp_mesh;

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

/// One integrated animation phase — the only way a bindable rate advances
/// anything in this engine (ADR-0135, finishing the rule ADR-0132 stated).
///
/// **A rate multiplier has to be integrated to be a rate at all.** A phase
/// computed as `time * rate` lets a rate bound to audio retroactively rescale
/// *all* elapsed time on every frame: at t = 100 s a swing from `1.0` to `1.5`
/// moves the phase by fifty seconds in a single frame — the figure snaps to a
/// new position rather than accelerating toward it, on a lane whose whole
/// method is binding parameters to audio. Integrated, the same swing bends the
/// motion.
///
/// [`step`](Self::step) is the **only** mutator, and there is deliberately no
/// `Add`/`AddAssign`/`Deref`/`DerefMut` impl. The constraint is the value: with
/// one, a scene could write `phase + self.rate * self.time` and compile, which
/// is exactly the door this type exists to close.
///
/// # No constant scale is folded into the accumulation
///
/// A scene carrying its own fixed rate — the attractor's `SPIN_RATE` — applies
/// it where the phase is **read**, never inside the sum. The accumulator is then
/// `Σ (rate · dt)` with `rate` at its `1.0` default, i.e. `Σ dt` term for term:
/// bit-for-bit the same summation the renderer performs for its own clock, so
/// the integrated form reproduces the multiply it replaced *exactly* and no
/// golden baseline moves. Folding a `0.18` in would sum `0.18 · dt` instead and
/// drift in the last bits of every capture.
///
/// # It steps in `update`, never in `advance`
///
/// The per-frame order is `set_time` → `advance` → `reset_params` → `set_param`
/// → `update` (`core/src/render/mod.rs`), so a scene stores the `dt` that
/// [`Scene::advance`] hands it and integrates in [`Scene::update`], where *this*
/// frame's rate has landed. Integrating in `advance` would use the previous
/// frame's.
///
/// The type is arithmetic with no device in it, which is what keeps every rate
/// in the engine testable on the CPU without rendering anything.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(crate) struct Phase(f32);

impl Phase {
    /// One frame's integration, at *this* frame's rate.
    pub(crate) fn step(&mut self, rate: f32, dt: f32) {
        self.0 += rate * dt;
    }

    /// The accumulated phase, in rate-scaled seconds. A scene with its own
    /// constant scale applies it here, on the read.
    pub(crate) fn get(self) -> f32 {
        self.0
    }
}

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
    /// contact-angle variants a beat can switch between — and, since ADR-0079,
    /// an optional ring ornament drawn inside it.
    Star {
        /// Star order `n` (from the tiling), e.g. 6 or 12. **`0` means no
        /// interlace at all** (`tiling = "none"`), which the loader accepts only
        /// alongside a non-empty `rings` roster — the ornament drawn alone.
        order: u32,
        /// Contact angle in degrees; variants are precomputed around it.
        contact_angle_deg: f32,
        /// The `[generator] rings` roster (ADR-0079): concentric rings of
        /// repeated motifs filling the interior the rosette leaves hollow. Empty
        /// — the default, and what an absent `rings` key means — is exactly the
        /// pre-Plan-0065 scene.
        rings: Vec<lines::star::RingSpec>,
    },
    /// A GPU compute-particle attractor (Plan 0016): which strange-attractor map
    /// the compute step iterates. Not a line scene — reuses this shared enum so
    /// the family rides the existing `configure` hook (no new trait method).
    Particles {
        /// The attractor family (De Jong, Clifford, Thomas, Lorenz, or one of
        /// the IFS figures).
        family: particles::AttractorFamily,
        /// The figure the bindable `morph` param travels **towards** (ADR-0075),
        /// from `[particles] morph_to`. `None` — the default — pins the figure,
        /// so `morph` is inert.
        ///
        /// IFS-only, and validated as such at load: `morph_to` on a map family
        /// is a load error rather than a silent no-op, because the author asked
        /// for something the engine cannot do.
        morph_to: Option<particles::ifs::IfsFigure>,
        /// Fraction of the tier's particle budget actually drawn (ADR-0069),
        /// validated at load into
        /// [`MIN_PARTICLE_DENSITY`](particles::MIN_PARTICLE_DENSITY)`..=1.0`.
        /// Structural, not bindable: an eased integer count would re-decide the
        /// picture every frame. `1.0` is the whole budget and the default.
        density: f32,
        /// The **tuple path** the bindable `morph` param walks along on a map
        /// family (ADR-0093), as `(from, to)` roster indices from
        /// `[particles] tuple_from` / `tuple_to`. `None` — the default — means
        /// there is no path and `morph` is inert, exactly as `morph_to` does for
        /// the IFS.
        ///
        /// **Both ends are structural on purpose.** The walk's framing is
        /// measured across it at load, which is thousands of map iterations; a
        /// path whose near end came from the per-frame `tuple` param would have
        /// to re-measure inside the frame loop every time that param moved.
        ///
        /// Map-family-only, and validated as such at load — the IFS reaches its
        /// own figure-to-figure travel through `morph_to` instead, and a tuple
        /// path on an IFS is a load error rather than a silent no-op.
        tuple_path: Option<(u32, u32)>,
    },
    /// The spectrum readout's `[spectrum]` table (Plan 0034 / ADR-0036): how many
    /// elements the frequency axis is divided into, how they are laid out, and how
    /// fast each one follows its band. All three are structure rather than
    /// expression — they are fixed for as long as the preset is loaded — so they
    /// ride the existing `configure` hook like every other declarative config.
    Spectrum {
        /// Element count, validated at load into
        /// `2..=`[`SPECTRUM_BINS`](crate::dsp::SPECTRUM_BINS).
        elements: usize,
        /// Which figure the elements form.
        layout: lines::SpectrumLayout,
        /// Per-element temporal easing in **seconds**, applied on the injected
        /// real `dt` — the same [`Easing`](crate::preset::Easing) the `[smoothing]`
        /// table uses, deliberately reused rather than a second vocabulary
        /// (ADR-0035).
        easing: crate::preset::Easing,
    },
    /// The warp mesh's `[mesh]` table (Plan 0100 / ADR-0113): the grid, in
    /// cells, that the per-vertex program is evaluated over.
    ///
    /// Structural for `[curve] family`'s reason — the vertex and index buffers
    /// are built from it, so an eased grid would rebuild them mid-frame — and
    /// **clamped to the tier at both consumers** rather than at load, since the
    /// loader does not know which tier will render the preset
    /// ([`warp_mesh::clamp_grid`]).
    WarpMesh {
        /// Requested cells, `(x, y)`, validated at load into
        /// [`MIN_MESH`](warp_mesh::MIN_MESH)`..=`[`MAX_MESH`](warp_mesh::MAX_MESH).
        mesh: (u32, u32),
        /// The compiled EEL2 programs a **converted** preset carries, from a
        /// `[milk]` table (Plan 0100 Phase 2 / ADR-0113). `None` — a
        /// hand-authored `warp_mesh` preset — drives the mesh from the ordinary
        /// `[params]` and `[per_vertex]` bindings instead, and executes no VM at
        /// all.
        ///
        /// Boxed because it is much the largest thing this enum carries and every
        /// other variant would pay for it by value.
        milk: Option<Box<crate::milk::MilkBundle>>,
        /// The salt the bundle's `rand()` draws under (ADR-0051).
        ///
        /// **The preset's `pinned_salt`, always** — its declared numeric seed, or
        /// `0` where it declared `seed = "random"`. So a bundle is a pure
        /// function of its inputs in the live app as well as in the harness,
        /// which is stronger than ADR-0051 requires and costs nothing: per-run
        /// variety is opt-in through `seed = "random"`, and no *converted* preset
        /// declares one. A hand-written bundle that does gets the pinned
        /// behaviour, and that is stated rather than discovered.
        salt: u32,
    },
}

impl GeneratorConfig {
    /// How many elements a per-element binding should be evaluated for under this
    /// config, or `0` when the system has no per-element surface (Plan 0034 Phase
    /// 4). Read once at preset load to size the render layer's scratch.
    ///
    /// The count lives here rather than on `Scene` because it is **preset data**:
    /// it comes off the `[spectrum]` table, not out of the scene's state, and the
    /// renderer already holds the preset.
    pub fn element_count(&self) -> usize {
        match self {
            GeneratorConfig::Spectrum { elements, .. } => *elements,
            GeneratorConfig::Curve { .. }
            | GeneratorConfig::LSystem { .. }
            | GeneratorConfig::Star { .. }
            | GeneratorConfig::Particles { .. }
            | GeneratorConfig::WarpMesh { .. } => 0,
        }
    }
}

/// Which construction hit the segment cap, for the surfaced message.
///
/// An enum rather than a `String` because one of the two producers is **per
/// frame**: with a `String`, an audio-driven `mirror_order` sitting over the cap
/// builds a fresh `format!("mirror x{order}")` on every single frame for as long
/// as it stayed there — a heap allocation on the hot path (Plan 0031 Phase 4).
/// The formatting now happens only in [`Display`](std::fmt::Display), i.e. only
/// when something actually prints it.
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

/// Reported when building a line scene's geometry hit the segment cap and
/// truncated. The cap must never be a silent cut (ADR-0007 Risks), so it travels
/// to the frontend two ways: out of `Scene::configure` at preset load, and off
/// `Scene::mirror_overflow` for the per-frame mirror. `None` is the normal case
/// where geometry fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapOverflow {
    /// How many draw segments were dropped at the cap.
    pub dropped: usize,
    /// Where the drop happened, for the surfaced message.
    pub context: OverflowContext,
    /// **The cap that bit**, carried rather than read from a constant: it is a
    /// tier value now (Plan 0044), so the same preset overflows at 20 000
    /// segments on the floor and not at all at 60 000 on rich. A message naming a
    /// cap the run was not using would be worse than no message.
    pub cap: usize,
}

impl std::fmt::Display for CapOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "geometry exceeded the {}-segment cap at {} (dropped {} segment(s)); \
             reduce the structure or its depth",
            self.cap, self.context, self.dropped
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

    /// How much of this scene's coverage the **backdrop** resolves against, for
    /// the frame it is about to render (ADR-0085). `1.0` is coverage-as-occlusion
    /// — what every frame did before `occlude` existed — and `0.0` is light that
    /// adds without covering.
    ///
    /// **Called unconditionally every frame**, immediately before
    /// [`render`](Self::render), in the same spirit as
    /// [`set_target_size`](Self::set_target_size). The renderer hands a literal
    /// `1.0` whenever a post stage is active, because then the scene draws into a
    /// scratch offscreen with no backdrop under it and the chain's last stage owns
    /// the seam instead — a scene must never apply this twice.
    ///
    /// Only a scene that **presents premultiplied over the backdrop** (ADR-0026 —
    /// the reaction-diffusion, attractor and fragment-field presents) has anything
    /// to do here. The additive families draw through
    /// [`gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`](crate::render::gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE),
    /// whose colour destination factor is `One`: with no stage active their light
    /// already adds to the backdrop rather than replacing it, so there is no
    /// occlusion at that seam for this to scale. Default no-op.
    fn set_occlude(&mut self, _occlude: f32) {}

    /// The scene's feedback field, for a probe that needs the value **before**
    /// this scene's present pass. `None` for every scene without one, which is
    /// every scene but the warp mesh.
    ///
    /// **`#[cfg(test)]`, and that gate is the whole justification.** ADR-0002
    /// keeps this trait thin and a real widening of it is ADR-worthy; this method
    /// does not exist in a shipped build, so the extension seam is unchanged. It
    /// exists because Plan 0111 Phase 2's bisect requires its five seams to be
    /// read from **one run** — same signal, same hop, same size, same adapter —
    /// and a `Box<dyn Scene>` cannot otherwise be asked for the one quantity that
    /// sits upstream of everything the bisect covers. Measuring seam A on a
    /// separately-driven scene would satisfy the arithmetic and quietly break
    /// that requirement.
    #[cfg(test)]
    fn feedback_field(&self) -> Option<&wgpu::Texture> {
        None
    }

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

    /// Apply one named parameter as a **per-element series** (Plan 0034 Phase 4,
    /// ADR-0036): `values` holds one evaluation of the binding per element, in
    /// element order. Reached only for a binding whose expression names `index`.
    ///
    /// **This is the whole channel, and it is deliberately this narrow.** It
    /// carries `(name, &[f32])` in one direction and returns nothing. A scene
    /// cannot ask the preset layer for anything, cannot see the expression, and
    /// cannot learn which preset is loaded — so this is `set_param` with a slice,
    /// not an inversion in which scenes read presets. The slice borrows the
    /// renderer's scratch, which is sized at preset load, so nothing here
    /// allocates.
    ///
    /// The default takes the **first** value and routes it through
    /// [`set_param`](Self::set_param) — exactly the `index = 0` reading a binding
    /// gets outside a per-element evaluation. So a scene with no per-element
    /// surface degrades a series to a scalar instead of dropping it, and a scene
    /// that never opts in behaves byte-for-byte as before. Only the spectrum
    /// readout overrides this.
    fn set_param_series(&mut self, name: &str, values: &[f32]) {
        if let Some(&first) = values.first() {
            self.set_param(name, first);
        }
    }

    /// Apply one named parameter as a **per-vertex series** (Plan 0100 Phase 1,
    /// ADR-0113): `values` holds one evaluation of the binding per mesh vertex,
    /// in row-major order from the top-left, `(meshx + 1) * (meshy + 1) ` long.
    ///
    /// The per-element channel one axis up, and deliberately just as narrow: it
    /// carries `(name, &[f32])` in one direction and returns nothing. Reached
    /// only for a binding in a `[per_vertex]` table, so a scene that never opts
    /// in is never called.
    ///
    /// Unlike [`set_param_series`](Self::set_param_series) the default does
    /// **nothing** rather than degrading to the first value. A per-vertex series
    /// varies over space and its first element is the top-left corner, which is
    /// not a sensible whole-scene reading of anything; the loader already warns
    /// that a `[per_vertex]` table on another system is inert.
    ///
    /// The slice borrows the renderer's scratch, sized at preset load from the
    /// same [`clamp_grid`](warp_mesh::clamp_grid) the scene uses, so nothing here
    /// allocates.
    fn set_per_vertex(&mut self, _name: &str, _values: &[f32]) {}

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

    /// Consume a preset's `[feedback]` structural table (ADR-0048). Invoked
    /// **once at preset load, off the hot path**, like
    /// [`configure`](Scene::configure) and [`set_palette`](Scene::set_palette),
    /// and load-time for `configure`'s reason: a warp kind is a shader path, not
    /// a scalar. Default no-op — the third and last thin off-hot-path widening of
    /// this trait.
    ///
    /// # One vocabulary, two buffers
    ///
    /// **This is the routing contract, and it is worth stating plainly because it
    /// will surprise someone.** The `fb_*` params and this table are consumed by
    /// *two* sinks: the engine [`Trails`](crate::render::trails::Trails) stage,
    /// which transforms the accumulation every scene composites through, and the
    /// attractor scene's own internal trail field, which is what reaches here.
    /// A preset may have **both** active at once — an attractor with `trails` on —
    /// and then a single `fb_rotate` turns *both* accumulations, each about its
    /// own buffer. Neither transforms the other's, and neither transforms the
    /// present deposit: the transform applies to the past.
    ///
    /// That is a deliberate design (ADR-0048's Alternative D was to give the
    /// engine stage the vocabulary and leave the attractor out), and the reason it
    /// is safe is that the two answer the same param names with the same
    /// arithmetic — [`feedback::Transform`](crate::render::feedback::Transform)
    /// and one shared WGSL snippet, not two implementations that must agree.
    fn set_feedback(&mut self, _cfg: crate::render::feedback::FeedbackConfig) {}

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
/// names, so a scene cannot silently end up in the wrong slot. Nothing here is
/// positional — reordering [`SystemKind::ALL`] reorders construction and nothing
/// else.
pub(crate) fn create_all(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    tier: &crate::render::TierConfig,
) -> Vec<(SystemKind, Box<dyn Scene>)> {
    // One shared line renderer for every line scene (ADR-0007: "one line
    // renderer"). A single instanced-quad pipeline + segment buffer, borrowed by
    // whichever line scene is active — only one draws per frame. (Two separate
    // line pipelines with byte-identical vertex layouts also mis-render on the
    // DX12 WARP software adapter the capture tests use; one renderer avoids it.)
    // `new_with_arcs`, not `new`: `star_pattern`'s circular motifs are one arc
    // instance each (ADR-0098), and the arc buffer holds
    // `max_segments` because the two kinds share **one** budget — everything
    // that passes `build_rings`'s cap check must reach the GPU, or a cap would
    // be silently cutting geometry, which ADR-0007 forbids.
    // `new_split_with_arcs`: any of the four line systems may ask for the
    // opacity-preserving seam through `stroke_blend` (ADR-0138), and the
    // pipelines are built here rather than when a preset first selects one —
    // building a GPU resource mid-run changes what a later pass resolves to on
    // the DX12 software adapter.
    let line_renderer = Rc::new(RefCell::new(lines::LineRenderer::new_split_with_arcs(
        device,
        surface_format,
        tier.max_segments,
        tier.max_segments,
        "lines",
    )));
    SystemKind::ALL
        .iter()
        .map(|&kind| {
            (
                kind,
                create(
                    kind,
                    device,
                    surface_format,
                    &mut || line_renderer.clone(),
                    tier,
                ),
            )
        })
        .collect()
}

/// A scene constructed **for one preset's `[layer]`** (ADR-0090 point 4, Plan
/// 0076 Phase 2), never taken from the roster — which is what makes same-system
/// pairs legal and keeps two dissolving sides' layers from sharing anything.
/// The stateful families duplicate their GPU state by construction: their
/// constructors are already self-contained (a second reaction-diffusion
/// ping-pong field, a second particle buffer), so this is the same exhaustive
/// [`create`] the roster uses, differing only in where a line scene gets its
/// renderer.
///
/// # The `LineRenderer` answer, recorded (the Phase 2 discovery duty)
///
/// **A layer line scene gets its own `LineRenderer`; the shared one is not
/// shareable between two live line draws in one frame.** `LineRenderer::draw`
/// uploads its instance and uniform buffers through `Queue::write_buffer`, and
/// queued writes are applied before the submission's passes execute — so two
/// draws through one renderer in one frame would both rasterize the *second*
/// draw's segments under the second draw's uniforms. Making it shareable would
/// need a partitioned instance buffer and per-draw uniform slots — a redesign
/// of the idiom, not constructor plumbing — so duplication is the answer, at
/// one pipeline plus one `max_segments` instance buffer per layered line
/// preset.
///
/// The duplicate is built **only when the layer is a line system**: a
/// fragment/swarm/particle layer pays no line pipeline, and WARP's documented
/// sensitivity to coexisting identical pipeline layouts (ADR-0058 / Plan 0053)
/// is only ever exercised by a preset that actually declares a line layer.
pub(crate) fn create_layer_scene(
    kind: SystemKind,
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    tier: &crate::render::TierConfig,
) -> Box<dyn Scene> {
    create(
        kind,
        device,
        surface_format,
        &mut || {
            // Arcs too — a `[layer]` may be a `star_pattern`, and a layer that
            // could not draw them would render a mandala with its circles
            // missing rather than fail.
            Rc::new(RefCell::new(lines::LineRenderer::new_split_with_arcs(
                device,
                surface_format,
                tier.max_segments,
                tier.max_segments,
                "layer-lines",
            )))
        },
        tier,
    )
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
///
/// **This is a statement about the roster's instances only.** A `[layer]`
/// scene ([`create_layer_scene`], Plan 0076 Phase 2) is constructed per preset
/// and shares nothing with the roster or with another preset's layer by
/// construction — so a preset's own main-plus-layer pair never consults this,
/// whatever the two systems are.
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
        | SystemKind::Attractor
        | SystemKind::Emitter
        | SystemKind::ShapeField
        | SystemKind::WarpMesh
        | SystemKind::ShapeCollage => false,
    }
}

/// Build the scene a [`SystemKind`] drives.
///
/// An **exhaustive** `match` with no wildcard arm — the same guard the golden
/// drift fixtures use: adding a variant fails to compile here until its scene is
/// constructed, so a new system cannot ship unbuilt or wired to the wrong scene.
///
/// `line_renderer` is a **source**, called only by the line arms: the roster
/// hands out clones of its one shared renderer, a layer construction builds a
/// fresh one on demand ([`create_layer_scene`]) — and a non-line kind builds
/// none at all.
fn create(
    kind: SystemKind,
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    line_renderer: &mut dyn FnMut() -> Rc<RefCell<lines::LineRenderer>>,
    tier: &crate::render::TierConfig,
) -> Box<dyn Scene> {
    match kind {
        SystemKind::FragmentField => Box::new(fragment_field::FragmentFieldScene::new(
            device,
            surface_format,
        )),
        SystemKind::Swarm => Box::new(swarm::SwarmScene::new(
            device,
            surface_format,
            tier.swarm_particles,
        )),
        SystemKind::ParametricCurve => Box::new(lines::ParametricCurveScene::new(
            line_renderer(),
            tier.max_segments,
        )),
        SystemKind::LSystem => {
            Box::new(lines::LSystemScene::new(line_renderer(), tier.max_segments))
        }
        SystemKind::StarPattern => Box::new(lines::StarPatternScene::new(
            line_renderer(),
            tier.max_segments,
        )),
        SystemKind::ReactionDiffusion => Box::new(reaction_diffusion::ReactionDiffusionScene::new(
            device,
            surface_format,
        )),
        SystemKind::Attractor => Box::new(particles::AttractorScene::new(
            device,
            surface_format,
            tier.attractor_particles,
            tier.attractor_trail_cap,
        )),
        SystemKind::Spectrum => Box::new(lines::SpectrumScene::new(
            line_renderer(),
            tier.max_segments,
        )),
        SystemKind::Emitter => Box::new(emitter::EmitterScene::new(
            device,
            surface_format,
            tier.emitter_objects,
        )),
        SystemKind::ShapeField => {
            Box::new(shape_field::ShapeFieldScene::new(device, surface_format))
        }
        SystemKind::WarpMesh => Box::new(warp_mesh::WarpMeshScene::new(
            device,
            surface_format,
            tier.mesh_grid,
            tier.max_segments,
        )),
        SystemKind::ShapeCollage => Box::new(shape_collage::ShapeCollageScene::new(
            device,
            surface_format,
            tier.collage_elements,
        )),
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
            SystemKind::ShapeField => "shape field",
            SystemKind::Swarm => "swarm",
            SystemKind::ParametricCurve => "parametric curve",
            SystemKind::LSystem => "l-system",
            SystemKind::StarPattern => "star pattern",
            SystemKind::ReactionDiffusion => "reaction diffusion",
            SystemKind::Attractor => "attractor",
            SystemKind::Spectrum => "spectrum",
            SystemKind::Emitter => "emitter",
            SystemKind::WarpMesh => "warp mesh",
            SystemKind::ShapeCollage => "shape collage",
        }
    }

    /// Every `SystemKind::ALL` entry builds the scene that kind is supposed to
    /// drive, and the roster covers exactly the roster — so transposing two
    /// factory arms — which silently points every preset of one system at
    /// another's scene — fails here.
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

        let scenes = create_all(
            &ctx.device,
            ctx.surface_format(),
            &crate::render::TierConfig::FLOOR,
        );

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
            SystemKind::Emitter,
            SystemKind::ShapeField,
            SystemKind::WarpMesh,
            SystemKind::ShapeCollage,
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

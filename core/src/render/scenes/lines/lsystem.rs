//! L-system scene: expensive to build, cheap to animate (ADR-0007 generator
//! build model). At preset load (`configure`, off the hot path) the grammar is
//! expanded and turtle-walked into one cached segment buffer *per depth*
//! `1..=max_depth`. Per frame the scene only picks the visible depth and applies
//! a rotation / scale / colour / draw-on transform into the draw buffer — no
//! expansion, no allocation.
//!
//! Beat accents advance `visible_depth` (grow one iteration); continuous motion
//! drives `rotation`, `hue`, `draw_progress`, etc.
//!
//! ## The colour axis: **generation depth** (ADR-0059)
//!
//! This scene honours `[palette]` / `[palette_b]` / `palette_mix` / `hue_spread`
//! / `saturation`, sampled on the CPU exactly as [`spectrum`](super::spectrum)
//! does. Each line scene walks `hue_spread` along the axis its own generator
//! makes meaningful, and for an L-system that axis is **generation depth**: the
//! branch-nesting level the turtle drew a segment at, `0` on the trunk and one
//! more for every open `[`. Colouring by it makes an older branch read as older,
//! which is what the whole subject of a rewriting system is.
//!
//! **The ramp is normalized over the figure's own deepest generation, not over
//! `visible_depth`.** ADR-0059 wrote the latter; it is wrong in both directions
//! and the code follows the measurement instead. A grammar can open more than one
//! branch per rewrite — `lsystem_fern`'s `X -> F+[[X]-X]-F[-FX]+X` opens two, so
//! its deepest generation runs 1, 3, 5, 7, 9, **11** over `visible_depth`
//! 1 to 6, and dividing by 6 would leave five sixths of the figure clamped at the
//! palette's far end — while a grammar with no brackets at all
//! (`lsystem_arrowhead`, deepest generation **0** at every one of its seven
//! depths) has no range for the divisor to describe. Normalizing over the built
//! figure's own maximum makes `hue_spread = 1` span the palette exactly once on
//! any grammar, and it is a **load-time** quantity, so an eased `visible_depth`
//! cannot sweep the divisor through fractional values mid-fall.
//!
//! A bracket-free grammar therefore has exactly one generation and colours flat —
//! that is a property of such a figure (every segment of a Sierpinski arrowhead
//! genuinely sits at the same recursion level), not a gap. Such a preset still
//! reaches the palette; what it cannot reach is a ramp across a figure that has
//! no depth to ramp along.
//!
//! `hue_spread = 0` collapses the ramp to the single `hue` the scene has always
//! drawn, so the surface is a strict superset.

// Hot-path panic-denial pragma: `update`/`render` run every displayed frame.
// `configure` (expansion + turtle) is build-time but colocated, so it obeys the
// same panic-free bar.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::cell::RefCell;
use std::rc::Rc;

use super::super::Scene;
use super::super::common;
use super::renderer::{
    LineRenderer, Segment3dInstance, SegmentInstance, StrokeMetric, joined_chord,
};
use super::turtle::TurtleMode;
use super::{
    CapOverflow, ColorRamp, GeneratorConfig, LineInstance, MAX_LSYSTEM_DEPTH, MirrorSpec,
    OverflowContext, ViewTransform, grammar, replicate_mirror, transform_cached, turtle,
};
use crate::dsp::AnalysisFrame;
use crate::render::camera::{self, CameraParams};
use crate::render::palette::Palette;
use crate::render::scenes::{
    FamilyParam, FamilyRange, ParamGroup, ParamKind, ParamSpec, default_of,
};

/// The bounding sphere a space figure is fitted into, in world units: the
/// volume the camera orbits, and the radius `focus` is normalized across.
const SPACE_RADIUS: f32 = 1.0;

/// How the figure is grown: `[generator] growth`.
///
/// `Fixed` is the default and the cached model: every depth expanded and
/// walked once at load. `Endless` caches nothing and walks an effectively
/// infinite derivation a few steps a frame, keeping the newest `trail`
/// segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Growth {
    /// Every depth cached at load; a frame picks one.
    #[default]
    Fixed,
    /// A vine that never ends: a lazy stream walked into a ring.
    Endless,
}

impl Growth {
    /// Every mode, in the order the loader names them.
    pub const ALL: [Growth; 2] = [Growth::Fixed, Growth::Endless];

    /// The mode as a preset writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Growth::Fixed => "fixed",
            Growth::Endless => "endless",
        }
    }

    /// The mode a preset's spelling names, or `None` for an unknown one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == name)
    }
}

/// `[generator] trail`'s default: how many segments an endless figure keeps.
pub const DEFAULT_TRAIL: u32 = 2000;

/// The most draw steps one frame emits, however fast `grow` asks. A frame past
/// it emits this many and drops the rest of its backlog, so a huge `grow`
/// costs a bounded frame rather than a stall.
const EMIT_CEILING: u64 = 1024;

/// The most stream symbols one frame reads. A draw step can sit behind many
/// symbols that draw nothing, so the ceiling on draw steps alone does not
/// bound the work; this does, for any grammar.
const SYMBOL_BUDGET: usize = 65_536;

/// The world size of one draw step of an endless figure. A whole unit would be
/// most of the view: the flat view spans `[-1, 1]` vertically and the camera's
/// default distance shows about three units. On a flat figure `scale`
/// multiplies it.
const ENDLESS_STEP: f32 = 0.025;

/// One segment in an endless figure's ring, in draw-step units.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RingSeg {
    a: [f32; 3],
    b: [f32; 3],
    /// The generation it was drawn at: the colour axis (ADR-0059).
    generation: u32,
    /// Whether it continues the segment drawn before it.
    joined: bool,
    /// The flat joint extensions, in [`PLACEHOLDER_WIDTH`](super::PLACEHOLDER_WIDTH)
    /// units, as the flat walk stores them.
    ext_a: f32,
    ext_b: f32,
}

/// An endless figure's CPU state (Plan 0237): the stream, the pen walking it,
/// the ring of the newest `trail` segments and the integrated rate. No GPU in
/// it, so every claim about it is testable on the CPU.
///
/// **Nothing here allocates after construction.** The stream's frames, the
/// pen's branch stack and the ring are each reserved at their bound.
#[derive(Debug, Clone)]
pub(crate) struct Endless {
    stream: grammar::Stream,
    pen: turtle::Pen,
    ring: Vec<RingSeg>,
    trail: usize,
    /// Where the next segment is written once the ring is full: its oldest.
    head: usize,
    /// `grow * dt`, integrated: draw steps owed since the start. `f64`, so a
    /// long run keeps whole steps exact.
    phase: f64,
    /// Draw steps taken since the start.
    emitted: u64,
}

impl Endless {
    /// The endless walk of `axiom` under `rules`, at `angle` radians, keeping
    /// `trail` segments, through [`grammar::STREAM_DEPTH`] levels.
    pub(crate) fn new(
        axiom: &str,
        rules: &[(char, String)],
        angle: f32,
        mode: TurtleMode,
        trail: usize,
    ) -> Self {
        let trail = trail.max(1);
        let depth = grammar::STREAM_DEPTH;
        Self {
            stream: grammar::Stream::new(axiom, rules, depth),
            pen: turtle::Pen::new(mode, angle, grammar::bracket_bound(axiom, rules, depth)),
            ring: Vec::with_capacity(trail),
            trail,
            head: 0,
            phase: 0.0,
            emitted: 0,
        }
    }

    /// The deepest generation a segment can be drawn at: the ramp's divisor.
    pub(crate) fn generations(&self) -> u32 {
        u32::try_from(self.pen.stack_capacity()).unwrap_or(u32::MAX)
    }

    /// Draw steps taken since the start.
    #[cfg(test)]
    fn emitted(&self) -> u64 {
        self.emitted
    }

    /// Segments the ring holds.
    pub(crate) fn len(&self) -> usize {
        self.ring.len()
    }

    /// One frame's growth: `grow` draw steps a second for `dt` seconds,
    /// integrated (ADR-0019), and at most [`EMIT_CEILING`] of them. A negative
    /// or non-finite `grow` or `dt` grows nothing.
    pub(crate) fn advance(&mut self, grow: f32, dt: f32) {
        let safe = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        self.phase += f64::from(safe(grow)) * f64::from(safe(dt));
        let owed = (self.phase.floor() as u64).saturating_sub(self.emitted);
        let mut budget = SYMBOL_BUDGET;
        for _ in 0..owed.min(EMIT_CEILING) {
            if !self.step(&mut budget) {
                break;
            }
        }
        // A frame that could not pay what it owed drops the rest, rather than
        // carrying a backlog that would burst on the frames after it.
        if (self.phase.floor() as u64) > self.emitted {
            self.phase = self.emitted as f64;
        }
    }

    /// Read the stream until the pen draws one step, within `budget` symbols.
    /// Where the stream ends it starts again from the axiom, with the pen
    /// standing where it stopped, so the ring fades on without a jump.
    fn step(&mut self, budget: &mut usize) -> bool {
        while *budget > 0 {
            *budget -= 1;
            let Some(ch) = self.stream.next_symbol() else {
                self.stream.restart();
                self.pen.clear_branches();
                continue;
            };
            if let Some(drawn) = self.pen.read(ch) {
                self.push(drawn);
                self.emitted += 1;
                return true;
            }
        }
        false
    }

    /// Write one segment over the oldest, or beside it while the ring fills.
    fn push(&mut self, drawn: turtle::Drawn) {
        let mut seg = RingSeg {
            a: drawn.a,
            b: drawn.b,
            generation: drawn.generation,
            joined: drawn.joined,
            ext_a: 0.0,
            ext_b: 0.0,
        };
        if drawn.joined
            && let Some(prev) = self.newest_mut()
        {
            // The flat joint, measured as the flat walk measures it: the
            // extension depends only on the turn, so draw-step units serve.
            let ext = super::renderer::miter_extension(
                super::PLACEHOLDER_WIDTH,
                [prev.a[0], prev.a[1]],
                [prev.b[0], prev.b[1]],
                [drawn.b[0], drawn.b[1]],
            );
            prev.ext_b = ext;
            seg.ext_a = ext;
        }
        if self.ring.len() < self.trail {
            self.ring.push(seg);
            self.head = self.ring.len() % self.trail;
        } else if let Some(slot) = self.ring.get_mut(self.head) {
            *slot = seg;
            self.head = (self.head + 1) % self.trail;
        }
    }

    /// The ring index of the segment `age` steps old, `0` the newest.
    fn index(&self, age: usize) -> Option<usize> {
        (age < self.ring.len()).then(|| (self.head + self.trail - 1 - age) % self.trail)
    }

    /// The segment `age` steps old, `0` the newest.
    fn at(&self, age: usize) -> Option<&RingSeg> {
        self.index(age).and_then(|i| self.ring.get(i))
    }

    fn newest_mut(&mut self) -> Option<&mut RingSeg> {
        self.index(0).and_then(|i| self.ring.get_mut(i))
    }

    /// The reserved room of every buffer this holds, for a test to hold
    /// against growth.
    #[cfg(test)]
    fn capacities(&self) -> [usize; 2] {
        [self.ring.capacity(), self.pen.stack_capacity()]
    }
}

/// How much light a segment `age` steps old keeps in a ring of `trail`:
/// full until it reaches the oldest `tail` fraction, then falling linearly to
/// nothing at the ring's end. `tail` is clamped to `[0, 1]`; `0` fades none.
pub(crate) fn fade(age: usize, trail: usize, tail: f32) -> f32 {
    let tail = if tail.is_finite() {
        tail.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let zone = tail * trail as f32;
    if zone <= 0.0 {
        return 1.0;
    }
    (trail.saturating_sub(age) as f32 / zone).min(1.0)
}

const DEFAULT_VISIBLE_DEPTH: f32 = default_of(PARAMS, "visible_depth");
const DEFAULT_ROTATION: f32 = default_of(PARAMS, "rotation");
const DEFAULT_HUE: f32 = 0.3;
/// Colour surface (ADR-0021 / ADR-0059), at the value that reproduces the single
/// flat `hue` this scene drew before the palette reached it: no ramp along the depth axis.
/// The palette-A-alone and unmodified-saturation halves of that rest in
/// `scenes::common`, which every system shares them with.
const DEFAULT_HUE_SPREAD: f32 = 0.0;
const DEFAULT_DRAW_PROGRESS: f32 = 1.0;
const DEFAULT_THICKNESS: f32 = 1.8;
const DEFAULT_SCALE: f32 = 1.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
/// The line renderer's **per-segment falloff** multiplier (Plan 0038 Phase 1) —
/// not a post-process bloom. `1.0` is the value this scene passed as a literal
/// before it was bound, so the default is exactly today's look.
const DEFAULT_GLOW: f32 = 1.0;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
// Geometry mirror (Phase 4): identity by default.
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;
const DEFAULT_GROW: f32 = default_of(PARAMS, "grow");
const DEFAULT_TAIL: f32 = default_of(PARAMS, "tail");

/// A generator scene driven by an L-system grammar.
pub struct LSystemScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007).
    renderer: Rc<RefCell<LineRenderer>>,
    /// Base geometry per depth (index `d - 1`), built once in `configure`.
    /// Positions only; colour/width are applied per frame.
    cached: Vec<Vec<SegmentInstance>>,
    /// Each cached depth's per-segment **generation depth**, index-aligned with
    /// [`cached`](Self::cached) row for row and segment for segment (ADR-0059's
    /// colour axis). Built beside the geometry, off the hot path.
    cached_depths: Vec<Vec<u32>>,
    /// The deepest generation present in each cached depth — the ramp's divisor,
    /// resolved at build time so no per-frame param can move it. See the module
    /// docs on why this is not `visible_depth`.
    cached_max_depth: Vec<u32>,
    /// One colour per generation, rebuilt each frame and indexed by a segment's
    /// generation depth. Sized in `build` to the deepest generation across every
    /// cached depth, so the per-frame fill allocates nothing and samples the
    /// palette once per *generation* rather than once per segment.
    depth_colors: Vec<[f32; 3]>,
    /// Reused per-frame draw buffer — the mirrored geometry actually rendered.
    /// Preallocated so replication allocates nothing on the hot path.
    draw_buf: Vec<SegmentInstance>,
    /// Reused buffer for the single (pre-mirror) transformed depth, replicated
    /// into [`draw_buf`](Self::draw_buf) by [`replicate_mirror`]. Preallocated.
    single_buf: Vec<SegmentInstance>,
    /// The active tier's segment ceiling
    /// ([`TierConfig::max_segments`](crate::render::TierConfig::max_segments)),
    /// resolved once at construction (Plan 0044). A field rather than a constant
    /// so the tier can raise it; both buffers above are preallocated to it, which
    /// is what keeps the per-frame replication allocation-free.
    max_segments: usize,
    /// Set when this frame's mirror replication overflowed the cap (Phase 4);
    /// `None` when it fit. Distinct from the load-time `overflow` below.
    mirror_overflow: Option<CapOverflow>,
    /// If a depth overflowed the segment cap at load: `(depth, dropped)`. Kept
    /// queryable rather than silently discarded (ADR-0007 cap is never silent);
    /// curated presets stay under the cap so this is normally `None`.
    overflow: Option<(u32, usize)>,
    /// Shared scene clock (seconds).
    time: f32,
    /// The preset's baked colour LUT (ADR-0021), sampled on the CPU per
    /// generation. Defaults to the engine cosine, which is the ramp this scene
    /// coloured through before the palette reached it.
    palette: Palette,
    visible_depth: f32,
    rotation: f32,
    /// The shared palette knobs (ADR-0021).
    colour: common::PaletteParams,
    /// The shared view transform (ADR-0018).
    pan: common::PanParams,
    hue_spread: f32,
    draw_progress: f32,
    thickness: f32,
    scale: f32,
    glow: f32,
    softness: f32,
    /// Whether this figure draws through the **opacity-preserving** seam
    /// rather than the additive one, from `stroke_blend` (ADR-0138).
    ///
    /// At or above [`OPAQUE_BLEND`](super::OPAQUE_BLEND) the whole batch
    /// composites over: a stroke laid on another replaces the interior of what
    /// it covers instead of summing with it, so a quantized palette keeps its
    /// plateaus. Below it the batch is additive light. `0` is the default, so a
    /// preset that does not bind this draws exactly what it drew.
    stroke_blend: f32,
    zoom: f32,
    mirror_order: f32,
    mirror_reflect: f32,

    /// Which turtle the configured grammar was walked by (ADR-0258).
    turtle: TurtleMode,
    /// The space turtle's own renderer: the `seg3d` pipeline and an instance
    /// buffer of the tier's
    /// [`seg3d_segments`](crate::render::TierConfig::seg3d_segments), built with
    /// the scene so a preset switch to a space grammar allocates nothing on the
    /// GPU. The shared 2D renderer above has no `seg3d` pipeline.
    lines3d: LineRenderer,
    /// The tier's `seg3d_segments`: the most segments a space depth caches.
    seg3d_cap: usize,
    /// The tier's cap on the circle of confusion, in pixels.
    max_coc: f32,
    /// The render target's size in pixels, handed in every frame.
    target: (u32, u32),
    /// The shared camera block, read in `space` mode.
    camera: CameraParams,
    /// The space walk per depth (index `d - 1`), fitted into a sphere of
    /// [`SPACE_RADIUS`], built once in `configure`. Empty in `flat` mode, as
    /// [`cached`](Self::cached) is in `space`; [`cached_depths`](Self::cached_depths)
    /// and [`cached_max_depth`](Self::cached_max_depth) serve whichever is built.
    cached3d: Vec<Vec<Segment3dInstance>>,
    /// The cached space depth this frame draws, picked in `update`.
    space_depth: usize,
    /// How many of that depth's segments this frame's `draw_progress` reveals.
    space_keep: usize,
    /// Reused 3D instance buffer, preallocated to `seg3d_cap`.
    instances3d: Vec<Segment3dInstance>,

    /// The endless figure, when `[generator] growth = "endless"`; `None` for
    /// a fixed one, whose geometry is the caches above.
    endless: Option<Endless>,
    /// The clamp of `trail` to the tier's cap, announced from `configure`.
    trail_overflow: Option<CapOverflow>,
    /// This frame's elapsed time, stored by `advance` for `update`.
    dt: f32,
    /// `grow`: draw steps a second.
    grow: f32,
    /// `tail`: the oldest fraction of the ring that fades.
    tail: f32,
    /// The endless ring's bounding radius about the orbit target, in world
    /// units, measured in `update`: what `focus` resolves against.
    endless_radius: f32,
}

impl LSystemScene {
    /// Build the scene over the shared line renderer, preallocating the draw
    /// buffer, and the space turtle's own `seg3d` renderer with `seg3d_cap`
    /// instances and blur held to `max_coc` pixels — the tier's caps. No grammar
    /// is expanded until a preset configures one.
    pub fn new(
        renderer: Rc<RefCell<LineRenderer>>,
        max_segments: usize,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        seg3d_cap: usize,
        max_coc: f32,
    ) -> Self {
        Self {
            turtle: TurtleMode::Flat,
            lines3d: LineRenderer::new_3d(device, surface_format, seg3d_cap, "lsystem-3d"),
            seg3d_cap,
            max_coc,
            target: (1, 1),
            camera: CameraParams::default(),
            cached3d: Vec::new(),
            space_depth: 0,
            space_keep: 0,
            instances3d: Vec::with_capacity(seg3d_cap),
            endless: None,
            trail_overflow: None,
            dt: super::super::FALLBACK_DT,
            grow: DEFAULT_GROW,
            tail: DEFAULT_TAIL,
            endless_radius: SPACE_RADIUS,
            renderer,
            cached: Vec::new(),
            cached_depths: Vec::new(),
            cached_max_depth: Vec::new(),
            depth_colors: Vec::new(),
            draw_buf: Vec::with_capacity(max_segments),
            single_buf: Vec::with_capacity(max_segments),
            max_segments,
            mirror_overflow: None,
            overflow: None,
            time: 0.0,
            // Replaced by the preset's palette on the next switch; the default
            // is the engine cosine, so an unconfigured scene still colours.
            palette: Palette::default_spectrum(),
            visible_depth: DEFAULT_VISIBLE_DEPTH,
            rotation: DEFAULT_ROTATION,
            colour: common::PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            hue_spread: DEFAULT_HUE_SPREAD,
            draw_progress: DEFAULT_DRAW_PROGRESS,
            thickness: DEFAULT_THICKNESS,
            scale: DEFAULT_SCALE,
            glow: DEFAULT_GLOW,
            softness: super::DEFAULT_SOFTNESS,
            stroke_blend: super::ADDITIVE_BLEND,
            zoom: DEFAULT_ZOOM,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }

    /// Expand + turtle-walk each depth `1..=max_depth` into a cached buffer.
    /// Off the hot path (called from `configure`).
    fn build(
        &mut self,
        axiom: &str,
        rules: &[(char, String)],
        angle_deg: f32,
        max_depth: u32,
        mode: TurtleMode,
    ) {
        self.cached.clear();
        self.cached3d.clear();
        self.cached_depths.clear();
        self.cached_max_depth.clear();
        self.overflow = None;
        self.turtle = mode;
        let depth = max_depth.clamp(1, MAX_LSYSTEM_DEPTH);
        let angle = angle_deg.to_radians();

        for d in 1..=depth {
            let string = grammar::expand(axiom, rules, d);
            let mut generations = Vec::new();
            let dropped = match mode {
                TurtleMode::Flat => {
                    let mut segs = Vec::new();
                    let dropped = turtle::walk_with_depths(
                        &string,
                        angle,
                        self.max_segments,
                        &mut segs,
                        &mut generations,
                    );
                    turtle::normalize_fit(&mut segs, 0.9);
                    self.cached.push(segs);
                    dropped
                }
                TurtleMode::Space => {
                    let mut segs = Vec::new();
                    let dropped = turtle::walk_3d_with_depths(
                        &string,
                        angle,
                        self.seg3d_cap,
                        &mut segs,
                        &mut generations,
                    );
                    turtle::sphere_fit(&mut segs, SPACE_RADIUS);
                    self.cached3d.push(segs);
                    dropped
                }
            };
            if dropped > 0 && self.overflow.is_none() {
                self.overflow = Some((d, dropped));
            }
            self.cached_max_depth
                .push(generations.iter().copied().max().unwrap_or(0));
            self.cached_depths.push(generations);
        }
        // One colour slot per reachable generation, sized once here so the
        // per-frame fill neither allocates nor indexes out of range.
        let generations = self
            .cached_max_depth
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_add(1) as usize;
        self.depth_colors.clear();
        self.depth_colors.resize(generations, [0.0; 3]);
    }

    /// The cached depth `visible_depth` names, as an index into whichever cache
    /// was built, or `None` before a grammar is configured.
    fn depth_index(&self, depths: usize) -> Option<usize> {
        if depths == 0 {
            return None;
        }
        let want = self.visible_depth.max(1.0) as usize;
        Some(want.min(depths).saturating_sub(1))
    }

    /// The colour ramp this frame's palette knobs describe.
    fn ramp(&self) -> ColorRamp {
        ColorRamp {
            hue: self.colour.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.colour.mix,
            palette_steps: self.colour.steps,
            saturation: self.colour.saturation,
            brightness: self.colour.brightness,
        }
    }

    /// A space frame's CPU half (ADR-0258): pick the depth, colour its
    /// generations and count the `draw_progress` prefix. No rotation, scale or
    /// mirror reaches it, and the flat buffer is emptied so nothing 2D is drawn
    /// beside it.
    fn update_space(&mut self) {
        self.draw_buf.clear();
        self.mirror_overflow = None;
        let Some(idx) = self.depth_index(self.cached3d.len()) else {
            self.space_keep = 0;
            return;
        };
        let ramp = self.ramp();
        fill_depth_colors(
            &mut self.depth_colors,
            &self.palette,
            ramp,
            self.cached_max_depth.get(idx).copied().unwrap_or(0),
        );
        let len = self.cached3d.get(idx).map_or(0, Vec::len);
        // The same rounding `transform_cached` reveals the flat prefix by.
        self.space_keep = ((len as f32) * self.draw_progress.clamp(0.0, 1.0)).round() as usize;
        self.space_depth = idx;
    }

    /// Draw the cached space depth through the shared camera: each segment
    /// clipped against the near plane, culled when wholly off one edge of the
    /// frame, coloured by its generation (ADR-0059), and stroked `thickness`
    /// pixels wide at the focal plane.
    fn render_space(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // The aspect is the render target's, handed in here (ADR-0037).
        let frame = self.camera.frame(
            aspect,
            self.zoom,
            [self.pan.x, self.pan.y],
            self.target,
            SPACE_RADIUS,
            self.max_coc,
        );
        if self.mirror_overflow.is_none() {
            self.mirror_overflow = frame.blur;
        }
        let width = if self.thickness.is_finite() {
            self.thickness.max(0.0)
        } else {
            DEFAULT_THICKNESS
        };
        let trunk = self.depth_colors.first().copied().unwrap_or([1.0; 3]);
        let base = self
            .cached3d
            .get(self.space_depth)
            .map_or(&[][..], Vec::as_slice);
        let generations = self
            .cached_depths
            .get(self.space_depth)
            .map_or(&[][..], Vec::as_slice);
        let keep = self.space_keep.min(base.len());

        let mut instances = std::mem::take(&mut self.instances3d);
        instances.clear();
        for (i, seg) in base.iter().take(keep).enumerate() {
            // A joined end names its neighbour's far point; a free end names
            // itself (`walk_3d_with_depths`). A neighbour past the revealed
            // prefix is not drawn, so that end draws free.
            let before = (seg.prev != seg.a).then_some(seg.prev);
            let after = (seg.next != seg.b && i + 1 < keep).then_some(seg.next);
            let Some([prev, a, b, next]) = joined_chord(&frame.view, before, seg.a, seg.b, after)
            else {
                continue;
            };
            if frame.view.outside(a, b, frame.margin) {
                continue;
            }
            let color = generations
                .get(i)
                .and_then(|&g| self.depth_colors.get(g as usize))
                .copied()
                .unwrap_or(trunk);
            instances.push(Segment3dInstance {
                a,
                b,
                color,
                width,
                alpha: 1.0,
                prev,
                next,
                skirt: 0.0,
            });
        }
        self.lines3d.draw_3d(
            queue,
            encoder,
            view,
            &frame,
            self.glow,
            self.softness,
            &instances,
        );
        self.instances3d = instances;
    }

    /// Set up an endless figure (Plan 0237): no cache, a stream and a ring of
    /// `trail` segments, held to the tier's cap for the turtle — `seg3d_segments`
    /// in space, `max_segments` on the plane — and the clamp kept to announce.
    fn configure_endless(
        &mut self,
        axiom: &str,
        rules: &[(char, String)],
        angle_deg: f32,
        mode: TurtleMode,
        trail: u32,
    ) {
        self.cached.clear();
        self.cached3d.clear();
        self.cached_depths.clear();
        self.cached_max_depth.clear();
        self.overflow = None;
        self.turtle = mode;
        let cap = match mode {
            TurtleMode::Flat => self.max_segments,
            TurtleMode::Space => self.seg3d_cap,
        };
        let asked = trail as usize;
        self.trail_overflow = (asked > cap).then(|| CapOverflow {
            dropped: asked - cap,
            context: OverflowContext::Trail {
                asked: trail,
                space: mode == TurtleMode::Space,
            },
            cap,
        });
        let endless = Endless::new(axiom, rules, angle_deg.to_radians(), mode, asked.min(cap));
        // One colour slot per generation the pen can reach, sized here so the
        // per-frame fill neither allocates nor indexes out of range.
        self.depth_colors.clear();
        self.depth_colors
            .resize(endless.generations() as usize + 1, [0.0; 3]);
        self.endless = Some(endless);
    }

    /// An endless frame's CPU half: grow the ring by this frame's `grow`,
    /// colour the generations, and on the plane lay the ring into the draw
    /// buffer, oldest first, fading by age over the `tail`.
    fn update_endless(&mut self) {
        self.mirror_overflow = None;
        self.draw_buf.clear();
        let ramp = self.ramp();
        let Some(endless) = self.endless.as_mut() else {
            return;
        };
        endless.advance(self.grow, self.dt);
        fill_depth_colors(
            &mut self.depth_colors,
            &self.palette,
            ramp,
            endless.generations(),
        );
        let len = endless.len();
        let trunk = self.depth_colors.first().copied().unwrap_or([1.0; 3]);

        if self.turtle == TurtleMode::Space {
            // The ring's reach about the orbit target, for `focus`.
            let mut reach = 0.0f32;
            for age in 0..len {
                if let Some(seg) = endless.at(age) {
                    for p in [seg.a, seg.b] {
                        let w = endless_world(p);
                        reach = reach.max((w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt());
                    }
                }
            }
            self.endless_radius = reach.max(ENDLESS_STEP);
            return;
        }

        let width = super::half_width(self.thickness);
        let (sin, cos) = self.rotation.sin_cos();
        let step = ENDLESS_STEP;
        for age in (0..len).rev() {
            let Some(seg) = endless.at(age) else {
                continue;
            };
            // The oldest segment's predecessor has left the ring, so its
            // joint has nothing to reach toward.
            let ext_a = if age + 1 < len { seg.ext_a } else { 0.0 };
            let flat = SegmentInstance {
                a: [seg.a[0] * step, seg.a[1] * step],
                b: [seg.b[0] * step, seg.b[1] * step],
                color: [1.0; 3],
                width: super::PLACEHOLDER_WIDTH,
                alpha: 1.0,
                ext_a,
                ext_b: seg.ext_b,
            };
            let color = self
                .depth_colors
                .get(seg.generation as usize)
                .copied()
                .unwrap_or(trunk);
            let mut instance = flat
                .rotate_scale(sin, cos, self.rotation, self.scale)
                .styled(color, width);
            instance.alpha = fade(age, endless.trail, self.tail);
            self.draw_buf.push(instance);
        }
    }

    /// Draw an endless space figure's ring through the shared camera, oldest
    /// first, each segment faded by age and joined to its neighbours in the
    /// ring.
    fn render_endless_space(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        let frame = self.camera.frame(
            aspect,
            self.zoom,
            [self.pan.x, self.pan.y],
            self.target,
            self.endless_radius,
            self.max_coc,
        );
        if self.mirror_overflow.is_none() {
            self.mirror_overflow = frame.blur;
        }
        let width = if self.thickness.is_finite() {
            self.thickness.max(0.0)
        } else {
            DEFAULT_THICKNESS
        };
        let trunk = self.depth_colors.first().copied().unwrap_or([1.0; 3]);
        let mut instances = std::mem::take(&mut self.instances3d);
        instances.clear();
        if let Some(endless) = self.endless.as_ref() {
            let len = endless.len();
            for age in (0..len).rev() {
                let Some(seg) = endless.at(age) else {
                    continue;
                };
                let before = if seg.joined && age + 1 < len {
                    endless.at(age + 1).map(|p| endless_world(p.a))
                } else {
                    None
                };
                let after = match age.checked_sub(1).and_then(|n| endless.at(n)) {
                    Some(next) if next.joined => Some(endless_world(next.b)),
                    _ => None,
                };
                let Some([prev, a, b, next]) = joined_chord(
                    &frame.view,
                    before,
                    endless_world(seg.a),
                    endless_world(seg.b),
                    after,
                ) else {
                    continue;
                };
                if frame.view.outside(a, b, frame.margin) {
                    continue;
                }
                instances.push(Segment3dInstance {
                    a,
                    b,
                    color: self
                        .depth_colors
                        .get(seg.generation as usize)
                        .copied()
                        .unwrap_or(trunk),
                    width,
                    alpha: fade(age, endless.trail, self.tail),
                    prev,
                    next,
                    skirt: 0.0,
                });
            }
        }
        self.lines3d.draw_3d(
            queue,
            encoder,
            view,
            &frame,
            self.glow,
            self.softness,
            &instances,
        );
        self.instances3d = instances;
    }
}

/// A point of an endless figure, from draw-step units to world units.
fn endless_world(p: [f32; 3]) -> [f32; 3] {
    [
        p[0] * ENDLESS_STEP,
        p[1] * ENDLESS_STEP,
        p[2] * ENDLESS_STEP,
    ]
}

/// Fill `out[g]` with generation `g`'s stroke colour, walking the shared
/// [`ColorRamp`] over the depth axis. `generations` is the deepest generation in
/// the visible figure — the ramp's divisor, so `hue_spread = 1` spans the palette
/// exactly once whatever the grammar's branching factor. A bracket-free figure
/// passes `0` here and colours flat; see the module docs.
///
/// Allocation-free into a buffer sized at build time, and one palette sample
/// **per generation** rather than per segment — every segment of a generation is
/// the same colour by definition, and a figure has a couple of dozen generations
/// against up to `max_segments` segments.
pub(crate) fn fill_depth_colors(
    out: &mut [[f32; 3]],
    palette: &Palette,
    ramp: ColorRamp,
    generations: u32,
) {
    let span = generations.max(1) as f32;
    for (generation, slot) in out.iter_mut().enumerate() {
        *slot = ramp.at(palette, generation as f32 / span);
    }
}

/// Colour each segment by **its own generation**, reading `colors` at the
/// generation `generations[i]` records for it.
///
/// `segs` is the transformed figure and `generations` the cached depth's
/// per-segment generation array. `transform_cached` keeps a **prefix** of the
/// cached geometry (the `draw_progress` reveal), so `zip` pairs each drawn
/// segment with its own generation and simply stops at the shorter of the two.
pub(crate) fn apply_depth_colors(
    segs: &mut [SegmentInstance],
    generations: &[u32],
    colors: &[[f32; 3]],
) {
    for (seg, &generation) in segs.iter_mut().zip(generations) {
        if let Some(&color) = colors.get(generation as usize) {
            seg.color = color;
        }
    }
}

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[ParamSpec] = &[
    VISIBLE_DEPTH,
    ROTATION,
    crate::render::scenes::common::hue(DEFAULT_HUE),
    crate::render::scenes::lines::hue_spread(DEFAULT_HUE_SPREAD),
    crate::render::scenes::common::SATURATION,
    crate::render::scenes::common::PALETTE_MIX,
    crate::render::scenes::common::PALETTE_STEPS,
    crate::render::scenes::common::PALETTE_CONTOUR,
    DRAW_PROGRESS,
    THICKNESS,
    SCALE,
    crate::render::scenes::common::brightness(DEFAULT_BRIGHTNESS),
    crate::render::scenes::lines::GLOW,
    crate::render::scenes::lines::SOFTNESS,
    crate::render::scenes::common::zoom(DEFAULT_ZOOM),
    crate::render::scenes::common::PAN_X,
    crate::render::scenes::common::PAN_Y,
    STROKE_BLEND,
    MIRROR_ORDER,
    MIRROR_REFLECT,
    YAW,
    PITCH,
    DISTANCE,
    FOV,
    FOCUS,
    APERTURE,
    FOG,
    SOLID,
    GROW,
    TAIL,
];

const VISIBLE_DEPTH: ParamSpec = ParamSpec {
    name: "visible_depth",
    default: 1.0,
    range: Some([1.0, 7.0]),
    doc: "Which recursion generation is drawn, counted from 1 and capped at `max_depth`; a \
          fraction floors to the generation below it, and anything under 2 draws the first.",
    kind: ParamKind::Modal,
    group: ParamGroup::Shape,
    main: true,
};
const GROW: ParamSpec = ParamSpec {
    name: "grow",
    default: 20.0,
    range: Some([0.0, 120.0]),
    doc: "How fast an endless figure grows, in draw steps a second; bind it to onset to make \
          it surge.",
    kind: ParamKind::Modal,
    group: ParamGroup::Motion,
    main: true,
};
const TAIL: ParamSpec = ParamSpec {
    name: "tail",
    default: 0.5,
    range: Some([0.0, 1.0]),
    doc: "The oldest fraction of an endless figure's trail that fades out; 0 keeps every \
          segment at full light until it is dropped.",
    kind: ParamKind::Modal,
    group: ParamGroup::Light,
    main: false,
};

// The specs whose reading depends on the turtle or the growth, re-declared with
// a doc line of this system's own. The editor schema keys a family row by the
// whole declaration, so one shared with another system would print this
// system's modes on that system's hover. Default, range and kind stay the
// shared block's, so only the wording can differ.
const DRAW_PROGRESS: ParamSpec = ParamSpec {
    doc: "How much of a fixed L-system is drawn, from its first segment: 0 none, 1 all of it.",
    ..crate::render::scenes::lines::DRAW_PROGRESS
};
const ROTATION: ParamSpec = ParamSpec {
    name: "rotation",
    default: 0.0,
    range: Some([0.0, std::f32::consts::TAU]),
    doc: "Turns a flat figure in its plane, in radians.",
    kind: ParamKind::Modal,
    group: ParamGroup::Motion,
    main: false,
};
const THICKNESS: ParamSpec = ParamSpec {
    doc: "Stroke width: on a flat figure in the shared line units, on a space figure in pixels \
          at the focal plane, wider nearer and narrower farther.",
    ..crate::render::scenes::lines::thickness(DEFAULT_THICKNESS)
};
const SCALE: ParamSpec = ParamSpec {
    doc: "Scales a flat figure about its centre; a space figure is sized by the camera's \
          distance instead.",
    ..crate::render::scenes::lines::scale(DEFAULT_SCALE)
};
const STROKE_BLEND: ParamSpec = ParamSpec {
    doc: "Moves a flat L-system's stroke from additive light toward opaque paint, so \
          crossing branches stop brightening.",
    ..crate::render::scenes::lines::STROKE_BLEND
};
const MIRROR_ORDER: ParamSpec = ParamSpec {
    doc: "Repeats a flat L-system this many times around the centre; 1 draws it once.",
    ..crate::render::scenes::lines::MIRROR_ORDER
};
const MIRROR_REFLECT: ParamSpec = ParamSpec {
    doc: "Alternates a flat L-system's repeats into mirror images rather than plain rotations.",
    ..crate::render::scenes::lines::MIRROR_REFLECT
};
const YAW: ParamSpec = ParamSpec {
    doc: "Turns the camera around a space tree, in radians; bind it to a slow clock to orbit.",
    ..camera::YAW
};
const PITCH: ParamSpec = ParamSpec {
    doc: "Raises the camera above a space tree, in radians; negative looks up from below.",
    ..camera::PITCH
};
const DISTANCE: ParamSpec = ParamSpec {
    doc: "How far the camera sits from a space tree's centre; nearer makes the tree larger \
          and exaggerates the perspective.",
    ..camera::DISTANCE
};
const FOV: ParamSpec = ParamSpec {
    doc: "The camera's vertical field of view onto a space tree, in radians; zoom divides it.",
    ..camera::FOV
};
const FOCUS: ParamSpec = ParamSpec {
    doc: "Where the focal plane sits in a space tree's depth: 0 at its nearest point, 1 at its \
          farthest.",
    ..camera::FOCUS
};
const APERTURE: ParamSpec = ParamSpec {
    doc: "The blur of a space tree's far side, in pixels; branches nearer than the focal plane \
          blur more, up to the tier's cap. 0 keeps every branch sharp, and wider costs fill.",
    ..camera::APERTURE
};
const FOG: ParamSpec = ParamSpec {
    doc: "Fades a space tree toward black with depth: at 1 its farthest point is black and its \
          nearest keeps its light. 0 is off.",
    ..camera::FOG
};
const SOLID: ParamSpec = ParamSpec {
    doc: "1 paints a space tree's near branches over its far ones, so it reads as an object and \
          crossings stop brightening; 0 is the additive glow. Solid sorts every segment by \
          depth each frame.",
    ..camera::SOLID
};

/// The modes [`FAMILY_PARAMS`] is written against: each turtle, fixed and
/// endless, in that order.
pub const MODES: [&str; 4] = ["flat", "space", "flat endless", "space endless"];

/// One row of [`FAMILY_PARAMS`], its ranges in [`MODES`]' order. `None` is a
/// mode that does not read the parameter.
macro_rules! per_mode {
    ($name:expr; $flat:expr, $space:expr, $flat_endless:expr, $space_endless:expr $(,)?) => {
        FamilyParam {
            name: $name,
            ranges: &[
                FamilyRange {
                    family: MODES[0],
                    range: $flat,
                },
                FamilyRange {
                    family: MODES[1],
                    range: $space,
                },
                FamilyRange {
                    family: MODES[2],
                    range: $flat_endless,
                },
                FamilyRange {
                    family: MODES[3],
                    range: $space_endless,
                },
            ],
        }
    };
}

/// A row read on the plane, fixed or endless: the in-plane transform and the
/// opaque stroke path, which `seg3d` does not draw (ADR-0258).
macro_rules! plane {
    ($spec:expr) => {
        per_mode!($spec.name; $spec.range, None, $spec.range, None)
    };
}

/// A row read by the flat fixed figure alone: the mirror, which neither the
/// space turtle nor an endless figure replicates.
macro_rules! flat_fixed {
    ($spec:expr) => {
        per_mode!($spec.name; $spec.range, None, None, None)
    };
}

/// A row read by the space turtle, fixed or endless: the camera block.
macro_rules! in_space {
    ($spec:expr) => {
        per_mode!($spec.name; None, $spec.range, None, $spec.range)
    };
}

/// A row read by a fixed figure alone: the cached depth and its reveal.
macro_rules! fixed {
    ($spec:expr) => {
        per_mode!($spec.name; $spec.range, $spec.range, None, None)
    };
}

/// A row read by an endless figure alone: its rate and its fade.
macro_rules! endless {
    ($spec:expr) => {
        per_mode!($spec.name; None, None, $spec.range, $spec.range)
    };
}

/// Every parameter whose reading depends on the turtle or the growth
/// (ADR-0258, ADR-0180 rule 4). The generated reference prints each as inert
/// on the modes that ignore it.
pub const FAMILY_PARAMS: &[FamilyParam] = &[
    fixed!(VISIBLE_DEPTH),
    fixed!(DRAW_PROGRESS),
    plane!(ROTATION),
    plane!(SCALE),
    plane!(STROKE_BLEND),
    flat_fixed!(MIRROR_ORDER),
    flat_fixed!(MIRROR_REFLECT),
    in_space!(YAW),
    in_space!(PITCH),
    in_space!(DISTANCE),
    in_space!(FOV),
    in_space!(FOCUS),
    in_space!(APERTURE),
    in_space!(FOG),
    in_space!(SOLID),
    endless!(GROW),
    endless!(TAIL),
];

impl Scene for LSystemScene {
    fn name(&self) -> &'static str {
        "l-system"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn advance(&mut self, dt: f32) {
        // Stored, not integrated: `update` grows the endless figure, so the
        // scene has one integration site.
        self.dt = dt;
    }

    fn reset_params(&mut self) {
        self.visible_depth = DEFAULT_VISIBLE_DEPTH;
        self.rotation = DEFAULT_ROTATION;
        self.colour.reset();
        self.pan.reset();
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.draw_progress = DEFAULT_DRAW_PROGRESS;
        self.thickness = DEFAULT_THICKNESS;
        self.scale = DEFAULT_SCALE;
        self.glow = DEFAULT_GLOW;
        self.softness = super::DEFAULT_SOFTNESS;
        self.stroke_blend = super::ADDITIVE_BLEND;
        self.zoom = DEFAULT_ZOOM;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
        self.camera.reset();
        self.grow = DEFAULT_GROW;
        self.tail = DEFAULT_TAIL;
    }

    fn set_target_size(&mut self, width: u32, height: u32) {
        self.target = (width, height);
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`, `render::camera`).
        if self.colour.set(name, value) || self.pan.set(name, value) || self.camera.set(name, value)
        {
            return;
        }
        match name {
            "visible_depth" => self.visible_depth = value,
            "rotation" => self.rotation = value,
            "hue_spread" => self.hue_spread = value,
            "draw_progress" => self.draw_progress = value,
            "thickness" => self.thickness = value,
            "scale" => self.scale = value,
            "glow" => self.glow = value,
            "softness" => self.softness = value,
            "zoom" => self.zoom = value,
            "stroke_blend" => self.stroke_blend = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            "grow" => self.grow = value,
            "tail" => self.tail = value,
            _ => {}
        }
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // Build + cache the grammar's geometry off the hot path. Every other
        // variant belongs to a sibling scene: matching only this one is what
        // keeps a new variant from editing four scenes that do not use it, and
        // `GeneratorConfig::element_count` is the one place that still has to
        // acknowledge every variant.
        if let GeneratorConfig::LSystem {
            axiom,
            rules,
            angle_deg,
            max_depth,
            seed: _,
            turtle,
            growth,
            trail,
        } = cfg
        {
            self.endless = None;
            self.trail_overflow = None;
            match growth {
                Growth::Fixed => self.build(axiom, rules, *angle_deg, *max_depth, *turtle),
                Growth::Endless => {
                    self.configure_endless(axiom, rules, *angle_deg, *turtle, *trail);
                    return self.trail_overflow;
                }
            }
        }
        // Surface a cap truncation so the frontend can report it — never a
        // silent cut (ADR-0007). `None` when every depth fit (the norm). The
        // cap that bit is the one the active turtle walked under.
        let cap = match self.turtle {
            TurtleMode::Flat => self.max_segments,
            TurtleMode::Space => self.seg3d_cap,
        };
        self.overflow.map(|(depth, dropped)| CapOverflow {
            dropped,
            context: OverflowContext::Depth(depth),
            cap,
        })
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        if self.endless.is_some() {
            self.update_endless();
            return;
        }
        if self.turtle == TurtleMode::Space {
            self.update_space();
            return;
        }
        // Pick the visible depth (1-based) and its cached base geometry.
        let Some(idx) = self.depth_index(self.cached.len()) else {
            self.draw_buf.clear();
            return;
        };
        let Some(base) = self.cached.get(idx) else {
            self.draw_buf.clear();
            return;
        };

        // The colour ramp along the **generation-depth** axis (ADR-0059). One
        // palette sample per generation rather than per segment: a figure has at
        // most a couple of dozen generations and up to `max_segments` segments,
        // and every segment of a generation is the same colour by definition.
        //
        // `hue_spread = 0` makes every slot `hue`, which is the single flat
        // colour this scene drew before the palette reached it.
        fill_depth_colors(
            &mut self.depth_colors,
            &self.palette,
            ColorRamp {
                hue: self.colour.hue,
                hue_spread: self.hue_spread,
                palette_mix: self.colour.mix,
                palette_steps: self.colour.steps,
                saturation: self.colour.saturation,
                brightness: self.colour.brightness,
            },
            self.cached_max_depth.get(idx).copied().unwrap_or(0),
        );
        let trunk = self.depth_colors.first().copied().unwrap_or([1.0; 3]);

        let width = super::half_width(self.thickness);
        transform_cached(
            base,
            self.rotation,
            self.scale,
            trunk,
            width,
            self.draw_progress,
            &mut self.single_buf,
        );
        if let Some(generations) = self.cached_depths.get(idx) {
            apply_depth_colors(&mut self.single_buf, generations, &self.depth_colors);
        }
        // Replicate the single transformed depth under the geometry mirror (Phase
        // 4). At the default identity spec, skip it: replication would copy the
        // whole segment set into a second buffer to produce exactly what it was
        // given, so swap instead — O(1), and both buffers were preallocated to
        // `max_segments`, so neither can grow later. `transform_cached` clears
        // before it fills, so whatever lands back in `single_buf` is overwritten.
        let mirror = MirrorSpec::from_params(self.mirror_order, self.mirror_reflect);
        if mirror.is_identity() {
            debug_assert!(
                self.single_buf.len() <= self.max_segments,
                "the cached base is capped at load, so identity cannot truncate"
            );
            std::mem::swap(&mut self.single_buf, &mut self.draw_buf);
            self.mirror_overflow = None;
            return;
        }
        let dropped = replicate_mirror(
            &self.single_buf,
            mirror,
            self.max_segments,
            &mut self.draw_buf,
        );
        self.mirror_overflow = (dropped > 0).then_some(CapOverflow {
            dropped,
            context: OverflowContext::Mirror(mirror.order),
            cap: self.max_segments,
        });
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        if self.turtle == TurtleMode::Space {
            if self.endless.is_some() {
                self.render_endless_space(queue, encoder, view, aspect);
            } else {
                self.render_space(queue, encoder, view, aspect);
            }
            return;
        }
        let xform = ViewTransform {
            zoom: self.zoom,
            pan: [self.pan.x, self.pan.y],
            _pad: 0.0,
        };
        let mut renderer = self.renderer.borrow_mut();
        if self.stroke_blend >= super::OPAQUE_BLEND {
            renderer.draw_opaque(
                queue,
                encoder,
                view,
                aspect,
                self.glow,
                self.softness,
                StrokeMetric::World,
                xform,
                &self.draw_buf,
                &[],
            );
        } else {
            renderer.draw(
                queue,
                encoder,
                view,
                aspect,
                self.glow,
                self.softness,
                StrokeMetric::World,
                xform,
                &self.draw_buf,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    /// The cap these tests run at — the floor tier's, which is the value the
    /// assertions below were written against and the one every shipped preset is
    /// authored and gated on.
    const CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

    /// A fixed base + repeated per-frame transforms must not grow the draw
    /// buffer — the per-frame half is allocation-free (ADR-0007). This is the
    /// "inspection" proof; expansion/turtle-walking live only in `build`.
    #[test]
    fn per_frame_transform_does_not_allocate() {
        let mut base = Vec::with_capacity(64);
        turtle::walk("F+F+F+F+F[-F]F", 0.5, CAP, &mut base);
        turtle::normalize_fit(&mut base, 0.9);

        let mut out = Vec::with_capacity(CAP);
        let cap = out.capacity();
        for frame in 0..16 {
            let rotation = frame as f32 * 0.05;
            transform_cached(&base, rotation, 1.0, [0.5; 3], 0.01, 1.0, &mut out);
        }
        assert_eq!(out.capacity(), cap, "per-frame transform reused the buffer");
        assert_eq!(out.len(), base.len(), "full progress draws every segment");
    }

    /// Walk a grammar the way `build` does and hand back the figure with its
    /// per-segment generations — the two arrays the colour path pairs up.
    fn figure(string: &str) -> (Vec<SegmentInstance>, Vec<u32>, u32) {
        let mut segs = Vec::new();
        let mut generations = Vec::new();
        turtle::walk_with_depths(string, 0.4, CAP, &mut segs, &mut generations);
        let deepest = generations.iter().copied().max().unwrap_or(0);
        (segs, generations, deepest)
    }

    /// Colour the figure exactly as `update` does, at a given spread.
    fn coloured(string: &str, hue_spread: f32) -> (Vec<SegmentInstance>, Vec<u32>) {
        let (mut segs, generations, deepest) = figure(string);
        let mut colors = vec![[0.0; 3]; deepest as usize + 1];
        fill_depth_colors(
            &mut colors,
            &Palette::default_spectrum(),
            ColorRamp {
                hue: DEFAULT_HUE,
                hue_spread,
                palette_mix: common::DEFAULT_PALETTE_MIX,
                palette_steps: crate::render::palette::DEFAULT_PALETTE_STEPS,
                saturation: common::DEFAULT_SATURATION,
                brightness: DEFAULT_BRIGHTNESS,
            },
            deepest,
        );
        apply_depth_colors(&mut segs, &generations, &colors);
        (segs, generations)
    }

    /// Plan 0054 Phase 1 done-when 2, ADR-0059's axis choice. **Both halves
    /// matter**: different generations must differ, and — the half that tells
    /// depth apart from traversal order — segments of the *same* generation must
    /// agree even when the walk visits them far apart.
    #[test]
    fn the_spread_colours_by_generation_and_not_by_traversal_order() {
        // Two first-generation branches at opposite ends of the walk, with a
        // second-generation branch inside the later one.
        let string = "F[+F]FF[+F[-F]F]F";
        let (segs, generations) = coloured(string, 0.6);
        assert!(
            generations.iter().copied().max().unwrap_or(0) >= 2,
            "the probe must actually branch twice, or this proves nothing"
        );

        // Same generation -> same colour, however far apart in the walk.
        for (i, a) in segs.iter().enumerate() {
            for (j, b) in segs.iter().enumerate() {
                if generations[i] == generations[j] {
                    assert_eq!(
                        a.color, b.color,
                        "segments {i} and {j} share generation {} and must share \
                         a colour — a traversal-order ramp would give them two",
                        generations[i]
                    );
                } else {
                    assert_ne!(
                        a.color, b.color,
                        "segments {i} and {j} sit at generations {} and {} and \
                         must differ",
                        generations[i], generations[j]
                    );
                }
            }
        }

        // A traversal-order ramp would have coloured the walk monotonically.
        // It does not: the trunk resumes its own colour after a branch.
        let first_trunk = generations.iter().position(|&g| g == 0).unwrap_or(0);
        let last_trunk = generations.len() - 1;
        assert_eq!(
            segs[first_trunk].color, segs[last_trunk].color,
            "the trunk keeps one colour on both sides of the branches"
        );
    }

    /// The other half of the superset claim: `hue_spread = 0` is one flat colour
    /// across every generation — exactly what this scene drew before ADR-0059,
    /// so no shipped preset moves until it opts in.
    #[test]
    fn zero_spread_is_one_flat_colour_over_every_generation() {
        let (flat, _) = coloured("F[+F]F[+F[-F]]F", 0.0);
        let first = flat.first().map(|s| s.color).unwrap_or([0.0; 3]);
        for (i, seg) in flat.iter().enumerate() {
            assert_eq!(seg.color, first, "segment {i} must carry the single hue");
        }
    }

    /// A bracket-free grammar (the Sierpinski arrowhead is the shipped one) has
    /// exactly one generation, so its ramp is flat **at every spread**. Pinned
    /// rather than left implicit: it is a property of the figure, and an author
    /// reaching for `hue_spread` on such a preset needs the docs to have said so.
    #[test]
    fn a_grammar_without_branches_has_one_generation() {
        let (_, _, deepest) = figure("F+G-F-G+F");
        assert_eq!(deepest, 0, "no brackets, no second generation");

        let (segs, _) = coloured("F+G-F-G+F", 1.0);
        let first = segs.first().map(|s| s.color).unwrap_or([0.0; 3]);
        for seg in &segs {
            assert_eq!(
                seg.color, first,
                "one generation colours flat at any spread"
            );
        }
    }

    /// The reveal shortens the drawn figure; it must not shift the colours off
    /// their segments. `transform_cached` keeps a prefix, so segment `i` is
    /// still generation `generations[i]`.
    #[test]
    fn the_draw_progress_reveal_keeps_each_segment_on_its_own_generation() {
        let string = "F[+F]F[+F[-F]]F";
        let (full, generations) = coloured(string, 0.6);

        let (base, _, deepest) = figure(string);
        let mut colors = vec![[0.0; 3]; deepest as usize + 1];
        fill_depth_colors(
            &mut colors,
            &Palette::default_spectrum(),
            ColorRamp {
                hue: DEFAULT_HUE,
                hue_spread: 0.6,
                palette_mix: common::DEFAULT_PALETTE_MIX,
                palette_steps: crate::render::palette::DEFAULT_PALETTE_STEPS,
                saturation: common::DEFAULT_SATURATION,
                brightness: DEFAULT_BRIGHTNESS,
            },
            deepest,
        );
        // Half the figure, exactly as `transform_cached` reveals it.
        let mut half = Vec::new();
        transform_cached(&base, 0.0, 1.0, [0.0; 3], 0.01, 0.5, &mut half);
        apply_depth_colors(&mut half, &generations, &colors);

        assert!(!half.is_empty() && half.len() < full.len(), "a real prefix");
        for (i, seg) in half.iter().enumerate() {
            assert_eq!(
                seg.color, full[i].color,
                "revealed segment {i} must keep generation {}'s colour",
                generations[i]
            );
        }
    }

    /// [`FAMILY_PARAMS`] is held to the engine: each row names a declared
    /// parameter once, lists every mode — each turtle, fixed and endless, by
    /// the names a preset writes — and carries a spec range that is one of its
    /// modes'. The mode-dependent set is exactly the in-plane transform, the
    /// mirror, the opaque path and the camera block (ADR-0258), the fixed
    /// figure's depth and reveal, and the endless figure's rate and fade.
    #[test]
    fn the_turtle_table_is_the_roster_and_names_every_inert_parameter() {
        let turtles: Vec<&str> = TurtleMode::ALL.iter().map(|m| m.as_str()).collect();
        let expected: Vec<String> = Growth::ALL
            .iter()
            .flat_map(|g| {
                turtles.iter().map(move |t| match g {
                    Growth::Fixed => (*t).to_string(),
                    Growth::Endless => format!("{t} {}", g.as_str()),
                })
            })
            .collect();
        assert_eq!(MODES.to_vec(), expected, "the modes are the roster's");
        let modes = MODES.to_vec();
        let mut seen = Vec::new();
        for row in FAMILY_PARAMS {
            assert!(!seen.contains(&row.name), "`{}` has two rows", row.name);
            seen.push(row.name);
            let spec = PARAMS
                .iter()
                .find(|spec| spec.name == row.name)
                .unwrap_or_else(|| panic!("`{}` is not a declared parameter", row.name));
            let listed: Vec<&str> = row.ranges.iter().map(|r| r.family).collect();
            assert_eq!(listed, modes, "`{}` must list every mode", row.name);
            assert!(
                row.ranges.iter().any(|r| r.range == spec.range),
                "`{}`'s spec range is no mode's range",
                row.name
            );
        }
        let inert_on = |mode: &str| -> Vec<&str> {
            FAMILY_PARAMS
                .iter()
                .filter(|row| {
                    row.ranges
                        .iter()
                        .any(|r| r.family == mode && r.range.is_none())
                })
                .map(|row| row.name)
                .collect()
        };
        let camera: Vec<&str> = CameraParams::SPECS.iter().map(|s| s.name).collect();
        let plane = ["rotation", "scale", "stroke_blend"];
        let mirror = ["mirror_order", "mirror_reflect"];
        let fixed = ["visible_depth", "draw_progress"];
        let endless = ["grow", "tail"];
        fn joined(parts: &[&[&'static str]]) -> Vec<&'static str> {
            let mut all: Vec<&str> = parts.iter().flat_map(|p| p.iter().copied()).collect();
            all.sort_unstable();
            all
        }
        let sorted = |mut v: Vec<&'static str>| {
            v.sort_unstable();
            v
        };
        assert_eq!(
            sorted(inert_on("flat")),
            joined(&[&camera, &endless]),
            "flat"
        );
        assert_eq!(
            sorted(inert_on("space")),
            joined(&[&plane, &mirror, &endless]),
            "space"
        );
        assert_eq!(
            sorted(inert_on("flat endless")),
            joined(&[&camera, &mirror, &fixed]),
            "flat endless"
        );
        assert_eq!(
            sorted(inert_on("space endless")),
            joined(&[&plane, &mirror, &fixed]),
            "space endless"
        );
    }

    /// The vine every endless test below grows: a branching grammar whose
    /// stream outlasts any test by far.
    fn vine(trail: usize) -> Endless {
        let rules = [('F', "F[+F]F[-F]F".to_string())];
        Endless::new("F", &rules, 25f32.to_radians(), TurtleMode::Flat, trail)
    }

    /// Plan 0237 Phase 4's done-when: the figure grows by integrated time, not
    /// by frames — ten seconds at 60 Hz and at 144 Hz emit within one step of
    /// each other, and of `grow * 10`.
    #[test]
    fn the_endless_rate_is_the_same_at_any_display_rate() {
        let mut slow = vine(1000);
        for _ in 0..600 {
            slow.advance(20.0, 1.0 / 60.0);
        }
        let mut fast = vine(1000);
        for _ in 0..1440 {
            fast.advance(20.0, 1.0 / 144.0);
        }
        let (a, b) = (slow.emitted(), fast.emitted());
        assert!(a.abs_diff(b) <= 1, "60 Hz emitted {a}, 144 Hz {b}");
        assert!(
            a.abs_diff(200) <= 1,
            "ten seconds at 20 a second is 200, not {a}"
        );
    }

    /// Plan 0237 Phase 4's done-when: after 6,000 frames at `grow = 20` the
    /// ring holds exactly `trail` segments, and nothing the endless state
    /// holds has grown since it was built.
    #[test]
    fn the_ring_fills_to_its_trail_and_nothing_grows() {
        let mut endless = vine(1000);
        let built = endless.capacities();
        for _ in 0..6000 {
            endless.advance(20.0, 1.0 / 60.0);
        }
        assert_eq!(endless.emitted(), 2000, "100 seconds at 20 a second");
        assert_eq!(endless.len(), 1000, "the ring holds exactly its trail");
        assert_eq!(endless.capacities(), built, "a buffer grew");
        // Newest first, each segment begins where the pen stood: the ring is
        // in emission order, and the newest really is the last emitted.
        let newest = endless.at(0).map(|s| s.b);
        assert_eq!(newest, Some(endless.pen.position()));
    }

    /// A `grow` far past the ceiling emits the ceiling and drops its backlog:
    /// the frame after it emits what that frame owes, not the arrears.
    #[test]
    fn a_huge_grow_is_held_to_the_ceiling_without_a_backlog() {
        let mut endless = vine(4000);
        endless.advance(1.0e9, 1.0 / 60.0);
        assert_eq!(endless.emitted(), EMIT_CEILING);
        endless.advance(60.0, 1.0 / 60.0);
        assert_eq!(
            endless.emitted(),
            EMIT_CEILING + 1,
            "one step owed, one taken"
        );
        endless.advance(f32::NAN, 1.0 / 60.0);
        endless.advance(-5.0, 1.0 / 60.0);
        assert_eq!(
            endless.emitted(),
            EMIT_CEILING + 1,
            "nothing grows backwards"
        );
    }

    /// A stream that runs out starts again from the axiom with the pen where
    /// it stood: the ring keeps growing, and the step after the restart
    /// begins where the last one ended.
    #[test]
    fn a_short_stream_restarts_without_a_jump() {
        // Two draw steps, then the end: depth 32 of a rule that never grows.
        let rules = [('F', "F+".to_string())];
        let mut endless = Endless::new("FF", &rules, 0.3, TurtleMode::Flat, 50);
        for _ in 0..120 {
            endless.advance(20.0, 1.0 / 60.0);
        }
        assert_eq!(endless.emitted(), 40);
        for age in 0..endless.len() - 1 {
            let (newer, older) = (endless.at(age), endless.at(age + 1));
            assert_eq!(
                newer.map(|s| s.a),
                older.map(|s| s.b),
                "segment {age} does not begin where the one before it ended"
            );
        }
    }

    /// The fade is full light until the oldest `tail` of the ring, then falls
    /// to nothing at the ring's end; `tail = 0` fades nothing.
    #[test]
    fn the_fade_falls_over_the_oldest_tail() {
        assert_eq!(fade(0, 100, 0.5), 1.0);
        assert_eq!(fade(49, 100, 0.5), 1.0);
        assert!((fade(75, 100, 0.5) - 0.5).abs() < 1e-6);
        assert!(fade(99, 100, 0.5) <= 0.02 + 1e-6);
        assert_eq!(fade(99, 100, 0.0), 1.0);
        assert_eq!(fade(99, 100, f32::NAN), 1.0);
    }

    /// The scene owns every buffer it draws from at its construction size:
    /// 6,000 frames of an endless figure in each turtle, rendered as it goes,
    /// grow none of them.
    #[test]
    fn an_endless_scene_grows_no_buffer_it_owns() {
        use crate::render::context::{RenderContext, RenderError};

        let ctx = match RenderContext::new_headless(64, 64, true) {
            Ok(ctx) => ctx,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return;
            }
            Err(e) => panic!("headless context build failed: {e}"),
        };
        let tier = crate::render::TierConfig::FLOOR;
        let format = crate::render::COMPOSITE_FORMAT;
        let shared = Rc::new(RefCell::new(LineRenderer::new(
            &ctx.device,
            format,
            tier.max_segments,
            "lsystem-test",
        )));
        let target = ctx
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("lsystem-endless-target"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        for turtle in TurtleMode::ALL {
            let mut scene = LSystemScene::new(
                Rc::clone(&shared),
                tier.max_segments,
                &ctx.device,
                format,
                tier.seg3d_segments as usize,
                tier.max_coc_px as f32,
            );
            scene.set_target_size(64, 64);
            let overflow = scene.configure(&GeneratorConfig::LSystem {
                axiom: "F".into(),
                rules: vec![('F', "F[+F]F[-F]F".into())],
                angle_deg: 25.0,
                max_depth: 4,
                seed: 0,
                turtle,
                growth: Growth::Endless,
                trail: 1000,
            });
            assert_eq!(overflow, None, "{turtle:?}: a trail inside the cap");
            let capacities = |s: &LSystemScene| {
                (
                    [
                        s.draw_buf.capacity(),
                        s.single_buf.capacity(),
                        s.instances3d.capacity(),
                        s.depth_colors.capacity(),
                    ],
                    s.endless.as_ref().map(Endless::capacities),
                )
            };
            let built = capacities(&scene);
            let frame = AnalysisFrame::default();
            for i in 0..6000 {
                scene.reset_params();
                scene.set_param("grow", 20.0);
                scene.advance(1.0 / 60.0);
                scene.update(&frame);
                if i % 500 == 0 || i == 5999 {
                    let mut encoder = ctx
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                    scene.render(&ctx.queue, &mut encoder, &target, 1.0);
                    ctx.queue.submit([encoder.finish()]);
                }
            }
            assert_eq!(
                scene.endless.as_ref().map(Endless::len),
                Some(1000),
                "{turtle:?}: the ring holds exactly its trail"
            );
            if turtle == TurtleMode::Flat {
                assert_eq!(scene.draw_buf.len(), 1000, "the whole ring is drawn");
            }
            assert_eq!(capacities(&scene), built, "{turtle:?}: a buffer grew");
        }
    }

    #[test]
    fn draw_progress_reveals_a_prefix() {
        let mut base = Vec::with_capacity(64);
        turtle::walk("FFFFFFFF", 0.0, CAP, &mut base);
        let mut out = Vec::with_capacity(64);
        transform_cached(&base, 0.0, 1.0, [1.0; 3], 0.01, 0.5, &mut out);
        assert_eq!(out.len(), 4, "half of eight segments");
    }
}

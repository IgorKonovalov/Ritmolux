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
use super::renderer::{LineRenderer, SegmentInstance};
use super::{
    CapOverflow, ColorRamp, GeneratorConfig, MAX_LSYSTEM_DEPTH, MirrorSpec, OverflowContext,
    ViewTransform, grammar, replicate_mirror, transform_cached, turtle,
};
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

const DEFAULT_VISIBLE_DEPTH: f32 = 1.0;
const DEFAULT_ROTATION: f32 = 0.0;
const DEFAULT_HUE: f32 = 0.3;
/// Colour surface (ADR-0021 / ADR-0059), all three at the value that reproduces
/// the single flat `hue` this scene drew before the palette reached it: no ramp
/// along the depth axis, palette A alone, unmodified saturation.
const DEFAULT_HUE_SPREAD: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
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
const DEFAULT_PAN: f32 = 0.0;
// Geometry mirror (Phase 4): identity by default.
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;

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
    hue: f32,
    hue_spread: f32,
    saturation: f32,
    palette_mix: f32,
    /// Hard palette bands and their contour (ADR-0078), raw as the preset
    /// bound them -- `palette::band_steps` / `band_contour` condition them on
    /// the way to the sample site.
    palette_steps: f32,
    palette_contour: f32,
    draw_progress: f32,
    thickness: f32,
    scale: f32,
    brightness: f32,
    glow: f32,
    softness: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    mirror_order: f32,
    mirror_reflect: f32,
}

impl LSystemScene {
    /// Build the scene over the shared line renderer, preallocating the draw
    /// buffer. No grammar is expanded until a preset configures one.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>, max_segments: usize) -> Self {
        Self {
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
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            draw_progress: DEFAULT_DRAW_PROGRESS,
            thickness: DEFAULT_THICKNESS,
            scale: DEFAULT_SCALE,
            brightness: DEFAULT_BRIGHTNESS,
            glow: DEFAULT_GLOW,
            softness: super::DEFAULT_SOFTNESS,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }

    /// Expand + turtle-walk each depth `1..=max_depth` into a cached buffer.
    /// Off the hot path (called from `configure`).
    fn build(&mut self, axiom: &str, rules: &[(char, String)], angle_deg: f32, max_depth: u32) {
        self.cached.clear();
        self.cached_depths.clear();
        self.cached_max_depth.clear();
        self.overflow = None;
        let depth = max_depth.clamp(1, MAX_LSYSTEM_DEPTH);
        let angle = angle_deg.to_radians();

        for d in 1..=depth {
            let string = grammar::expand(axiom, rules, d);
            let mut segs = Vec::new();
            let mut generations = Vec::new();
            let dropped = turtle::walk_with_depths(
                &string,
                angle,
                self.max_segments,
                &mut segs,
                &mut generations,
            );
            turtle::normalize_fit(&mut segs, 0.9);
            if dropped > 0 && self.overflow.is_none() {
                self.overflow = Some((d, dropped));
            }
            self.cached_max_depth
                .push(generations.iter().copied().max().unwrap_or(0));
            self.cached.push(segs);
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
pub const PARAMS: &[&str] = &[
    "visible_depth",
    "rotation",
    "hue",
    "hue_spread",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "draw_progress",
    "thickness",
    "scale",
    "brightness",
    "glow",
    "softness",
    "zoom",
    "pan_x",
    "pan_y",
    "mirror_order",
    "mirror_reflect",
];

impl Scene for LSystemScene {
    fn name(&self) -> &'static str {
        "l-system"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn reset_params(&mut self) {
        self.visible_depth = DEFAULT_VISIBLE_DEPTH;
        self.rotation = DEFAULT_ROTATION;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.draw_progress = DEFAULT_DRAW_PROGRESS;
        self.thickness = DEFAULT_THICKNESS;
        self.scale = DEFAULT_SCALE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.glow = DEFAULT_GLOW;
        self.softness = super::DEFAULT_SOFTNESS;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "visible_depth" => self.visible_depth = value,
            "rotation" => self.rotation = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "draw_progress" => self.draw_progress = value,
            "thickness" => self.thickness = value,
            "scale" => self.scale = value,
            "brightness" => self.brightness = value,
            "glow" => self.glow = value,
            "softness" => self.softness = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            _ => {}
        }
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // Build + cache the grammar's geometry off the hot path. Other config
        // variants belong to sibling line scenes and are ignored.
        match cfg {
            GeneratorConfig::LSystem {
                axiom,
                rules,
                angle_deg,
                max_depth,
                seed: _,
            } => self.build(axiom, rules, *angle_deg, *max_depth),
            GeneratorConfig::Curve { .. }
            | GeneratorConfig::Star { .. }
            | GeneratorConfig::Particles { .. }
            | GeneratorConfig::Spectrum { .. }
            | GeneratorConfig::WarpMesh { .. } => {}
        }
        // Surface a cap truncation so the frontend can report it — never a
        // silent cut (ADR-0007). `None` when every depth fit (the norm).
        self.overflow.map(|(depth, dropped)| CapOverflow {
            dropped,
            context: OverflowContext::Depth(depth),
            cap: self.max_segments,
        })
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Pick the visible depth (1-based) and its cached base geometry.
        let depths = self.cached.len();
        if depths == 0 {
            self.draw_buf.clear();
            return;
        }
        let want = self.visible_depth.max(1.0) as usize;
        let idx = want.min(depths).saturating_sub(1);
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
                hue: self.hue,
                hue_spread: self.hue_spread,
                palette_mix: self.palette_mix,
                palette_steps: self.palette_steps,
                saturation: self.saturation,
                brightness: self.brightness,
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
        let xform = ViewTransform {
            zoom: self.zoom,
            pan: [self.pan_x, self.pan_y],
            _pad: 0.0,
        };
        self.renderer.borrow_mut().draw(
            queue,
            encoder,
            view,
            aspect,
            self.glow,
            self.softness,
            xform,
            &self.draw_buf,
        );
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
                palette_mix: DEFAULT_PALETTE_MIX,
                palette_steps: palette::DEFAULT_PALETTE_STEPS,
                saturation: DEFAULT_SATURATION,
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
                palette_mix: DEFAULT_PALETTE_MIX,
                palette_steps: palette::DEFAULT_PALETTE_STEPS,
                saturation: DEFAULT_SATURATION,
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

    #[test]
    fn draw_progress_reveals_a_prefix() {
        let mut base = Vec::with_capacity(64);
        turtle::walk("FFFFFFFF", 0.0, CAP, &mut base);
        let mut out = Vec::with_capacity(64);
        transform_cached(&base, 0.0, 1.0, [1.0; 3], 0.01, 0.5, &mut out);
        assert_eq!(out.len(), 4, "half of eight segments");
    }
}

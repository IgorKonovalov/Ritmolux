//! Turtle interpretation: walk an L-system string into line segments, with a
//! branch stack for `[`/`]`. A build-time step (runs inside `Scene::configure`,
//! off the hot path) that produces the base geometry a generator scene caches
//! and then only transforms per frame.
//!
//! Commands (the common turtle vocabulary):
//! - `F`, `G` — step forward, drawing a segment
//! - `f`      — step forward without drawing
//! - `+`      — turn left by the configured angle
//! - `-`      — turn right by the configured angle
//! - `[`      — push position + heading
//! - `]`      — pop position + heading
//! - anything else — no-op (grammar variables such as `X` that only expand)

// Under render/, so it carries the panic pragma even though it runs only at
// preset load. Written panic-free (no unwrap/index/panic).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::f32::consts::FRAC_PI_2;

use super::PLACEHOLDER_WIDTH;
use super::renderer::{Segment3dInstance, SegmentInstance, miter_extension};

/// Which turtle walks the grammar: `[generator] turtle` (ADR-0258).
///
/// `Flat` is the plane walk above, and the default, so a preset that does not
/// name a turtle walks exactly what it always did. `Space` carries a full
/// orientation frame and reads the five space symbols; in `Flat` they stay
/// inert grammar variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurtleMode {
    /// The plane turtle: `x`, `y` and one heading.
    #[default]
    Flat,
    /// The space turtle: a position and a heading, left and up frame.
    Space,
}

impl TurtleMode {
    /// Every mode, in the order the loader names them.
    pub const ALL: [TurtleMode; 2] = [TurtleMode::Flat, TurtleMode::Space];

    /// The mode as a preset writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            TurtleMode::Flat => "flat",
            TurtleMode::Space => "space",
        }
    }

    /// The mode a preset's spelling names, or `None` for an unknown one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == name)
    }
}

/// The space turtle's state: a position and an orientation frame of three unit
/// vectors, heading `H`, left `L` and up `U`, with `U = H x L`.
///
/// It starts at the origin heading world `+y` with left `-x` and up `+z`, so a
/// grammar that only yaws draws in the `xy` plane facing a camera at yaw `0`,
/// and turns the same way round as the flat turtle does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turtle3d {
    /// Where the pen is.
    pub pos: [f32; 3],
    /// `H`: the direction a step moves.
    pub heading: [f32; 3],
    /// `L`: the direction a `+` turns toward.
    pub left: [f32; 3],
    /// `U`: `H x L`.
    pub up: [f32; 3],
}

impl Default for Turtle3d {
    fn default() -> Self {
        Self {
            pos: [0.0; 3],
            heading: [0.0, 1.0, 0.0],
            left: [-1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
        }
    }
}

impl Turtle3d {
    /// Apply one turn symbol by `angle` radians, and say whether `ch` was one.
    ///
    /// - `+` and `-` yaw about `U`, turning `H` toward and away from `L`;
    /// - `&` and `^` pitch about `L`, turning `H` away from and toward `U`;
    /// - `\` and `/` roll about `H`, turning `L` toward and away from `U`;
    /// - `|` yaws half a turn.
    pub fn turn(&mut self, ch: char, angle: f32) -> bool {
        match ch {
            '+' => self.yaw(angle),
            '-' => self.yaw(-angle),
            '&' => self.pitch(angle),
            '^' => self.pitch(-angle),
            '\\' => self.roll(angle),
            '/' => self.roll(-angle),
            '|' => {
                // Exact rather than `yaw(PI)`, whose `sin(PI)` is not zero.
                self.heading = neg(self.heading);
                self.left = neg(self.left);
            }
            _ => return false,
        }
        true
    }

    /// One step along `H`.
    pub fn step(&mut self) {
        self.pos = add(self.pos, self.heading);
    }

    fn yaw(&mut self, angle: f32) {
        let (s, c) = angle.sin_cos();
        let (h, l) = (self.heading, self.left);
        self.heading = mix(h, c, l, s);
        self.left = mix(l, c, h, -s);
    }

    fn pitch(&mut self, angle: f32) {
        let (s, c) = angle.sin_cos();
        let (h, u) = (self.heading, self.up);
        self.heading = mix(h, c, u, -s);
        self.up = mix(u, c, h, s);
    }

    fn roll(&mut self, angle: f32) {
        let (s, c) = angle.sin_cos();
        let (l, u) = (self.left, self.up);
        self.left = mix(l, c, u, s);
        self.up = mix(u, c, l, -s);
    }
}

/// `a * ca + b * cb`.
fn mix(a: [f32; 3], ca: f32, b: [f32; 3], cb: f32) -> [f32; 3] {
    [
        a[0] * ca + b[0] * cb,
        a[1] * ca + b[1] * cb,
        a[2] * ca + b[2] * cb,
    ]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn neg(a: [f32; 3]) -> [f32; 3] {
    [-a[0], -a[1], -a[2]]
}

/// [`walk_with_depths`] for the space turtle: `s` walked into 3D segments in
/// `out`, with each segment's generation depth in `depths`, index-aligned the
/// same way and under the same cap. Both are cleared first; the returned
/// `usize` is how many draw steps the cap dropped.
///
/// A segment that continues the pen's run carries the run's neighbouring
/// points in [`prev`](Segment3dInstance::prev) and
/// [`next`](Segment3dInstance::next), so the two meet on one mitred corner; a
/// free end carries its own endpoint there, which is how the `seg3d` pipeline
/// reads "free". The run breaks where the flat walk's does: at `f`, `[`, `]`
/// and a segment lost to the cap. Colour, width and alpha are placeholders
/// the scene fills per frame.
pub fn walk_3d_with_depths(
    s: &str,
    angle: f32,
    max_segments: usize,
    out: &mut Vec<Segment3dInstance>,
    depths: &mut Vec<u32>,
) -> usize {
    out.clear();
    depths.clear();

    let mut turtle = Turtle3d::default();
    let mut stack: Vec<Turtle3d> = Vec::new();
    let mut dropped = 0usize;
    let mut run: Option<usize> = None;

    for ch in s.chars() {
        match ch {
            'F' | 'G' => {
                let a = turtle.pos;
                turtle.step();
                let b = turtle.pos;
                if out.len() < max_segments {
                    let mut prev = a;
                    if let Some(before) = run.and_then(|i| out.get_mut(i)) {
                        before.next = b;
                        prev = before.a;
                    }
                    run = Some(out.len());
                    depths.push(stack.len() as u32);
                    out.push(Segment3dInstance {
                        a,
                        b,
                        color: [1.0, 1.0, 1.0],
                        width: 1.0,
                        alpha: 1.0,
                        prev,
                        next: b,
                        skirt: 0.0,
                    });
                } else {
                    dropped += 1;
                    run = None;
                }
            }
            'f' => {
                turtle.step();
                run = None;
            }
            '[' => {
                stack.push(turtle);
                run = None;
            }
            ']' => {
                if let Some(saved) = stack.pop() {
                    turtle = saved;
                }
                run = None;
            }
            // A turn keeps the pen on the paper, so it does not break the run.
            _ => {
                turtle.turn(ch, angle);
            }
        }
    }
    dropped
}

/// Centre `segs` on their bounding box's centre and scale them uniformly so
/// every endpoint lies within a sphere of radius `radius` about the origin,
/// and return the sphere's radius after the fit — `radius`, or `0` for an
/// empty or degenerate set, which is left untouched.
///
/// A sphere rather than a box because the figure turns under a camera: a
/// sphere's extent is the same from every side, so the fit does not depend on
/// the view. The run neighbours in `prev` and `next` move with the endpoints.
pub fn sphere_fit(segs: &mut [Segment3dInstance], radius: f32) -> f32 {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for seg in segs.iter() {
        for p in [seg.a, seg.b] {
            for ((lo, hi), v) in min.iter_mut().zip(max.iter_mut()).zip(p) {
                *lo = lo.min(v);
                *hi = hi.max(v);
            }
        }
    }
    let centre = [
        0.5 * (min[0] + max[0]),
        0.5 * (min[1] + max[1]),
        0.5 * (min[2] + max[2]),
    ];
    let mut reach = 0.0f32;
    for seg in segs.iter() {
        for p in [seg.a, seg.b] {
            let d = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
            reach = reach.max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        }
    }
    if !reach.is_finite() || reach <= f32::EPSILON {
        return 0.0;
    }
    let scale = radius / reach;
    let fit = |p: [f32; 3]| {
        [
            (p[0] - centre[0]) * scale,
            (p[1] - centre[1]) * scale,
            (p[2] - centre[2]) * scale,
        ]
    };
    for seg in segs.iter_mut() {
        seg.a = fit(seg.a);
        seg.b = fit(seg.b);
        seg.prev = fit(seg.prev);
        seg.next = fit(seg.next);
    }
    radius
}

/// Walk `s` into `out` (cleared first) as base geometry — positions only; the
/// scene fills colour/width per frame. `angle` is in radians. Segments beyond
/// `max_segments` are dropped and counted (the ADR-0007 cap is never silent):
/// the returned `usize` is how many draw steps were dropped.
pub fn walk(s: &str, angle: f32, max_segments: usize, out: &mut Vec<SegmentInstance>) -> usize {
    // The depth side-channel is build-time scratch the caller did not ask for.
    let mut depths = Vec::new();
    walk_with_depths(s, angle, max_segments, out, &mut depths)
}

/// [`walk`], plus the **generation depth** of every emitted segment written into
/// `depths` (cleared first) — the branch-nesting level the turtle drew it at
/// (ADR-0059's `lsystem` colour axis). Depth `0` is the trunk; each unclosed `[`
/// is one more generation, so a segment's depth is how many branch pushes are
/// still open above it.
///
/// **`depths` is index-aligned with `out` by construction**, which is the whole
/// reason it is produced here rather than by a second pass over the string: both
/// are pushed in the same branch, under the same cap, so a segment dropped at the
/// cap drops its depth with it. A separate scanner would have to re-derive which
/// characters draw, and would silently desynchronise the moment the turtle's
/// vocabulary changed.
pub fn walk_with_depths(
    s: &str,
    angle: f32,
    max_segments: usize,
    out: &mut Vec<SegmentInstance>,
    depths: &mut Vec<u32>,
) -> usize {
    out.clear();
    depths.clear();

    // Start at the origin pointing up; the whole figure is fit-normalized after.
    let step = 1.0_f32;
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    let mut heading = FRAC_PI_2;
    let mut stack: Vec<(f32, f32, f32)> = Vec::new();
    let mut dropped = 0usize;
    // Index of the segment the pen is currently continuing from, or `None` when
    // the run is broken (ADR-0041). This is what a join flag has to be true of:
    // the next drawn segment starts exactly where that one ended.
    let mut run: Option<usize> = None;

    for ch in s.chars() {
        match ch {
            'F' | 'G' => {
                let (dy, dx) = heading.sin_cos();
                let nx = x + dx * step;
                let ny = y + dy * step;
                if out.len() < max_segments {
                    // One joint, extended from both sides. A turn does not break
                    // the run — `+`/`-` only change heading — which is why the
                    // state is a run rather than a look at the previous char.
                    //
                    // The extension is in units of the placeholder `width`
                    // below; `LineInstance::styled` rescales it to whatever
                    // half-width the frame is drawn at.
                    let mut ext_a = 0.0;
                    if let Some(prev) = run.and_then(|i| out.get_mut(i)) {
                        // The turn `+`/`-` made between the two draws IS the
                        // joint's interior angle, and both sides of a joint
                        // reach the same corner point, so one length serves the
                        // pair.
                        let ext = miter_extension(PLACEHOLDER_WIDTH, prev.a, prev.b, [nx, ny]);
                        prev.ext_b = ext;
                        ext_a = ext;
                    }
                    run = Some(out.len());
                    // Generation depth = how many branch pushes are still open.
                    depths.push(stack.len() as u32);
                    out.push(SegmentInstance {
                        a: [x, y],
                        b: [nx, ny],
                        color: [1.0, 1.0, 1.0],
                        width: PLACEHOLDER_WIDTH,
                        alpha: 1.0,
                        ext_a,
                        ext_b: 0.0,
                    });
                } else {
                    dropped += 1;
                    // Nothing can join to a segment that was never emitted.
                    run = None;
                }
                x = nx;
                y = ny;
            }
            'f' => {
                let (dy, dx) = heading.sin_cos();
                x += dx * step;
                y += dy * step;
                // The pen moved without drawing, so the next segment starts
                // somewhere the last one does not reach.
                run = None;
            }
            '+' => heading += angle,
            '-' => heading -= angle,
            '[' => {
                stack.push((x, y, heading));
                // A branch start is not a continuation of the segment before it;
                // flagging it would extend that stroke backward along the
                // branch's own direction, into space it never covered.
                run = None;
            }
            ']' => {
                if let Some((px, py, ph)) = stack.pop() {
                    x = px;
                    y = py;
                    heading = ph;
                }
                run = None;
            }
            _ => {}
        }
    }
    dropped
}

/// Center `segs` and uniformly scale them to fit within `[-target, target]` on
/// the larger axis, so any figure (whatever its raw extent per depth) frames
/// itself in the view. A degenerate (zero-extent) set is left untouched.
pub fn normalize_fit(segs: &mut [SegmentInstance], target: f32) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for seg in segs.iter() {
        for p in [seg.a, seg.b] {
            min_x = min_x.min(p[0]);
            min_y = min_y.min(p[1]);
            max_x = max_x.max(p[0]);
            max_y = max_y.max(p[1]);
        }
    }
    let extent = (max_x - min_x).max(max_y - min_y);
    if !extent.is_finite() || extent <= f32::EPSILON {
        return;
    }
    let cx = 0.5 * (min_x + max_x);
    let cy = 0.5 * (min_y + max_y);
    let scale = 2.0 * target / extent;
    for seg in segs.iter_mut() {
        seg.a = [(seg.a[0] - cx) * scale, (seg.a[1] - cy) * scale];
        seg.b = [(seg.b[0] - cx) * scale, (seg.b[1] - cy) * scale];
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn walk_produces_one_segment_per_draw_step() {
        let mut out = Vec::with_capacity(16);
        // A closed square: four forward steps turning 90 degrees.
        walk("F+F+F+F", std::f32::consts::FRAC_PI_2, 100, &mut out);
        assert_eq!(out.len(), 4, "four F steps -> four segments");

        // A branch: the bracketed F is a third segment; `]` restores state so
        // the trailing F continues from the branch point.
        out.clear();
        walk("F[+F]F", std::f32::consts::FRAC_PI_2, 100, &mut out);
        assert_eq!(out.len(), 3, "trunk + branch + trunk");
    }

    /// The turtle is the tricky producer: it is a chain, but the chain
    /// **breaks** every time the pen stops continuing from where it was — at a
    /// branch push or pop, and at a move-without-draw. Asserted on the extension
    /// pattern rather than on pixels (ADR-0158); the lengths are in
    /// [`PLACEHOLDER_WIDTH`], the units a cached walk stores them in, and
    /// `LineInstance::styled` rescales them per frame.
    ///
    /// **A straight joint carries exactly the flat half-width**, which is what
    /// makes `F F` and `F + F` different assertions rather than one: the
    /// straight run is the miter's `theta = pi` case and the right-angle turn is
    /// `theta = pi / 2`, so `1 / sin(pi / 4) = sqrt(2)`.
    #[test]
    fn the_turtle_joins_within_a_run_and_breaks_at_a_branch() {
        use crate::render::scenes::lines::{MITER_SLACK, expected_miter};

        const W: f32 = PLACEHOLDER_WIDTH;
        let mut out = Vec::with_capacity(16);
        // Trunk of two, a one-segment branch, then a trunk of two more. `[+F]`
        // turns before drawing, so the two trunk runs are collinear and every
        // joint here is straight — the miter is exactly the flat half-width.
        walk("FF[+F]FF", FRAC_PI_2, 100, &mut out);
        assert_eq!(out.len(), 5, "two trunk, one branch, two trunk");
        assert_eq!(
            out.iter().map(|s| (s.ext_a, s.ext_b)).collect::<Vec<_>>(),
            vec![(0.0, W), (W, 0.0), (0.0, 0.0), (0.0, W), (W, 0.0)],
            "joined inside each run, free on both sides of the branch; a \
             straight joint's miter IS the flat half-width"
        );
        // The branch segment starts at the same point the first run ended, and
        // that is exactly the case the extension must *not* claim: it is a new
        // stroke, not a continuation, so extending it backward would run along
        // the branch's own direction into space it never covered.
        assert_eq!(
            out[1].b, out[2].a,
            "the branch does start at the trunk's end"
        );
        assert_eq!(
            (out[2].ext_a, out[2].ext_b),
            (0.0, 0.0),
            "and is still free at both ends"
        );

        // A turn is not a break — that is the whole reason the walk tracks a run
        // rather than looking at the previous character — and the turn IS the
        // joint's interior angle. A right angle needs `1 / sin(pi / 4)`.
        out.clear();
        walk("F+F", FRAC_PI_2, 100, &mut out);
        let want = expected_miter(W, out[0].a, out[0].b, out[1].b);
        assert!(
            (want - W * std::f32::consts::SQRT_2).abs() <= W * MITER_SLACK,
            "the reference itself: a right-angle joint is sqrt(2) half-widths, \
             got {want}"
        );
        assert!(
            out[0].ext_a == 0.0
                && out[1].ext_b == 0.0
                && (out[0].ext_b - want).abs() <= want * MITER_SLACK
                && (out[1].ext_a - want).abs() <= want * MITER_SLACK,
            "a turn keeps the pen on the paper, and the joint reaches the \
             corner the turn makes: got {:?} against {want} at the joint",
            out.iter().map(|s| (s.ext_a, s.ext_b)).collect::<Vec<_>>()
        );

        // A gentler turn is a longer reach, which is the whole property: the
        // extension has to track the angle rather than being a constant with an
        // angle-shaped comment.
        out.clear();
        walk("F+F", FRAC_PI_2 / 3.0, 100, &mut out);
        let gentle = expected_miter(W, out[0].a, out[0].b, out[1].b);
        assert!(
            gentle < want && (out[0].ext_b - gentle).abs() <= gentle * MITER_SLACK,
            "a 30-degree turn is a shallower corner than a right angle, so it \
             reaches {gentle} against the right angle's {want}; got {}",
            out[0].ext_b
        );

        // A move-without-draw is: the pen teleports, so the next segment starts
        // somewhere the last one never reached.
        out.clear();
        walk("FfF", 0.0, 100, &mut out);
        assert_eq!(
            out.iter().map(|s| (s.ext_a, s.ext_b)).collect::<Vec<_>>(),
            vec![(0.0, 0.0), (0.0, 0.0)],
            "`f` breaks the run"
        );
        assert_ne!(out[0].b, out[1].a, "and the two really are disjoint");

        // A segment lost to the cap cannot be joined to, either.
        out.clear();
        let dropped = walk("FFFF", 0.0, 2, &mut out);
        assert_eq!((out.len(), dropped), (2, 2));
        assert_eq!(
            out.iter().map(|s| (s.ext_a, s.ext_b)).collect::<Vec<_>>(),
            vec![(0.0, W), (W, 0.0)],
            "the kept prefix keeps its own joint and claims none past the cap"
        );
    }

    /// Plan 0054 Phase 1 (ADR-0059). The `lsystem` colour axis is **generation
    /// depth**, not traversal order, and this is where the two are told apart:
    /// the depth channel has to say "this segment is on a second-generation
    /// branch" for segments that are far apart in the walk.
    #[test]
    fn the_depth_channel_reports_branch_generation_not_traversal_order() {
        let mut out = Vec::new();
        let mut depths = Vec::new();

        // Trunk, a branch, more trunk, a second branch carrying a sub-branch.
        walk_with_depths("F[+F]F[+F[-F]]F", FRAC_PI_2, 100, &mut out, &mut depths);
        assert_eq!(out.len(), 6, "three trunk, two branch, one sub-branch");
        assert_eq!(
            depths,
            vec![0, 1, 0, 1, 2, 0],
            "depth counts open branch pushes, so the two first-generation \
             branches share a depth despite sitting at opposite ends of the walk"
        );

        // A grammar with no branches has exactly one generation — a real
        // property of such a figure (the Sierpinski arrowhead is one), not a
        // defect: every segment of it sits at the same recursion level.
        out.clear();
        depths.clear();
        walk_with_depths("F+F-F+F", FRAC_PI_2, 100, &mut out, &mut depths);
        assert_eq!(depths, vec![0; 4], "no brackets, one generation");

        // The two channels stay index-aligned through the cap: a segment that
        // was never emitted contributes no depth either.
        out.clear();
        depths.clear();
        let dropped = walk_with_depths("F[+FFFF]F", FRAC_PI_2, 3, &mut out, &mut depths);
        assert_eq!((out.len(), depths.len(), dropped), (3, 3, 3));
        assert_eq!(depths, vec![0, 1, 1]);
    }

    #[test]
    fn walk_is_deterministic_for_a_fixed_structure() {
        let mut a = Vec::with_capacity(64);
        let mut b = Vec::with_capacity(64);
        let s = "FF+F[-F]+FF";
        walk(s, 0.4, 100, &mut a);
        walk(s, 0.4, 100, &mut b);
        assert_eq!(a, b, "same string + angle -> identical geometry");
    }

    #[test]
    fn the_segment_cap_truncates_and_reports_the_drop() {
        let mut out = Vec::with_capacity(8);
        // Ten draw steps, but a cap of 3: seven are dropped and counted.
        let dropped = walk("FFFFFFFFFF", 0.0, 3, &mut out);
        assert_eq!(out.len(), 3, "only the cap is kept");
        assert_eq!(dropped, 7, "the overflow is counted, never silent");
    }

    /// Plan 0237 Phase 1: `&` is a pitch in space and an inert variable on
    /// the plane. Four pitched steps at a right angle close a square standing
    /// in the vertical `yz` plane; the same string walked flat is four steps
    /// up one line.
    #[test]
    fn a_pitched_square_stands_up_in_space_and_lies_straight_on_the_plane() {
        let mut space = Vec::new();
        let mut depths = Vec::new();
        walk_3d_with_depths("F&F&F&F", FRAC_PI_2, 100, &mut space, &mut depths);
        assert_eq!(space.len(), 4, "four draw steps");
        let close = |p: [f32; 3], q: [f32; 3]| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-6);
        let corners = [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, -1.0],
            [0.0, 0.0, -1.0],
        ];
        for (k, seg) in space.iter().enumerate() {
            assert!(
                close(seg.a, corners[k]) && close(seg.b, corners[(k + 1) % 4]),
                "side {k} runs {:?} -> {:?}, not along the square",
                seg.a,
                seg.b
            );
            assert!(
                seg.a[0].abs() < 1e-6 && seg.b[0].abs() < 1e-6,
                "side {k} leaves the vertical yz plane"
            );
        }
        assert!(
            close(space[3].b, space[0].a),
            "the fourth side ends where the first began"
        );

        let mut flat = Vec::new();
        walk("F&F&F&F", FRAC_PI_2, 100, &mut flat);
        assert_eq!(flat.len(), 4, "four draw steps");
        for (k, seg) in flat.iter().enumerate() {
            let y = k as f32;
            assert!(
                seg.a[0].abs() < 1e-6
                    && seg.b[0].abs() < 1e-6
                    && (seg.a[1] - y).abs() < 1e-5
                    && (seg.b[1] - (y + 1.0)).abs() < 1e-5,
                "flat step {k} runs {:?} -> {:?}, off the straight line up",
                seg.a,
                seg.b
            );
        }
    }

    /// The space walk is the flat walk where it only yaws: a grammar of `+`
    /// and `-` draws the same figure in the `xy` plane, turning the same way.
    #[test]
    fn a_yaw_only_grammar_walks_the_flat_figure_in_the_xy_plane() {
        let s = "F+F-F[+F]F";
        let mut flat = Vec::new();
        walk(s, 0.4, 100, &mut flat);
        let mut space = Vec::new();
        let mut depths = Vec::new();
        walk_3d_with_depths(s, 0.4, 100, &mut space, &mut depths);
        assert_eq!(flat.len(), space.len());
        for (f, p) in flat.iter().zip(&space) {
            for (a, b) in [(f.a, p.a), (f.b, p.b)] {
                assert!(
                    (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5 && b[2].abs() < 1e-6,
                    "flat {a:?} against space {b:?}"
                );
            }
        }
    }

    /// The space walk's run carries the flat walk's breaks: a joined end names
    /// its neighbour's far point, a free end names itself.
    #[test]
    fn the_space_walk_joins_within_a_run_and_breaks_at_a_branch() {
        let mut out = Vec::new();
        let mut depths = Vec::new();
        walk_3d_with_depths("FF[&F]F", FRAC_PI_2, 100, &mut out, &mut depths);
        assert_eq!(out.len(), 4);
        assert_eq!(depths, vec![0, 0, 1, 0]);
        let free_a = |s: &Segment3dInstance| s.prev == s.a;
        let free_b = |s: &Segment3dInstance| s.next == s.b;
        assert!(free_a(&out[0]) && !free_b(&out[0]) && out[0].next == out[1].b);
        assert!(!free_a(&out[1]) && out[1].prev == out[0].a && free_b(&out[1]));
        assert!(
            free_a(&out[2]) && free_b(&out[2]),
            "the branch stands alone"
        );
        assert!(free_a(&out[3]) && free_b(&out[3]), "a pop breaks the run");

        // The cap drops and counts, and keeps the two channels aligned.
        let dropped = walk_3d_with_depths("F[&FFF]F", FRAC_PI_2, 2, &mut out, &mut depths);
        assert_eq!((out.len(), depths.len(), dropped), (2, 2, 3));
    }

    /// The sphere fit centres the figure and holds every endpoint inside the
    /// asked radius, with at least one on it; neighbours move with endpoints.
    #[test]
    fn the_sphere_fit_holds_the_figure_inside_its_radius() {
        let mut out = Vec::new();
        let mut depths = Vec::new();
        walk_3d_with_depths("FF&F/F^F", 0.7, 100, &mut out, &mut depths);
        let radius = sphere_fit(&mut out, 1.0);
        assert_eq!(radius, 1.0);
        let len = |p: [f32; 3]| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let reach = out
            .iter()
            .flat_map(|s| [len(s.a), len(s.b)])
            .fold(0.0f32, f32::max);
        assert!(
            (reach - 1.0).abs() < 1e-5,
            "the farthest end reaches {reach}"
        );
        for pair in out.windows(2) {
            if pair[0].next != pair[0].b {
                assert_eq!(pair[0].next, pair[1].b, "a joined neighbour moved apart");
                assert_eq!(pair[1].prev, pair[0].a);
            }
        }
        assert_eq!(sphere_fit(&mut [], 1.0), 0.0, "nothing to fit");
    }

    #[test]
    fn normalize_fit_centers_and_scales_into_the_target_box() {
        let mut out = Vec::with_capacity(16);
        walk("F+F+F+F", std::f32::consts::FRAC_PI_2, 100, &mut out);
        normalize_fit(&mut out, 0.9);
        for seg in &out {
            for p in [seg.a, seg.b] {
                assert!(p[0].abs() <= 0.9 + 1e-4 && p[1].abs() <= 0.9 + 1e-4);
            }
        }
    }
}

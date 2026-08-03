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

use super::renderer::{JOINED_A, JOINED_B, SegmentInstance};

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
/// ([ADR-0059](../../../../../docs/adrs/0059-line-scenes-colour-along-their-generator-axis.md)'s
/// `lsystem` colour axis). Depth `0` is the trunk; each unclosed `[` is one more
/// generation, so a segment's depth is how many branch pushes are still open
/// above it.
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
                    // One joint, flagged from both sides. A turn does not break
                    // the run — `+`/`-` only change heading — which is why the
                    // state is a run rather than a look at the previous char.
                    let mut joined = 0;
                    if let Some(prev) = run.and_then(|i| out.get_mut(i)) {
                        prev.joined |= JOINED_B;
                        joined |= JOINED_A;
                    }
                    run = Some(out.len());
                    // Generation depth = how many branch pushes are still open.
                    depths.push(stack.len() as u32);
                    out.push(SegmentInstance {
                        a: [x, y],
                        b: [nx, ny],
                        color: [1.0, 1.0, 1.0],
                        width: 0.01,
                        joined,
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

    /// Plan 0039 Phase 3 done-when 2 and 4 (ADR-0041). The turtle is the tricky
    /// producer: it is a chain, but the chain **breaks** every time the pen stops
    /// continuing from where it was — at a branch push or pop, and at a
    /// move-without-draw. Asserted on the flag pattern rather than on pixels.
    #[test]
    fn the_turtle_joins_within_a_run_and_breaks_at_a_branch() {
        let mut out = Vec::with_capacity(16);
        // Trunk of two, a one-segment branch, then a trunk of two more.
        walk("FF[+F]FF", FRAC_PI_2, 100, &mut out);
        assert_eq!(out.len(), 5, "two trunk, one branch, two trunk");
        assert_eq!(
            out.iter().map(|s| s.joined).collect::<Vec<_>>(),
            vec![JOINED_B, JOINED_A, 0, JOINED_B, JOINED_A],
            "joined inside each run, free on both sides of the branch"
        );
        // The branch segment starts at the same point the first run ended, and
        // that is exactly the case the flag must *not* claim: it is a new stroke,
        // not a continuation, so extending it backward would run along the
        // branch's own direction into space it never covered.
        assert_eq!(
            out[1].b, out[2].a,
            "the branch does start at the trunk's end"
        );
        assert_eq!(out[2].joined, 0, "and is still free at both ends");

        // A turn is not a break — that is the whole reason the walk tracks a run
        // rather than looking at the previous character.
        out.clear();
        walk("F+F", FRAC_PI_2, 100, &mut out);
        assert_eq!(
            out.iter().map(|s| s.joined).collect::<Vec<_>>(),
            vec![JOINED_B, JOINED_A],
            "a turn keeps the pen on the paper"
        );

        // A move-without-draw is: the pen teleports, so the next segment starts
        // somewhere the last one never reached.
        out.clear();
        walk("FfF", 0.0, 100, &mut out);
        assert_eq!(
            out.iter().map(|s| s.joined).collect::<Vec<_>>(),
            vec![0, 0],
            "`f` breaks the run"
        );
        assert_ne!(out[0].b, out[1].a, "and the two really are disjoint");

        // A segment lost to the cap cannot be joined to, either.
        out.clear();
        let dropped = walk("FFFF", 0.0, 2, &mut out);
        assert_eq!((out.len(), dropped), (2, 2));
        assert_eq!(
            out.iter().map(|s| s.joined).collect::<Vec<_>>(),
            vec![JOINED_B, JOINED_A],
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

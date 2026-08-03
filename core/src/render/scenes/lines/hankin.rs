//! Hankin star patterns: build an n-fold star rosette by the contact-angle
//! method. `n` contact points sit symmetrically on a circle; from each, a ray
//! leaves at the contact angle from the inward normal, and adjacent rays meet at
//! the petal tips. Connecting each contact point to its two neighbouring tips
//! traces the interlaced star.
//!
//! **Since Plan 0054 / [ADR-0060](../../../../../docs/adrs/0060-star-pattern-variants-interpolate.md)
//! this runs from `Scene::update`, not only from `configure`.** `variant` is a
//! continuous contact angle, so a bound param reaches this construction during
//! playback; `star.rs`'s hysteresis cache bounds the rate (one rebuild per
//! `STEP_DEG` of travel, measured at 0.34 us for the reachable `n = 12`), but the
//! call itself is on the hot path and the panic pragma below is load-bearing
//! rather than precautionary.
//!
//! v1 scope (ADR-0007 / plan Risks): a small set of regular n-fold stars with a
//! contact angle — not arbitrary tessellations. The construction is a pure
//! deterministic function of `(n, contact_angle)` and, by building every petal
//! from the same rotation-equivariant rule, its segment set is invariant under a
//! `2*pi/n` rotation (directly unit-tested).

// Hot-path panic-denial pragma: reachable from `Scene::update` since ADR-0060
// (see the module docs). Written panic-free.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::f32::consts::TAU;

use super::renderer::{JOINED_A, JOINED_B, SegmentInstance};

/// Map a `tiling` name to its star order `n`. Accepts a few named/numeric
/// regular tilings (the v1 set); returns `None` for anything else so the loader
/// can reject it.
pub fn tiling_order(tiling: &str) -> Option<u32> {
    Some(match tiling.trim() {
        "square" | "4" | "4.4.4.4" => 4,
        "hexagon" | "6" | "6.6.6" => 6,
        "octagon" | "8" | "4.8.8" => 8,
        "dodecagon" | "12" | "3.12.12" => 12,
        _ => return None,
    })
}

/// Intersect ray `p + t*d` with ray `q + s*e` (t, s unbounded — infinite lines).
/// `None` if near-parallel.
fn line_intersect(p: [f32; 2], d: [f32; 2], q: [f32; 2], e: [f32; 2]) -> Option<[f32; 2]> {
    let denom = d[0] * e[1] - d[1] * e[0];
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / denom;
    Some([p[0] + t * d[0], p[1] + t * d[1]])
}

/// Build an `n`-fold star rosette with the given `contact_angle` (radians) into
/// `out` (cleared first). Produces `2 * n` segments when every petal tip
/// resolves. Positions are in roughly the unit disc; the scene fit-normalizes.
pub fn star_rosette(n: u32, contact_angle: f32, out: &mut Vec<SegmentInstance>) {
    out.clear();
    if n < 3 {
        return;
    }
    let nf = n as f32;

    // Contact point k, evenly spaced on the unit circle.
    let contact = |k: i32| -> [f32; 2] {
        let a = TAU * (k as f32) / nf;
        [a.cos(), a.sin()]
    };
    // Rotate a vector by `ang` radians.
    let rotate = |v: [f32; 2], ang: f32| -> [f32; 2] {
        let (s, c) = ang.sin_cos();
        [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
    };

    for k in 0..n as i32 {
        let m0 = contact(k);
        let m1 = contact(k + 1);
        // Inward normals (toward the centre) — the contact points lie on the
        // unit circle, so the inward normal is just the negated position.
        let in0 = [-m0[0], -m0[1]];
        let in1 = [-m1[0], -m1[1]];
        // Adjacent rays lean toward each other at the contact angle and meet at
        // the petal tip between the two contact points. m0's ray tilts toward
        // m1 (clockwise off its inward normal); m1's tilts back toward m0.
        let d0 = rotate(in0, -contact_angle);
        let d1 = rotate(in1, contact_angle);
        if let Some(tip) = line_intersect(m0, d0, m1, d1) {
            // The rosette is a **closed chain**, so every one of its `2n`
            // vertices is a joint (ADR-0041's Outcome note; Plan 0040 Phase 3).
            // The `b` ends meet at this petal's tip — but the `a` ends are not
            // free either: petal `k + 1` starts from `contact(k + 1)` again, the
            // same point from the same closure, so each contact point is shared
            // by two segments' `a` ends and the figure runs
            // `contact(0) -> tip(0) -> contact(1) -> tip(1) -> …`.
            //
            // The contact points are the **sharper** half. The two rays leave
            // one `2 * contact_angle` apart, so a stroke through turns by
            // `pi - 2 * contact_angle` and the wedge is
            // `half_width / tan(contact_angle)` — wider than the one at the tips
            // for any star pointier than 45 degrees, against `star.rs`'s
            // `CONTACT_MIN_DEG = 8`. Plan 0039 flagged only the tips.
            out.push(seg(m0, tip, JOINED_A | JOINED_B));
            out.push(seg(m1, tip, JOINED_A | JOINED_B));
        }
    }
}

fn seg(a: [f32; 2], b: [f32; 2], joined: u32) -> SegmentInstance {
    SegmentInstance {
        a,
        b,
        color: [1.0, 1.0, 1.0],
        width: 0.01,
        joined,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn tiling_names_map_to_orders() {
        assert_eq!(tiling_order("hexagon"), Some(6));
        assert_eq!(tiling_order("6.6.6"), Some(6));
        assert_eq!(tiling_order("8"), Some(8));
        assert_eq!(tiling_order("nonsense"), None);
    }

    #[test]
    fn rosette_has_the_expected_segment_count() {
        let mut out = Vec::new();
        star_rosette(6, 30f32.to_radians(), &mut out);
        // Two segments per petal (contact -> tip -> next contact).
        assert_eq!(out.len(), 12);

        let mut oct = Vec::new();
        star_rosette(8, 30f32.to_radians(), &mut oct);
        assert_eq!(oct.len(), 16);
    }

    /// Plan 0040 Phase 3 (ADR-0041's Outcome note), replacing Plan 0039's
    /// `the_star_joins_in_pairs_at_the_petal_tip`.
    ///
    /// **The shipped test could not see the defect it was meant to guard.** It
    /// asserted `!close(pair[0].a, pair[1].a)` — that the two contact points
    /// *within* one petal are distinct, which is true and stays true — and said
    /// nothing about the sharing *across* petals. So it passed unchanged both
    /// before and after this fix, which is exactly why it is gone.
    ///
    /// The rosette is a **closed chain**, not a set of pairs: petal `k` emits
    /// segments from `contact(k)` and `contact(k + 1)`, and petal `k + 1` emits
    /// one from `contact(k + 1)` again. All `2n` vertices are joints.
    #[test]
    fn the_star_is_a_closed_chain_joined_at_every_vertex() {
        let n = 5usize;
        let mut out = Vec::new();
        star_rosette(n as u32, 30f32.to_radians(), &mut out);
        assert_eq!(out.len(), 2 * n, "two segments per petal on a 5-fold star");

        for (i, seg) in out.iter().enumerate() {
            assert_eq!(
                seg.joined,
                JOINED_A | JOINED_B,
                "segment {i} lies in a closed chain, so both its ends are joints"
            );
        }

        // Within a petal: both rays end on the shared tip, and they start from
        // two distinct contact points.
        for (p, pair) in out.chunks_exact(2).enumerate() {
            assert!(
                close(pair[0].b, pair[1].b),
                "petal {p}: both rays must end on the shared tip"
            );
            assert!(
                !close(pair[0].a, pair[1].a),
                "petal {p}: a petal spans two distinct contact points"
            );
        }

        // Across petals — the half Plan 0039 missed. Segment `2k + 1` starts at
        // `contact(k + 1)` and so does segment `2k + 2`.
        //
        // The wrap-around pair (`2n - 1` against `0`) is the reason this uses
        // `close` rather than an exact compare: it is `contact(n)` against
        // `contact(0)`, the same point reached as `cos(TAU)` and `cos(0)`, which
        // are not bit-identical in f32.
        for k in 0..n {
            let (i, j) = (2 * k + 1, (2 * k + 2) % (2 * n));
            assert!(
                close(out[i].a, out[j].a),
                "segments {i} and {j} must meet at contact point {}",
                k + 1
            );
            assert!(
                out[i].joined & JOINED_A != 0 && out[j].joined & JOINED_A != 0,
                "both segments at contact point {} must declare that end joined, \
                 or the sharper half of the rosette keeps the notch",
                k + 1
            );
        }
    }

    #[test]
    fn rosette_is_invariant_under_a_2pi_over_n_rotation() {
        let n = 6u32;
        let mut out = Vec::new();
        star_rosette(n, 32f32.to_radians(), &mut out);
        assert!(!out.is_empty());

        let ang = TAU / n as f32;
        let (s, c) = ang.sin_cos();
        let rot = |p: [f32; 2]| [p[0] * c - p[1] * s, p[0] * s + p[1] * c];

        // Every segment, rotated by 2*pi/n, must match some original segment
        // (as an unordered endpoint pair) — the pattern has n-fold symmetry.
        for seg in &out {
            let ra = rot(seg.a);
            let rb = rot(seg.b);
            let matched = out.iter().any(|other| {
                (close(other.a, ra) && close(other.b, rb))
                    || (close(other.a, rb) && close(other.b, ra))
            });
            assert!(matched, "rotated segment has no image in the pattern");
        }
    }

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }
}

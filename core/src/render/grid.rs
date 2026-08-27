//! **The** internal-grid quantization policy, in one implementation (ADR-0037).
//!
//! Three places in the engine rasterize into an offscreen whose pixel dimensions
//! are not the surface's and then blit that offscreen to the surface with a
//! fullscreen triangle: the attractor's trail accumulation (Plan 0029) and the
//! two [`PostStage`](super::post::PostStage) internal grids (Plan 0033). All of
//! them want the same arithmetic and none of them wants the same numbers.
//!
//! # Why this is one function now
//!
//! It was two. `post.rs::internal_grid_size` was a line-for-line copy of
//! `particles::trail_grid_size` — same branch structure, same `u64` overflow
//! guard, same round-up — differing only in the cap constants. **That is how the
//! two came to have different aspect behavior at all**: Plan 0029 corrected the
//! attractor's projection to use the render target's aspect and left a comment
//! saying why, and the copy taken for the post stages did not carry the lesson,
//! so the same defect shipped a second time one layer up (ADR-0037). A shared
//! policy that is shared only by resemblance is not shared.
//!
//! # What the policy is, and what it deliberately is not
//!
//! Round each axis up to `step`; when either axis exceeds its cap, scale **both**
//! by a single factor first. The single factor is Plan 0029's lesson: clamping
//! each axis independently squashed a 3440x1440 ultrawide into a 16:9 grid, so
//! the picture's shape changed discontinuously as the window crossed the cap.
//!
//! It is **not** an aspect-preserving policy, and it does not try to be. The
//! round-up to `step` means the grid's ratio is only approximately the target's,
//! and that is fine — a grid is a **resolution, not a shape** (ADR-0037). Every
//! present out of one of these grids is a plain normalized stretch, and any pass
//! computing screen-destined geometry takes its aspect from the render target, so
//! the grid's own aspect cancels out of the picture. Once that holds, `step` and
//! `cap` are pure cost/quality knobs with no geometric side effect. That is the
//! whole prize, and it is why unifying the arithmetic here does not mean
//! unifying the constants: the two call sites cap at different sizes for
//! different, documented reasons, and those numbers stay theirs.
//!
//! Pure, GPU-free, and no wall clock — so a fixed-size headless capture stays
//! byte-reproducible (NFR §6).

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Pure arithmetic, but the pragma is the file-level convention
// for everything under render/ — and this one runs per resize on both paths.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The internal grid to rasterize into for a render target of `surface`, capped
/// at `cap` and quantized to `step` on each axis.
///
/// Never returns 0 on either axis and never exceeds either cap. Total on every
/// input: a zero `surface` axis floors to 1, a zero `step` floors to 1, and the
/// ratio arithmetic is done in `u64` so the products cannot overflow.
pub(crate) fn grid_size(surface: (u32, u32), cap: (u32, u32), step: u32) -> (u32, u32) {
    let w = surface.0.max(1);
    let h = surface.1.max(1);
    let (cap_w, cap_h) = (cap.0.max(1), cap.1.max(1));
    // Integer ratio compare and derivation, so the grid is an exact function of
    // the target size on every target rather than a float rounding of one.
    let (fit_w, fit_h) = if w <= cap_w && h <= cap_h {
        (w, h)
    } else if u64::from(w) * u64::from(cap_h) >= u64::from(h) * u64::from(cap_w) {
        // Width binds: pin it to the cap and derive the height from the target's
        // own aspect (<= cap_h by the branch condition).
        (
            cap_w,
            (u64::from(h) * u64::from(cap_w) / u64::from(w)) as u32,
        )
    } else {
        (
            (u64::from(w) * u64::from(cap_h) / u64::from(h)) as u32,
            cap_h,
        )
    };
    (
        quantize_axis(fit_w, cap_w, step),
        quantize_axis(fit_h, cap_h, step),
    )
}

/// Round one axis up to the next `step` multiple, floored at one step (never 0)
/// and clamped back under `cap` — the round-up overshoots on an axis already
/// sitting at the cap.
fn quantize_axis(px: u32, cap: u32, step: u32) -> u32 {
    let step = step.max(1);
    px.div_ceil(step).max(1).saturating_mul(step).min(cap)
}

#[cfg(test)]
mod tests {
    //! The policy's own properties. The two call sites keep their existing
    //! test sets against their own wrappers (`post.rs`, `core/tests/attractor.rs`);
    //! what is asserted here is that they are **one function**.

    // Test asserts panic on failure; allowed here over the file's pragma.
    #![allow(clippy::panic)]

    use super::grid_size;
    use crate::render::TierConfig;
    use crate::render::post::internal_grid_size;
    use crate::render::scenes::particles::trail_grid_size;

    /// Both call sites' caps at the tier every golden capture runs at (Plan 0044).
    /// These tests pin the shared *policy*, not either tier's numbers.
    const FLOOR: TierConfig = TierConfig::FLOOR;

    /// Sizes whose quantized grid lands under **both** call sites' caps, so
    /// neither cap binds and the two wrappers are answering the same question.
    /// These are the motivating targets plus the degenerate ends.
    const UNCAPPED: [(u32, u32); 8] = [
        (1, 1),
        (17, 3),
        (160, 100),
        (640, 480),
        (1280, 720),
        (1280, 800),
        (1366, 768),
        (1600, 900),
    ];

    /// **The drift guard.** Below both caps the two wrappers must agree exactly,
    /// because they are the same policy with different ceilings. This is what a
    /// future edit to one cannot silently break: there is no second
    /// implementation left to edit, and if someone reintroduces one this fails.
    #[test]
    fn the_two_call_sites_are_one_policy() {
        for surface in UNCAPPED {
            let post = internal_grid_size(surface, FLOOR.post_cap);
            let trail = trail_grid_size(surface.0, surface.1, FLOOR.attractor_trail_cap);
            assert_eq!(
                post, trail,
                "{surface:?}: the post stages and the attractor must quantize \
                 identically where neither cap binds — post {post:?}, trail {trail:?}"
            );
        }
    }

    /// And the caps stay different **on purpose** (ADR-0034's dual-live memory
    /// arithmetic for the post stages, Plan 0029's fill budget for the attractor).
    /// Unifying the policy is not unifying the numbers, so this pins that the
    /// difference survives — a "cleanup" that made the two caps equal would be a
    /// silent memory or fill-rate regression, not a tidy-up.
    #[test]
    fn the_caps_stay_different_on_purpose() {
        let surface = (3840, 2160);
        assert_ne!(
            internal_grid_size(surface, FLOOR.post_cap),
            trail_grid_size(surface.0, surface.1, FLOOR.attractor_trail_cap),
            "above the post cap the two must diverge — the attractor is allowed a \
             larger grid than the stages, which are charged twice by a dual-live dissolve"
        );
    }

    /// The cap is honoured on both axes, nothing is degenerate, and every axis
    /// lands on the step or on the cap — for arbitrary caps and steps, not just
    /// the two the engine ships.
    #[test]
    fn the_policy_is_bounded_quantized_and_non_degenerate() {
        for cap in [(1920, 1080), (2560, 1440), (256, 256), (1, 1)] {
            for step in [1, 64, 256, 4096] {
                for surface in [(1, 1), (17, 3), (640, 480), (3440, 1440), (100, 4000)] {
                    let (w, h) = grid_size(surface, cap, step);
                    assert!(w > 0 && h > 0, "{surface:?} {cap:?} {step}: degenerate");
                    assert!(
                        w <= cap.0.max(1) && h <= cap.1.max(1),
                        "{surface:?} over cap"
                    );
                    for (axis, axis_cap) in [(w, cap.0.max(1)), (h, cap.1.max(1))] {
                        assert!(
                            axis % step.max(1) == 0 || axis == axis_cap,
                            "{surface:?} {cap:?} {step}: axis {axis} is neither a \
                             multiple of the step nor the cap"
                        );
                    }
                }
            }
        }
    }

    /// A degenerate `step` cannot divide by zero or spin — the function is total,
    /// which matters because it runs on every resize on both paths.
    #[test]
    fn a_zero_step_is_total() {
        assert_eq!(grid_size((640, 480), (1920, 1080), 0), (640, 480));
        assert_eq!(grid_size((0, 0), (1920, 1080), 0), (1, 1));
    }

    /// Pure: the same inputs always yield the same grid, with no wall clock
    /// anywhere in it, so a fixed-size headless capture stays byte-reproducible
    /// (NFR §6).
    #[test]
    fn the_policy_is_a_pure_function() {
        for surface in [(800, 600), (2048, 1152), (3440, 1440)] {
            assert_eq!(
                grid_size(surface, (1920, 1080), 256),
                grid_size(surface, (1920, 1080), 256)
            );
        }
    }
}

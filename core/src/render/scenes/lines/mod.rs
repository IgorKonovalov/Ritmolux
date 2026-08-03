//! Line-geometry scenes (ADR-0007): a line-art category built on one shared
//! [`LineRenderer`] (segments -> thick glowing instanced quads) and two build
//! models over it — a cheap **parametric** system sampled every frame (the
//! Maurer rose) and, from Phase 3, an expensive **generator** system built and
//! cached at preset load. Ported in spirit from the user's Maurer rose,
//! L-system, and Islamic-star sketches; none of that JavaScript is reused, only
//! the math.
//!
//! The renderer and the per-frame scene halves are hot-path; the generators
//! (grammar/turtle/Hankin, from later phases) run only at load. All files here
//! live under `render/` and so carry the panic pragma the hygiene guard scans
//! for recursively — the build-time files are written panic-free too.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). `palette` may be called per frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub mod curves;
pub mod grammar;
pub mod hankin;
pub mod lsystem;
pub mod parametric;
pub mod renderer;
pub mod spectrum;
pub mod star;
pub mod turtle;

pub use lsystem::LSystemScene;
pub use parametric::ParametricCurveScene;
pub use renderer::{JOINED_A, JOINED_B, LineRenderer, SegmentInstance};
pub use spectrum::{SpectrumLayout, SpectrumScene};
pub use star::StarPatternScene;

/// The shared structural-config and cap-overflow types now live one level up, in
/// [`scenes`](super) — every scene family can see them there without the line
/// module having to reach sideways into `particles` for an attractor variant
/// (Plan 0031 Phase 6, closing Plan 0016's close-review minor 2). Re-exported
/// here because the sibling line scenes and the preset schema name them through
/// this path.
pub use super::{CapOverflow, GeneratorConfig, OverflowContext};

/// Hard clamp on L-system iteration depth, enforced at preset load. A branching
/// rule expands exponentially, so an unbounded `max_depth` would stall a preset
/// switch and blow the segment cap (ADR-0007 Risks). Curated presets stay well
/// under this; the turtle's own segment cap is the second backstop.
pub const MAX_LSYSTEM_DEPTH: u32 = 7;

/// Hard clamp on the geometry-mirror rotational order (Plan 0018 Phase 4). Beyond
/// a couple dozen the fold is visually indistinguishable and only multiplies
/// segment count toward the cap; a sane ceiling keeps a runaway `mirror_order`
/// expression from doing useless work before the segment cap bites.
pub const MAX_MIRROR_ORDER: u32 = 24;

/// The shared camera transform every scene family applies (ADR-0018): a uniform
/// **zoom** about the frame centre, then a **pan**, in world space before the
/// aspect divide. Identity (`zoom = 1`, `pan = 0`) leaves geometry exactly where
/// a scene placed it, so a preset that binds none of `zoom`/`pan_x`/`pan_y` is
/// unchanged. `#[repr(C)]` + `Pod` so it uploads straight into a line-renderer
/// uniform slot. Rotate is reserved for a follow-up (ADR-0018 reserves it).
///
/// Defined here for Phase 1 (the line scenes are the walking skeleton); Phase 2
/// threads the same transform through the fragment and swarm scenes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewTransform {
    /// Uniform scale about the frame centre (`1.0` = no zoom).
    pub zoom: f32,
    /// Pan offset in world units `(x, y)`, applied after the zoom.
    pub pan: [f32; 2],
    /// Padding to fill a 16-byte uniform slot (unused).
    pub _pad: f32,
}

impl Default for ViewTransform {
    /// The identity view: no zoom, no pan.
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
            _pad: 0.0,
        }
    }
}

/// Which parametric curve family a `[curve]` preset draws. Extend as Plan 0010's
/// follow-ups add curve families (epicycloids, Lissajous, ...); unknown names
/// are rejected at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveFamily {
    /// The Maurer rose — `sin(n * theta)` walked at a fixed angular step.
    MaurerRose,
}

impl CurveFamily {
    /// Parse a `[curve] family` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "maurer_rose" => CurveFamily::MaurerRose,
            _ => return None,
        })
    }
}

/// The colour surface every line scene shares
/// ([ADR-0021](../../../../../docs/adrs/0021-shared-palette-system.md) /
/// [ADR-0059](../../../../../docs/adrs/0059-line-scenes-colour-along-their-generator-axis.md)):
/// `hue` places the whole figure in the baked palette and `hue_spread` says how
/// far the palette travels **across** it.
///
/// The axis `u` walks is the one thing that differs per scene — generation depth
/// on the L-system, path position on the parametric curve, radius on the star,
/// band index on the spectrum readout — so the *colour* half lives here once
/// rather than as four similar-but-not-identical loops that drift
/// (ADR-0059's own stated risk). Each generator computes its `u` and asks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColorRamp {
    /// Where the figure sits in the palette.
    pub hue: f32,
    /// How far the palette travels from `u = 0` to `u = 1`. `0` — every scene's
    /// default — is the single flat `hue` the line scenes drew before the
    /// palette reached them, which is what makes the surface a strict superset.
    pub hue_spread: f32,
    /// A/B crossfade position (`0` = palette A alone).
    pub palette_mix: f32,
    /// Shared saturation modulation, applied to the sampled colour.
    pub saturation: f32,
    /// Stroke brightness, folded in here because these scenes carry it in the
    /// segment colour rather than as a separate uniform.
    pub brightness: f32,
}

impl ColorRamp {
    /// The stroke colour at normalized position `u` along the scene's own axis.
    /// Allocation-free; runs per segment (or per generation) on the hot path.
    pub(crate) fn at(self, pal: &crate::render::palette::Palette, u: f32) -> [f32; 3] {
        let rgb = crate::render::palette::desaturate(
            pal.sample(self.hue + self.hue_spread * u, self.palette_mix),
            self.saturation,
        );
        [
            rgb[0] * self.brightness,
            rgb[1] * self.brightness,
            rgb[2] * self.brightness,
        ]
    }
}

/// iq-style cosine palette (RGB phase-shifted), matching the swarm/fragment
/// scenes so line art shares the engine's colour language.
pub fn palette(t: f32) -> [f32; 3] {
    let tau = std::f32::consts::TAU;
    [
        0.5 + 0.5 * (tau * (t + 0.10)).cos(),
        0.5 + 0.5 * (tau * (t + 0.42)).cos(),
        0.5 + 0.5 * (tau * (t + 0.62)).cos(),
    ]
}

/// The per-frame half shared by every **generator** line scene (L-system,
/// star): transform cached base geometry into `out` — rotate by `rotation`
/// (radians), scale, colour, set `width`, and reveal a `progress` prefix
/// (line-draw-on). Allocation-free into a preallocated `out`; expansion /
/// construction lives at load, this is the only per-frame work.
pub(crate) fn transform_cached(
    base: &[SegmentInstance],
    rotation: f32,
    scale: f32,
    color: [f32; 3],
    width: f32,
    progress: f32,
    out: &mut Vec<SegmentInstance>,
) {
    out.clear();
    let (sin, cos) = rotation.sin_cos();
    let keep = ((base.len() as f32) * progress.clamp(0.0, 1.0)).round() as usize;
    let rot = |p: [f32; 2]| -> [f32; 2] {
        [
            (p[0] * cos - p[1] * sin) * scale,
            (p[0] * sin + p[1] * cos) * scale,
        ]
    };
    for seg in base.iter().take(keep) {
        out.push(SegmentInstance {
            a: rot(seg.a),
            b: rot(seg.b),
            color,
            width,
            // Connectivity is a property of the cached structure, not of this
            // frame's rotation/scale/colour, so it passes straight through.
            joined: seg.joined,
        });
    }
}

/// N-fold geometry-mirror spec (Plan 0018 Phase 4): replicate a line scene's
/// segment set under rotational (and optionally reflective) symmetry to build a
/// true geometric fractal. Driven by the `mirror_order` / `mirror_reflect` named
/// params. `order = 1, reflect = false` is the identity — the base drawn once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MirrorSpec {
    /// Rotational symmetry order (`>= 1`).
    pub order: u32,
    /// Also emit a reflected copy per sector (dihedral symmetry).
    pub reflect: bool,
}

impl MirrorSpec {
    /// Build a spec from the raw `mirror_order` / `mirror_reflect` param values —
    /// the shared conversion every line scene uses. The order rounds and clamps to
    /// `1..=MAX_MIRROR_ORDER` (a non-finite or `< 1` value is the identity);
    /// `reflect` is a `>= 0.5` threshold so a preset can drive it with a `beat`.
    pub fn from_params(order: f32, reflect: f32) -> Self {
        let order = if order.is_finite() {
            (order.round() as i64).clamp(1, MAX_MIRROR_ORDER as i64) as u32
        } else {
            1
        };
        Self {
            order,
            reflect: reflect >= 0.5,
        }
    }

    /// How many copies of the base a full replication emits.
    fn copies(self) -> usize {
        self.order.max(1) as usize * if self.reflect { 2 } else { 1 }
    }

    /// Whether replication would be a no-op — one sector, no reflection, so the
    /// output is the input. The common case (no shipped preset binds
    /// `mirror_order`), and the one the scenes skip the copy for.
    pub(crate) fn is_identity(self) -> bool {
        self.order <= 1 && !self.reflect
    }
}

/// Replicate `single` (already positioned/coloured segments) about the frame
/// centre under `mirror.order`-fold rotation, plus an optional reflected copy per
/// sector, into `out` (cleared first) — a geometric kaleidoscope whose segment
/// set is invariant under a `2*pi/order` rotation. Truncates at `cap` (the active
/// tier's [`max_segments`](crate::render::TierConfig::max_segments), which the
/// scene resolved at construction) and returns the number of segments dropped, so
/// the caller can surface it — the cap is never a silent cut (ADR-0007 Risks).
///
/// Allocation-free into a preallocated `out`; the per-frame half of every mirrored
/// line scene.
pub(crate) fn replicate_mirror(
    single: &[SegmentInstance],
    mirror: MirrorSpec,
    cap: usize,
    out: &mut Vec<SegmentInstance>,
) -> usize {
    out.clear();
    let n = mirror.order.max(1);
    let wanted = single.len() * mirror.copies();
    for k in 0..n {
        let sector = std::f32::consts::TAU * (k as f32) / (n as f32);
        let (sin, cos) = sector.sin_cos();
        for reflected in [false, true] {
            if reflected && !mirror.reflect {
                continue;
            }
            // Reflect across the x-axis (optional), then rotate into the sector.
            let map = |p: [f32; 2]| -> [f32; 2] {
                let y = if reflected { -p[1] } else { p[1] };
                [p[0] * cos - y * sin, p[0] * sin + y * cos]
            };
            for seg in single {
                if out.len() >= cap {
                    break;
                }
                out.push(SegmentInstance {
                    a: map(seg.a),
                    b: map(seg.b),
                    color: seg.color,
                    width: seg.width,
                    // A reflected or rotated copy has the same connectivity as
                    // its source: the geometry moves, the topology does not.
                    joined: seg.joined,
                });
            }
        }
    }
    wanted.saturating_sub(out.len())
}

#[cfg(test)]
mod tests {
    // Test asserts index the produced Vec; allowed here over the file's hot-path
    // pragma since test code is not the render path.
    #![allow(clippy::indexing_slicing)]

    use super::*;

    /// The cap these tests run at — the floor tier's, which is the value they were
    /// written against and the one every shipped preset is authored and gated on
    /// (Plan 0044).
    const FLOOR_CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

    fn seg(a: [f32; 2], b: [f32; 2]) -> SegmentInstance {
        SegmentInstance {
            a,
            b,
            color: [0.4, 0.7, 1.0],
            width: 0.01,
            joined: 0,
        }
    }

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    /// A 6-fold mirror of an asymmetric base must be invariant under a `2*pi/6`
    /// rotation (the Hankin-style symmetry proof) and emit exactly `order` copies.
    #[test]
    fn mirror_is_invariant_under_a_2pi_over_order_rotation() {
        // A deliberately asymmetric little scribble so symmetry is non-trivial.
        let single = vec![seg([0.1, 0.05], [0.4, 0.2]), seg([0.4, 0.2], [0.3, 0.5])];
        let order = 6u32;
        let mut out = Vec::new();
        let dropped = replicate_mirror(
            &single,
            MirrorSpec {
                order,
                reflect: false,
            },
            10_000,
            &mut out,
        );
        assert_eq!(dropped, 0, "well under the cap");
        assert_eq!(
            out.len(),
            single.len() * order as usize,
            "one rotated copy of the base per sector"
        );

        let ang = std::f32::consts::TAU / order as f32;
        let (s, c) = ang.sin_cos();
        let rot = |p: [f32; 2]| [p[0] * c - p[1] * s, p[0] * s + p[1] * c];
        for seg in &out {
            let ra = rot(seg.a);
            let rb = rot(seg.b);
            let matched = out.iter().any(|other| {
                (close(other.a, ra) && close(other.b, rb))
                    || (close(other.a, rb) && close(other.b, ra))
            });
            assert!(matched, "rotated segment has no image in the mirrored set");
        }
    }

    /// The identity spec (`from_params(1, 0)`) copies the base through unchanged —
    /// which is *why* the scenes skip the call entirely at that spec (Plan 0031
    /// Phase 4). This test is the equivalence that licenses the skip: replication
    /// at an identity spec yields exactly the input, no drop and no transform, so
    /// swapping the buffers instead produces the same frame.
    #[test]
    fn identity_spec_copies_the_base_unchanged() {
        let single = vec![
            seg([0.2, 0.3], [0.5, 0.1]),
            seg([-0.4, 0.05], [0.15, -0.35]),
        ];
        let spec = MirrorSpec::from_params(1.0, 0.0);
        assert!(spec.is_identity(), "the spec the scenes skip on");
        let mut out = Vec::new();
        let dropped = replicate_mirror(&single, spec, 10_000, &mut out);
        assert_eq!(dropped, 0, "identity never truncates");
        assert_eq!(out.len(), single.len(), "no copies added or lost");
        for (produced, source) in out.iter().zip(&single) {
            assert!(
                close(produced.a, source.a) && close(produced.b, source.b),
                "identity applies no transform"
            );
            assert_eq!(produced.color, source.color);
            assert_eq!(produced.width, source.width);
        }
    }

    /// Exactly which specs the scenes may skip replication for. A spec that is
    /// *not* the identity must not be treated as one — that would silently drop a
    /// preset's mirror.
    #[test]
    fn is_identity_covers_exactly_the_no_op_specs() {
        assert!(
            MirrorSpec {
                order: 1,
                reflect: false
            }
            .is_identity()
        );
        // A non-finite or sub-1 order clamps to 1, which is the identity.
        assert!(MirrorSpec::from_params(0.0, 0.0).is_identity());
        assert!(MirrorSpec::from_params(f32::NAN, 0.0).is_identity());
        assert!(
            MirrorSpec::from_params(1.4, 0.0).is_identity(),
            "rounds to 1"
        );
        // Reflection at order 1 still doubles the geometry — not an identity.
        assert!(
            !MirrorSpec {
                order: 1,
                reflect: true
            }
            .is_identity()
        );
        assert!(!MirrorSpec::from_params(1.0, 1.0).is_identity());
        assert!(!MirrorSpec::from_params(2.0, 0.0).is_identity());
        assert!(
            !MirrorSpec::from_params(1.6, 0.0).is_identity(),
            "rounds to 2"
        );
    }

    /// The message an operator actually reads. Plan 0031 Phase 4 replaced the
    /// per-frame `format!` with an enum that formats only in `Display`; ADR-0007
    /// requires the text stay informative, so both renderings are pinned here.
    #[test]
    fn the_overflow_message_is_unchanged() {
        assert_eq!(OverflowContext::Mirror(6).to_string(), "mirror x6");
        assert_eq!(OverflowContext::Depth(6).to_string(), "depth 6");
        assert_eq!(
            CapOverflow {
                dropped: 350,
                context: OverflowContext::Mirror(6),
                cap: FLOOR_CAP,
            }
            .to_string(),
            format!(
                "geometry exceeded the {FLOOR_CAP}-segment cap at mirror x6 \
                 (dropped 350 segment(s)); reduce the structure or its depth"
            )
        );
        assert_eq!(
            CapOverflow {
                dropped: 1,
                context: OverflowContext::Depth(7),
                cap: FLOOR_CAP,
            }
            .to_string(),
            format!(
                "geometry exceeded the {FLOOR_CAP}-segment cap at depth 7 \
                 (dropped 1 segment(s)); reduce the structure or its depth"
            )
        );
    }

    /// **The message names the cap that actually bit**, not a constant.
    ///
    /// The cap is a tier value now (Plan 0044), so the two assertions above would
    /// both still pass if `Display` had gone back to reading a hardcoded 20 000 —
    /// the floor's cap *is* 20 000. Formatting the same overflow at the rich cap
    /// is what makes the reading non-vacuous: a reverted `Display` would print the
    /// floor's number here and fail. ADR-0007 requires the surfaced cut be
    /// informative, and a message quoting a cap the run was not using is worse
    /// than none.
    #[test]
    fn the_overflow_message_names_the_cap_it_carries() {
        let rich_cap = crate::render::TierConfig::RICH.max_segments;
        assert_ne!(
            rich_cap, FLOOR_CAP,
            "the two tiers must differ for this to test anything"
        );

        let at = |cap: usize| {
            CapOverflow {
                dropped: 7,
                context: OverflowContext::Mirror(3),
                cap,
            }
            .to_string()
        };
        assert!(
            at(rich_cap).contains(&rich_cap.to_string()),
            "{}",
            at(rich_cap)
        );
        assert!(
            !at(rich_cap).contains(&FLOOR_CAP.to_string()),
            "a rich-tier overflow must not quote the floor's cap: {}",
            at(rich_cap)
        );
        assert_ne!(at(rich_cap), at(FLOOR_CAP));
    }

    /// Plan 0039 Phase 1 done-when 4. Replication moves geometry; it does not
    /// change **topology**. A rotated or reflected copy of a joined chain is
    /// still a joined chain, so every copy must carry its source's per-endpoint
    /// flags through verbatim — dropping them would silently reopen the notch on
    /// every mirrored copy while the un-mirrored original looked correct.
    #[test]
    fn the_mirror_carries_the_join_flags_through() {
        let mut single = vec![
            seg([0.1, 0.05], [0.4, 0.2]),
            seg([0.4, 0.2], [0.3, 0.5]),
            seg([0.3, 0.5], [0.6, 0.45]),
        ];
        // A three-segment chain: the interior vertices are joints, the two outer
        // ends are free — the pattern every chained producer emits.
        single[0].joined = JOINED_B;
        single[1].joined = JOINED_A | JOINED_B;
        single[2].joined = JOINED_A;
        let expected: Vec<u32> = single.iter().map(|s| s.joined).collect();

        for spec in [
            MirrorSpec {
                order: 4,
                reflect: false,
            },
            MirrorSpec {
                order: 3,
                reflect: true,
            },
        ] {
            let mut out = Vec::new();
            let dropped = replicate_mirror(&single, spec, 10_000, &mut out);
            assert_eq!(dropped, 0, "well under the cap");
            assert_eq!(out.len(), single.len() * spec.copies());
            for (i, produced) in out.iter().enumerate() {
                assert_eq!(
                    produced.joined,
                    expected[i % expected.len()],
                    "{spec:?} copy of segment {} lost its connectivity",
                    i % expected.len()
                );
            }
        }
    }

    /// The same claim for the generator scenes' per-frame transform: rotation,
    /// scale, colour and width are this frame's styling, but connectivity
    /// belongs to the cached structure and survives untouched.
    #[test]
    fn the_cached_transform_carries_the_join_flags_through() {
        let mut base = vec![seg([0.0, 0.0], [0.3, 0.1]), seg([0.3, 0.1], [0.5, 0.4])];
        base[0].joined = JOINED_B;
        base[1].joined = JOINED_A;

        let mut out = Vec::new();
        transform_cached(&base, 0.7, 1.4, [0.2, 0.9, 0.3], 0.02, 1.0, &mut out);
        assert_eq!(out.len(), base.len());
        for (produced, source) in out.iter().zip(&base) {
            assert_eq!(produced.joined, source.joined);
            assert_ne!(produced.color, source.color, "styling still applies");
        }
    }

    /// Reflection doubles the copy count and stays rotationally symmetric.
    #[test]
    fn reflection_doubles_the_copies() {
        let single = vec![seg([0.1, 0.2], [0.4, 0.3])];
        let mut out = Vec::new();
        replicate_mirror(
            &single,
            MirrorSpec {
                order: 5,
                reflect: true,
            },
            10_000,
            &mut out,
        );
        assert_eq!(out.len(), single.len() * 5 * 2, "rotation x reflection");
    }

    /// Exceeding `cap` truncates the output and reports the exact drop — the cap
    /// is never a silent cut (ADR-0007).
    #[test]
    fn overflow_truncates_and_reports_the_drop() {
        // 100 base segments, 6-fold = 600 wanted, capped at 250 -> 350 dropped.
        let single: Vec<_> = (0..100)
            .map(|i| seg([i as f32 * 0.001, 0.1], [0.2, i as f32 * 0.001]))
            .collect();
        let mut out = Vec::new();
        let dropped = replicate_mirror(
            &single,
            MirrorSpec {
                order: 6,
                reflect: false,
            },
            250,
            &mut out,
        );
        assert_eq!(out.len(), 250, "output is truncated at the cap");
        assert_eq!(dropped, 600 - 250, "the exact drop is reported");
    }
}

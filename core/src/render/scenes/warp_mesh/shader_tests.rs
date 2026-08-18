//! The shader surface's **CPU half** (Plan 0110 Phase 1): the uniform fill, the
//! hue corners, the roam vectors, the `rot_*` matrices and the procedural noise.
//!
//! None of this needs a GPU, and that is the point. `shader.rs` is built only
//! for a bundle that carries WGSL, so until Plan 0110 nothing in the crate
//! reached it on any adapter — yet more than half the file is arithmetic that a
//! plain `cargo test` can hold to its documented contract. What is asserted here
//! is **properties, not frozen numbers** (ADR-0071): each one is either exact
//! because the mechanism makes it exact (a channel divided by its own maximum is
//! 1.0), or carries a tolerance derived from the mechanism (Rodrigues' formula
//! produces a rotation, so the rows are orthonormal to `f32`).
//!
//! The GPU half — `MilkShaderResources::build`, the bind groups, the blur chain
//! — is `core/tests/warp_mesh.rs`'s, driven by the shader-carrying fixture
//! Phase 2 writes.

// Test asserts panic on failure; allowed here over the module's hot-path pragma.
#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used
)]

use super::*;

/// The smallest runtime [`fill_uniform`] resolves against. Every lane this file
/// asserts on is derived from the *arguments* rather than from what a program
/// left, so an empty bundle is not a weaker fixture here — it is the one that
/// isolates the conversion from the VM.
fn empty_runtime() -> crate::milk::MilkRuntime {
    let bundle =
        crate::milk::MilkBundle::from_assembly(None, None, None).expect("the empty bundle decodes");
    crate::milk::MilkRuntime::new(bundle, 0x5A17_C0DE)
}

/// One degenerate [`fill_uniform`] call: what it is, the target size, the
/// aspect, the frame's `dt`, and the per-second decay.
type DegenerateCase = (&'static str, (u32, u32), f32, f32, f32);

/// Every `f32` in the block, for the totality sweep.
fn all_lanes(u: &MilkUniform) -> Vec<f32> {
    let mut out = Vec::new();
    for row in [
        u.clock,
        u.bands,
        u.bands_att,
        u.texsize,
        u.aspect,
        u.rand_frame,
        u.rand_preset,
        u.misc,
    ] {
        out.extend_from_slice(&row);
    }
    for row in u.hue {
        out.extend_from_slice(&row);
    }
    for row in u.q {
        out.extend_from_slice(&row);
    }
    for row in u.roam {
        out.extend_from_slice(&row);
    }
    for row in u.rot {
        out.extend_from_slice(&row);
    }
    out
}

/// The mean absolute difference between horizontally adjacent texels, over every
/// channel — the statistic that separates the interpolated noise arm from the
/// per-texel one without freezing either.
fn mean_neighbour_delta(data: &[u8], size: usize) -> f64 {
    let mut sum = 0.0f64;
    let mut count = 0u64;
    for y in 0..size {
        for x in 0..size.saturating_sub(1) {
            for c in 0..4 {
                let a = f64::from(data[(y * size + x) * 4 + c]);
                let b = f64::from(data[(y * size + x + 1) * 4 + c]);
                sum += (a - b).abs();
                count += 1;
            }
        }
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}

/// **The staleness key separates every field it is asked to separate.** It is
/// what decides whether a preset switch rebuilds the pipelines, so a key that
/// ignored `blur` would leave a three-level chain built for a preset that asked
/// for none — or worse, none built for one that reads `GetBlur3`.
#[test]
fn a_spec_key_is_stable_and_separates_every_field() {
    let base = ShaderSpec {
        warp: Some("warp module".into()),
        comp: Some("comp module".into()),
        blur: 2,
    };
    assert_eq!(base.key(), base.key(), "repeated calls agree");
    assert_eq!(base.key(), base.clone().key(), "and so do equal specs");

    let mut warp = base.clone();
    warp.warp = Some("a different warp module".into());
    let mut warp_absent = base.clone();
    warp_absent.warp = None;
    let mut comp = base.clone();
    comp.comp = None;
    let mut blur = base.clone();
    blur.blur = 3;
    for (what, other) in [
        ("a changed warp", warp),
        ("a dropped warp", warp_absent),
        ("a dropped comp", comp),
        ("a deeper blur", blur),
    ] {
        assert_ne!(base.key(), other.key(), "{what} must rebuild the pipelines");
    }

    // The default — what a bundle without shaders configures — is itself stable,
    // so a native preset does not thrash the rebuild check.
    assert_eq!(ShaderSpec::default().key(), ShaderSpec::default().key());
}

/// **Each hue corner is normalized to its brightest channel**, which is the
/// documented reason the corners stay bright as they drift: a corner whose peak
/// drifted with the sines would darken the whole `hue_shader` mix.
#[test]
fn every_hue_corner_is_normalized_to_its_brightest_channel() {
    for time in [0.0, 1.0, 13.75, 600.0, 86_400.0] {
        let corners = hue_corners(time);
        for (k, row) in corners.iter().enumerate() {
            let peak = row[0].max(row[1]).max(row[2]);
            assert_eq!(peak, 1.0, "corner {k} at t={time} peaks at {peak}");
            assert_eq!(row[3], 1.0, "corner {k} at t={time} is opaque");
            assert!(
                row.iter().all(|c| c.is_finite() && *c > 0.0),
                "corner {k} at t={time}: {row:?}"
            );
        }
    }
    assert_eq!(hue_corners(3.5), hue_corners(3.5), "pure in `time`");
    assert_ne!(hue_corners(3.5), hue_corners(90.0), "and it does drift");
}

/// Every roam component is the reference's `0.5 + 0.5 * f` remap, so a shader
/// multiplying a coordinate by one never mirrors it.
#[test]
fn every_roam_component_lands_in_the_documented_unit_range() {
    for time in [0.0, 0.37, 12.0, 3600.0, 100_000.0] {
        for (r, row) in roam_vectors(time).iter().enumerate() {
            for (i, c) in row.iter().enumerate() {
                assert!(
                    (0.0..=1.0).contains(c),
                    "roam[{r}][{i}] = {c} at t={time} is outside 0..=1"
                );
            }
        }
    }
    assert_eq!(roam_vectors(2.0), roam_vectors(2.0), "pure in `time`");
    assert_ne!(roam_vectors(0.0), roam_vectors(2.0), "and it does move");
}

/// **Every `rot_*` matrix is a rotation and nothing else.** Rodrigues' formula
/// about a unit axis produces an orthonormal 3x3 block; a drift into a scale or
/// a shear would silently stretch every preset that reads one, and 28 files in
/// the corpus do.
#[test]
fn every_rot_matrix_is_an_orthonormal_rotation() {
    for (time, salt) in [(0.0, 0u32), (4.25, 0x1234_5678), (912.5, 0xFFFF_FFFF)] {
        let rows = rot_rows(time, salt);
        assert_eq!(rows.len(), ROT_MATRICES * 4, "four rows per matrix");
        for m in 0..ROT_MATRICES {
            let r: [[f32; 3]; 3] = std::array::from_fn(|i| {
                let row = rows[m * 4 + i];
                [row[0], row[1], row[2]]
            });
            for (i, row) in r.iter().enumerate() {
                let norm = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
                assert!(
                    (norm - 1.0).abs() < 1e-4,
                    "salt {salt} matrix {m} row {i} has norm {norm}"
                );
            }
            for (i, j) in [(0, 1), (0, 2), (1, 2)] {
                let dot = r[i][0] * r[j][0] + r[i][1] * r[j][1] + r[i][2] * r[j][2];
                assert!(
                    dot.abs() < 1e-4,
                    "salt {salt} matrix {m} rows {i},{j} are not perpendicular: {dot}"
                );
            }
            // The fourth row is a translation, so `float4x3` indexing reads a
            // point rather than a direction.
            let t = rows[m * 4 + 3];
            assert_eq!(t[3], 1.0, "matrix {m}'s fourth row is a point");
            assert!(t.iter().all(|v| v.is_finite()), "matrix {m}: {t:?}");
        }
    }

    assert_eq!(
        rot_rows(4.25, 7),
        rot_rows(4.25, 7),
        "the same salt reproduces — a capture replays"
    );
    assert_ne!(
        rot_rows(4.25, 7),
        rot_rows(4.25, 8),
        "a different salt re-seeds the axes"
    );
    assert_ne!(rot_rows(0.0, 7), rot_rows(4.25, 7), "and they do spin");
}

/// **The noise set is the right size, seeded, and takes two genuinely different
/// arms.** `noise_lq` is per-texel randoms; `noise_mq`/`_hq` are a coarser
/// lattice interpolated up, and "smoothly interpolated" is checkable as a lower
/// mean neighbour difference at equal size without freezing a byte.
#[test]
fn the_noise_textures_are_sized_seeded_and_smoothed_as_documented() {
    // Exactly RGBA8 at the requested extent — `write_texture` is handed
    // `bytes_per_row = size * 4`, so a short buffer is a device error.
    for size in [1u32, 8, 32] {
        for zoom in [1u32, 4] {
            assert_eq!(
                noise_2d(size, zoom, 1).len(),
                (size as usize).pow(2) * 4,
                "noise_2d({size}, {zoom})"
            );
            assert_eq!(
                noise_3d(size, zoom, 1).len(),
                (size as usize).pow(3) * 4,
                "noise_3d({size}, {zoom})"
            );
        }
    }

    // Deterministic in `(size, zoom, seed)` — MilkDrop builds its noise once at
    // startup and this engine has to hand every run and every machine the same
    // bytes (the module docs' determinism claim).
    assert_eq!(noise_2d(16, 1, 3), noise_2d(16, 1, 3));
    assert_eq!(noise_3d(8, 2, 3), noise_3d(8, 2, 3));
    assert_ne!(
        noise_2d(16, 1, 3),
        noise_2d(16, 1, 4),
        "the seed reaches it"
    );
    assert_ne!(noise_3d(8, 2, 3), noise_3d(8, 2, 4), "the seed reaches it");

    // The two arms are different code and different pictures.
    let flat = noise_2d(64, 1, 11);
    let lattice = noise_2d(64, 4, 11);
    assert_ne!(flat, lattice, "zoom picks a different arm");
    let flat_delta = mean_neighbour_delta(&flat, 64);
    let lattice_delta = mean_neighbour_delta(&lattice, 64);
    assert!(
        lattice_delta < flat_delta,
        "the interpolated arm must be smoother than the per-texel one: \
         zoom 4 gave {lattice_delta}, zoom 1 gave {flat_delta}"
    );
    // The same separation in 3D, along the x axis of the first slice.
    let vol_flat = noise_3d(16, 1, 11);
    let vol_lattice = noise_3d(16, 4, 11);
    assert!(
        mean_neighbour_delta(&vol_lattice, 16) < mean_neighbour_delta(&vol_flat, 16),
        "the 3D volumes take the same two arms"
    );
}

/// **The uniform's rate conversion and its aspect pair.** `decay` arrives
/// per second and reaches the shader as *this frame's* factor (ADR-0019 applied
/// to a shader input), and the aspect lanes carry the EEL convention plus their
/// own reciprocals so a shader never divides.
#[test]
fn the_uniform_converts_decay_and_keeps_the_aspect_pair_reciprocal() {
    let runtime = empty_runtime();
    let landscape = fill_uniform(
        &runtime,
        2.0,
        1.0,
        (1920, 1080),
        16.0 / 9.0,
        0.93,
        1.0,
        0.0,
        crate::milk::DEFAULT_QUANTIZE_STEPS,
    );
    // A one-second frame decays by exactly the per-second value: `v^1 == v`.
    assert_eq!(
        landscape.misc[0], 0.93,
        "at dt == 1 the frame factor IS the per-second one"
    );
    assert_eq!(
        landscape.misc[3],
        crate::milk::DEFAULT_QUANTIZE_STEPS,
        "the quantize count rides the free misc.w lane (ADR-0118)"
    );
    assert_eq!(landscape.aspect[0], 1.0, "landscape: the longer axis is x");
    assert!(landscape.aspect[1] > 1.0);
    assert_eq!(landscape.aspect[2], 1.0 / landscape.aspect[0]);
    assert_eq!(landscape.aspect[3], 1.0 / landscape.aspect[1]);
    assert_eq!(landscape.texsize[0], 1920.0);
    assert_eq!(landscape.texsize[1], 1080.0);
    assert_eq!(landscape.texsize[2], 1.0 / 1920.0);
    assert_eq!(landscape.texsize[3], 1.0 / 1080.0);
    assert_eq!(landscape.clock[0], 2.0, "the clock is the injected time");
    assert_eq!(landscape.clock[1], crate::milk::NOMINAL_FPS);

    let portrait = fill_uniform(
        &runtime,
        2.0,
        1.0,
        (1080, 1920),
        1080.0 / 1920.0,
        0.93,
        1.0,
        0.0,
        0.0,
    );
    assert_eq!(portrait.aspect[1], 1.0, "portrait: the longer axis is y");
    assert!(portrait.aspect[0] > 1.0);
    assert_eq!(portrait.aspect[2], 1.0 / portrait.aspect[0]);
    assert_eq!(portrait.aspect[3], 1.0 / portrait.aspect[1]);

    // A half-second frame takes the square root of the per-second factor, which
    // is what makes the fade rate independent of the refresh.
    let half = fill_uniform(&runtime, 0.0, 0.5, (64, 64), 1.0, 0.25, 1.0, 0.0, 0.0);
    assert!(
        (half.misc[0] - 0.5).abs() < 1e-6,
        "0.25 per second over half a second is 0.5, got {}",
        half.misc[0]
    );
}

/// **The whole pure half is total on degenerate input.** The file carries the
/// hot-path `#![deny(clippy::panic, clippy::indexing_slicing, ...)]` pragma;
/// these are the cases that make the claim observable rather than merely
/// asserted by a lint.
#[test]
fn the_pure_half_is_total_on_degenerate_input() {
    // A zero size or a zero zoom clamps to one rather than dividing by it.
    for (size, zoom) in [(0u32, 0u32), (0, 4), (4, 0), (1, 8)] {
        let two = noise_2d(size, zoom, 1);
        assert_eq!(two.len(), (size.max(1) as usize).pow(2) * 4);
        let three = noise_3d(size, zoom, 1);
        assert_eq!(three.len(), (size.max(1) as usize).pow(3) * 4);
    }

    let runtime = empty_runtime();
    // `decay` is deliberately not swept past zero: it reaches here from
    // `FrameOutputs::decay`, which is a saturated `per_second_factor` and so is
    // finite by construction. What is swept is what the *scene* hands over.
    let cases: [DegenerateCase; 8] = [
        ("a zero size", (0, 0), 1.0, 1.0 / 60.0, 0.98),
        ("a zero aspect", (64, 64), 0.0, 1.0 / 60.0, 0.98),
        ("a negative aspect", (64, 64), -2.0, 1.0 / 60.0, 0.98),
        ("a NaN aspect", (64, 64), f32::NAN, 1.0 / 60.0, 0.98),
        (
            "an infinite aspect",
            (64, 64),
            f32::INFINITY,
            1.0 / 60.0,
            0.98,
        ),
        ("a zero dt", (64, 64), 1.0, 0.0, 0.98),
        ("a NaN dt", (64, 64), 1.0, f32::NAN, 0.98),
        ("a negative decay", (64, 64), 1.0, 1.0 / 60.0, -1.0),
    ];
    for (what, size, aspect, dt, decay) in cases {
        let u = fill_uniform(&runtime, 0.0, dt, size, aspect, decay, 1.0, 0.0, 255.0);
        for (i, value) in all_lanes(&u).iter().enumerate() {
            assert!(value.is_finite(), "{what}: lane {i} came back {value}");
        }
        // The aspect pair stays invertible, which is what the reciprocal lanes
        // and every `rad`/`ang` in a converted shader rest on.
        assert!(
            u.aspect[0] > 0.0 && u.aspect[1] > 0.0,
            "{what}: aspect pair {:?}",
            u.aspect
        );
        assert!(u.misc[0] >= 0.0, "{what}: decay went negative");
    }
}

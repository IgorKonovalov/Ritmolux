// Tests index fixed-size arrays and panic on failure; allowed over the
// file's hot-path pragma — this is not the render path.
#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    ATTACK_FRAC, DEFAULT_SOURCE_Y, Field, Instance, Object, RETIRE_MARGIN, Spawn, bounds, build,
    exit_time, size_factor, sprite_angle, twinkle_factor,
};

/// A test aspect, and the retirement bound it gives.
const ASPECT: f32 = 16.0 / 9.0;

/// **The two particle scenes share one shape vocabulary** (Plan 0070 Phase
/// 2's done-when): the same parameter names, and the same set of `shape`
/// values, because the roster is one list in one place and both scenes go
/// through the same quantizer to reach it.
///
/// It lives here rather than in `marks.rs` because this is the file that can
/// see both scenes — and because the *second* adopter is where a drift would
/// be introduced.
///
/// The last claim is the one that would catch a scene quietly growing its
/// own roster: every value outside the list clamps to the same place, so a
/// private copy with a different ceiling fails here rather than rendering a
/// different figure on one of the two scenes.
#[test]
fn both_particle_scenes_carry_the_same_shape_vocabulary() {
    use crate::render::scenes::{marks, swarm};

    for name in marks::PARAMS {
        assert!(
            swarm::PARAMS.contains(&name),
            "the swarm must carry `{name}`"
        );
        assert!(
            super::PARAMS.contains(&name),
            "the emitter must carry `{name}`"
        );
    }

    for (index, name) in marks::SHAPES.iter().enumerate() {
        assert_eq!(
            marks::mark_shape(index as f32),
            index as f32,
            "`{name}` must survive the quantizer as {index}"
        );
    }
    assert_eq!(marks::mark_shape(-1.0), 0.0);
    assert_eq!(
        marks::mark_shape(marks::SHAPES.len() as f32),
        marks::SHAPES.len() as f32 - 1.0,
        "a shape past the roster clamps to its last entry, for both scenes"
    );
}

/// A spawn config with everything named, so each test says exactly what it
/// varies.
fn cfg(rate: f32, gravity: f32, speed: f32) -> Spawn {
    Spawn {
        rate,
        gravity,
        speed,
        angle: 0.0,
        spread: 0.0,
        lifetime: 4.0,
        lifetime_spread: 0.0,
        source_half_width: ASPECT,
        source_y: DEFAULT_SOURCE_Y,
        prewarm: 0.0,
        bound: bounds(ASPECT),
    }
}

/// **The closed form is the parabola it claims to be** (Phase 1's first
/// done-when).
///
/// An object launched straight up with speed `v0` against gravity `g`
/// reaches its apex at `t = v0 / g`, at height `v0^2 / (2 g)` above its spawn
/// point. Both are *derived*, not fitted: there is no integrator here, so the
/// only error is f32 rounding of the two multiply-adds the position costs,
/// and the tolerance below is a handful of ulps of the apex height rather
/// than a number chosen to make a run pass.
///
/// Two `(v0, g)` pairs, because one pair could coincide with an arithmetic
/// slip (a swapped factor of two agrees with the truth at `g = 2`).
#[test]
fn an_object_follows_the_closed_form_parabola() {
    for (v0, g) in [(1.75f32, 1.5f32), (3.2, 9.81)] {
        let object = Object {
            p0: [0.0, 0.0],
            v0: [0.0, v0],
            t0: 0.0,
            lifetime: 100.0,
            gravity: g,
            death_time: f32::INFINITY,
            seed: 0,
            alive: true,
        };
        let apex_t = v0 / g;
        let apex_h = v0 * v0 / (2.0 * g);
        // A few ulps of the height, which is the magnitude the arithmetic
        // rounds at. Not a tolerance in the fitted sense — the closed form is
        // exact in real arithmetic.
        let slack = 8.0 * f32::EPSILON * apex_h;

        let at_apex = object.position(apex_t)[1];
        assert!(
            (at_apex - apex_h).abs() <= slack,
            "apex height at v0={v0} g={g}: got {at_apex}, want {apex_h} (slack {slack:e})"
        );

        // ...and it really is the maximum: nothing on a fine sweep either
        // side of it goes higher.
        for i in 0..=2000 {
            let t = apex_t * 2.0 * (i as f32 / 2000.0);
            let y = object.position(t)[1];
            assert!(
                y <= apex_h + slack,
                "t={t} rises to {y}, above the apex {apex_h}"
            );
        }
        // The launch point and the symmetric return, which pin the other two
        // roots of the same quadratic.
        assert!(object.position(0.0)[1].abs() <= slack);
        assert!(
            object.position(2.0 * apex_t)[1].abs() <= 4.0 * slack,
            "a symmetric flight returns to its launch height"
        );
    }
}

/// Drive a field through an explicit list of **scene times** and report the
/// live population at the last one, keyed by spawn time so two runs can be
/// compared object for object.
fn population(times: &[f32], cfg: &Spawn, capacity: usize) -> Vec<(f32, [f32; 2])> {
    let mut field = Field::new(capacity);
    let mut last = 0.0;
    for &t in times {
        field.step(t, cfg);
        last = t;
    }
    let mut out: Vec<(f32, [f32; 2])> = field
        .objects
        .iter()
        .filter(|o| o.alive)
        .map(|o| (o.t0, o.position(last)))
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// **The trajectory is identical at different frame cadences** (Phase 1's
/// second done-when), and so is the population.
///
/// The claim ADR-0057 makes is structural rather than tuned, so it is
/// asserted as **exact equality** and not within a tolerance: position holds
/// no `dt`, spawn instants advance by the spawn period rather than by the
/// frame step, and retirement compares against a death time solved at spawn.
/// Nothing in the scene can see how the elapsed time was divided up.
///
/// A steady 60 Hz cadence against a deliberately ragged one — long frames,
/// short frames, a stall — both starting and ending at the same scene time.
#[test]
fn the_trajectory_is_the_same_under_any_frame_cadence() {
    const START: f32 = 0.02;
    const END: f32 = 4.0;
    let cfg = cfg(90.0, 1.5, 1.9);

    let steady: Vec<f32> = (0..=240)
        .map(|i| START + (END - START) * (i as f32 / 240.0))
        .collect();
    // Ragged: frame lengths swinging between a third and three times the
    // steady one, plus one long stall, landing on exactly the same end time.
    let ragged: Vec<f32> = {
        let mut times = vec![START];
        let mut t = START;
        let mut i = 0u32;
        while t < END {
            let step = match i % 5 {
                0 => 0.004,
                1 => 0.050,
                2 => 0.011,
                3 => 0.120,
                _ => 0.007,
            };
            t += step;
            if t < END {
                times.push(t);
            }
            i += 1;
        }
        times.push(END);
        times
    };
    assert!(
        ragged.len() * 2 < steady.len(),
        "the ragged cadence must genuinely differ from the steady one: \
         {} frames against {}",
        ragged.len(),
        steady.len()
    );

    let a = population(&steady, &cfg, 4096);
    let b = population(&ragged, &cfg, 4096);
    assert!(
        a.len() > 100,
        "the comparison must have a population to compare: {} objects",
        a.len()
    );
    assert_eq!(
        a, b,
        "the same scene time under two cadences must give the same objects \
         at the same places — the position is a closed form, so this is \
         exact, not approximate"
    );
}

/// **Objects genuinely leave** (Phase 1's third done-when) — the property the
/// swarm fails by construction, since ADR-0044's toroidal `bounds` wraps every
/// particle back into frame and its population is constant forever.
///
/// Spawn for a second, then switch the source off and run far past the
/// longest possible flight. A cascade empties; a torus does not. The
/// lower-half count is read at both ends because that is where a wrapped
/// object would have re-entered from the top and landed.
#[test]
fn objects_leave_the_frame_and_do_not_come_back() {
    let mut field = Field::new(4096);
    let on = cfg(200.0, 2.0, 1.4);
    let off = Spawn { rate: 0.0, ..on };

    let mut t = 0.0f32;
    let dt = 1.0 / 60.0;
    while t < 1.0 {
        t += dt;
        field.step(t, &on);
    }
    let populated = field.live;
    let lower_before = field
        .objects
        .iter()
        .filter(|o| o.alive && o.position(t)[1] < 0.0)
        .count();
    assert!(
        populated > 50 && lower_before > 5,
        "the field must fill before it drains, got {populated} live \
         ({lower_before} in the lower half)"
    );

    // Source off. Every object's lifetime is 4 s and the fastest flight is
    // far shorter, so ten seconds is well past the last possible retirement.
    let mut previous = field.live;
    while t < 11.0 {
        t += dt;
        field.step(t, &off);
        assert!(
            field.live <= previous,
            "with the source off the population may only fall; it went \
             {previous} -> {} at t={t}",
            field.live
        );
        previous = field.live;
    }

    assert_eq!(
        field.live, 0,
        "every object must have been retired — a toroidal world would still \
         hold all {populated} of them"
    );
    let lower_after = field
        .objects
        .iter()
        .filter(|o| o.alive && o.position(t)[1] < 0.0)
        .count();
    assert_eq!(
        lower_after, 0,
        "the frame's lower half was replenished from the top: {lower_after} \
         objects, from {lower_before}"
    );
}

/// **The pool cannot be overrun** (Phase 1's fourth done-when) — the phase's
/// real-time hazard, made unrepresentable rather than merely unlikely.
///
/// A `spawn_rate` demanding two orders of magnitude more objects per second
/// than the pool holds. The live count saturates at capacity, the vectors
/// behind it never reallocate (their capacities are read before and after),
/// and nothing panics.
#[test]
fn the_pool_saturates_instead_of_overrunning() {
    const CAPACITY: usize = 256;
    let mut field = Field::new(CAPACITY);
    let objects_capacity = field.objects.capacity();
    let free_capacity = field.free.capacity();

    // 100 000 objects a second against a 256-slot pool with a 4 s lifetime:
    // the demand is ~1560x what the pool can hold.
    let cfg = cfg(100_000.0, 1.5, 1.9);
    let mut t = 0.0f32;
    for _ in 0..60 {
        t += 1.0 / 60.0;
        field.step(t, &cfg);
        assert!(
            field.live <= CAPACITY,
            "the pool went over capacity: {} live in {CAPACITY} slots",
            field.live
        );
    }

    assert_eq!(
        field.live, CAPACITY,
        "an unbounded spawn rate must saturate the pool, not fall short of it"
    );
    assert_eq!(field.objects.len(), CAPACITY, "the pool must not grow");
    assert!(field.free.is_empty(), "a saturated pool has no free slots");
    assert_eq!(
        (field.objects.capacity(), field.free.capacity()),
        (objects_capacity, free_capacity),
        "neither the pool nor its free list may reallocate — this is the \
         hot path, and a spawn is the one place it could"
    );

    // And it drains again rather than latching: the dropped spawns left no
    // state behind.
    let off = Spawn { rate: 0.0, ..cfg };
    while t < 12.0 {
        t += 1.0 / 60.0;
        field.step(t, &off);
    }
    assert_eq!(field.live, 0, "a saturated pool must still drain");
    assert_eq!(field.free.len(), CAPACITY);
}

/// The retirement bound is the **render target's** shape, scaled outward —
/// never a square, and never an internal grid's aspect (ADR-0037).
#[test]
fn the_retirement_bound_takes_its_shape_from_the_target() {
    for aspect in [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0, 9.0 / 16.0] {
        let b = bounds(aspect);
        assert!(
            (b[0] / b[1] - aspect).abs() < 1e-5,
            "bound shape {:.4} must equal the target's {aspect:.4}",
            b[0] / b[1]
        );
        assert!(b[1] > 1.0, "the bound must sit outside the visible frame");
    }
}

/// The death-time solve, against the three shapes of path it has to answer
/// for. This is what makes retirement cadence-independent, so it is asserted
/// directly rather than only through the population comparison above.
#[test]
fn the_exit_time_is_the_last_crossing_not_the_first() {
    let b = bounds(ASPECT);

    // Up and over: the object arcs *above* the top bound and comes back. A
    // first-crossing answer would retire it in mid-air; the true exit is when
    // it finally falls past the bottom.
    let g = 1.0f32;
    let v0 = [0.0f32, 3.0f32];
    let p0 = [0.0f32, DEFAULT_SOURCE_Y];
    let t = exit_time(p0, v0, g, b);
    let apex = v0[1] / g;
    assert!(
        t > apex,
        "the exit must come after the apex at {apex}, got {t}"
    );
    let y_at_exit = p0[1] + v0[1] * t - 0.5 * g * t * t;
    assert!(
        (y_at_exit + b[1]).abs() < 1e-3,
        "the exit must land on the bottom bound {}, got {y_at_exit}",
        -b[1]
    );
    // It really did leave the frame at the top on the way, which is the case
    // a sampled bound would have got wrong.
    assert!(
        p0[1] + v0[1] * apex - 0.5 * g * apex * apex > RETIRE_MARGIN,
        "this path must genuinely go above the bound, or it proves nothing"
    );

    // Sideways: linear in t, so the side exit is exact and permanent.
    let t_side = exit_time([0.0, 0.0], [2.0, 0.0], 0.0, b);
    assert!(
        (t_side - b[0] / 2.0).abs() < 1e-5,
        "a horizontal path leaves at bound_x / speed: {t_side}"
    );

    // No gravity, no downward speed, no side speed: it never leaves, and only
    // the lifetime retires it.
    assert_eq!(exit_time([0.0, 0.0], [0.0, 1.0], 0.0, b), f32::INFINITY);
}

/// A spawn draws its source position from its **seed**, so an object's whole
/// description is a pure function of `(seed, spawn time, config)` — and the
/// source line spans the target's width rather than a square.
#[test]
fn a_spawn_is_a_pure_function_of_its_seed() {
    let cfg = cfg(60.0, 1.5, 1.9);
    for seed in [0u32, 1, 7, 0xDEAD_BEEF, u32::MAX] {
        let a = build(seed, 0.25, &cfg);
        let b = build(seed, 0.25, &cfg);
        assert_eq!(a.p0, b.p0);
        assert_eq!(a.v0, b.v0);
        assert_eq!(a.death_time, b.death_time);
        assert!(
            a.p0[0].abs() <= cfg.source_half_width,
            "the source line spans the frame width, got x={}",
            a.p0[0]
        );
        assert_eq!(a.p0[1], DEFAULT_SOURCE_Y);
    }
}

// -----------------------------------------------------------------------
// The source is geometry a preset owns (Plan 0090 Phase 1, ADR-0104)
// -----------------------------------------------------------------------

/// **The defaults are the line this scene shipped with, bit for bit** — Phase
/// 1's first done-when, and the whole reason `source_width` is a *fraction*.
///
/// Asserted as exact equality rather than within a tolerance, because that is
/// the claim: `aspect * 1.0` is the identity in IEEE-754, so the resolved
/// half-width is the same float `self.aspect` was, and the one committed
/// emitter baseline cannot move by arithmetic. A tolerance here would pass for
/// an absolute width that merely *approximated* the frame, which is the
/// alternative ADR-0104 rejected (F).
#[test]
fn the_default_source_geometry_is_the_line_the_scene_shipped_with() {
    for aspect in [
        16.0f32 / 9.0,
        16.0 / 10.0,
        4.0 / 3.0,
        21.0 / 9.0,
        9.0 / 16.0,
    ] {
        assert_eq!(
            super::source_half_width(aspect, super::DEFAULT_SOURCE_WIDTH),
            aspect,
            "the default source line must be the frame's own half-width, exactly"
        );
        assert_eq!(
            super::source_line_y(DEFAULT_SOURCE_Y, bounds(aspect)),
            -1.12,
            "the default source y must be the constant this scene shipped with"
        );
    }
}

/// **`source_width = 0` is a point source**, asserted exactly across seeds.
///
/// The spawn site multiplies a unit draw by the half-width, so zero collapses
/// every draw onto the same `x` — there is no distribution left to be nearly
/// zero, which is why this is `== 0.0` and not a bound on the spread.
///
/// The negative arm is the same clamp `lifetime_spread` takes: a width is only
/// meaningful as a magnitude, and a negative one would mirror the seed's draw
/// rather than narrow it.
#[test]
fn a_zero_source_width_is_a_point_source() {
    for width in [0.0f32, -0.5, -3.0] {
        let half = super::source_half_width(ASPECT, width);
        assert_eq!(half, 0.0, "source_width = {width} must collapse the line");
        let cfg = Spawn {
            source_half_width: half,
            ..cfg(60.0, 1.5, 1.9)
        };
        for seed in [0u32, 1, 7, 0xDEAD_BEEF, u32::MAX] {
            assert_eq!(
                build(seed, 0.25, &cfg).p0[0],
                0.0,
                "every object must leave from x = 0 exactly at source_width = {width}"
            );
        }
    }
    // ...and the property is falsifiable: the same seeds do scatter at a width.
    let wide = cfg(60.0, 1.5, 1.9);
    let xs: Vec<f32> = [0u32, 1, 7, 0xDEAD_BEEF, u32::MAX]
        .iter()
        .map(|&seed| build(seed, 0.25, &wide).p0[0])
        .collect();
    assert!(
        xs.windows(2).any(|w| w[0] != w[1]),
        "the full-width source must scatter its spawns, or the point-source \
         assertion above holds vacuously: {xs:?}"
    );
}

/// **A source outside the retirement bound is clamped to it, and the objects
/// it spawns are alive for at least a frame** — Phase 1's second done-when.
///
/// The failure mode this guards is invisible rather than ugly: an object
/// spawned past the bound has an `exit_time` that has *already* passed, so it
/// is retired on the frame it was born on and the pool churns against itself
/// forever, drawing nothing. So the assertion is on the death time, which is
/// where the churn would be, and not on a pixel.
#[test]
fn a_source_past_the_bound_is_clamped_and_still_spawns_live_objects() {
    let bound = bounds(ASPECT);
    const FRAME: f32 = 1.0 / 60.0;

    for asked in [9.0f32, -9.0, f32::INFINITY, f32::NAN] {
        let y = super::source_line_y(asked, bound);
        assert!(
            y.abs() <= bound[1],
            "source_y = {asked} must land inside the retirement bound, got {y}"
        );
        let cfg = Spawn {
            source_y: y,
            ..cfg(60.0, 1.5, 1.9)
        };
        for seed in [0u32, 1, 7, 0xDEAD_BEEF, u32::MAX] {
            let object = build(seed, 0.25, &cfg);
            assert!(
                object.death_time > 0.25 + FRAME,
                "an object spawned from source_y = {asked} (clamped to {y}) \
                 must live at least a frame, not be born dead: death_time \
                 {} against spawn 0.25",
                object.death_time
            );
        }
    }
    // The clamp lands *on* the bound rather than somewhere inside it, which is
    // what makes the reachable range the whole of the legal one.
    assert_eq!(super::source_line_y(9.0, bound), bound[1]);
    assert_eq!(super::source_line_y(-9.0, bound), -bound[1]);
    // A source inside the visible frame is legal and is NOT clamped — the
    // decision ADR-0104 took (C), and the one that makes a slow look reachable.
    assert_eq!(super::source_line_y(0.0, bound), 0.0);
    assert_eq!(super::source_line_y(0.6, bound), 0.6);
}

/// **A narrowed source is visible in a render, and `pan_x` carries it
/// off-centre** — Phase 1's third done-when, stated as a property of the lit
/// pixels rather than as a frozen count.
///
/// Two captures of one fixture, differing only in `source_width`. The narrow
/// one has to put its ejecta in a column narrower than the frame; the wide one
/// is the control that makes that non-vacuous, because a capture that drew
/// almost nothing would satisfy the narrow arm on its own. `spread` is closed
/// for both so the horizontal extent is the *source's* and not the cone's.
///
/// The off-centre jet is the narrow source plus the scene's own `pan_x` — the
/// two together are the look `presets/README.md` used to route to engine
/// feedback, so the third capture asserts the column actually moves.
///
/// Needs a GPU adapter, so it skips where there is none (ADR-0016).
#[test]
fn a_narrow_source_draws_a_column_and_pan_carries_it_off_centre() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::CaptureImage;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const FIXTURE: &str = include_str!("../../../../tests/fixtures/emitter.toml");
    const SIZE: u32 = 96;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };

    /// The lit columns of a capture, as fractions of the frame width: the
    /// leftmost, the rightmost, and the mean.
    fn lit_columns(img: &CaptureImage, size: u32) -> Option<(f32, f32, f32)> {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut sum = 0.0f64;
        let mut count = 0u32;
        for (i, p) in img.rgba.chunks_exact(4).enumerate() {
            if p[0] <= 8 && p[1] <= 8 && p[2] <= 8 {
                continue;
            }
            let x = (i as u32 % size) as f32 / (size - 1) as f32;
            lo = lo.min(x);
            hi = hi.max(x);
            sum += x as f64;
            count += 1;
        }
        (count > 0).then(|| (lo, hi, (sum / count as f64) as f32))
    }

    // Capture the fixture with `extra` appended to its `[params]` table.
    let capture = |renderer: &mut Renderer, extra: &str| -> CaptureImage {
        let toml = format!("{FIXTURE}\nspread = \"0\"\n{extra}");
        let preset =
            Preset::from_toml_str(&toml).expect("the emitter fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(&name, &AnalysisFrame::default(), 45)
            .expect("capture the emitter column")
    };

    let wide = capture(&mut renderer, "source_width = \"1\"\n");
    let narrow = capture(&mut renderer, "source_width = \"0.1\"\n");
    let panned = capture(&mut renderer, "source_width = \"0.1\"\npan_x = \"0.5\"\n");

    let (wide_lo, wide_hi, _) = lit_columns(&wide, SIZE).expect("the wide source drew nothing");
    let (narrow_lo, narrow_hi, narrow_mid) =
        lit_columns(&narrow, SIZE).expect("the narrow source drew nothing");
    let (_, _, panned_mid) = lit_columns(&panned, SIZE).expect("the panned source drew nothing");
    eprintln!(
        "emitter source: wide span {:.3}, narrow span {:.3}, centre {narrow_mid:.3} -> \
         {panned_mid:.3} under pan_x",
        wide_hi - wide_lo,
        narrow_hi - narrow_lo
    );

    // The control: a full-width source really does span the frame, so the
    // narrow arm below is measuring the source and not an empty capture.
    assert!(
        wide_hi - wide_lo > 0.8,
        "the full-width source must span the frame: {:.3}",
        wide_hi - wide_lo
    );
    // A tenth of the frame's half-width, plus a mark's own radius either side.
    // Half the frame is the bar rather than the arithmetic minimum, because the
    // claim is "a column" and not a pixel count.
    assert!(
        narrow_hi - narrow_lo < 0.5,
        "source_width = 0.1 must put the ejecta in a column: {:.3} of the frame",
        narrow_hi - narrow_lo
    );
    // ...and the column is where the source is, not merely small.
    assert!(
        (narrow_mid - 0.5).abs() < 0.1,
        "the centred column must sit at the middle of the frame: {narrow_mid:.3}"
    );
    assert!(
        panned_mid - narrow_mid > 0.15,
        "pan_x must carry the column off centre — this is the off-centre jet: \
         {narrow_mid:.3} -> {panned_mid:.3}"
    );
}

// -----------------------------------------------------------------------
// `spawn_fade` — the ramp that makes an inside-frame source usable
// (Plan 0090 Phase 2, ADR-0104)
// -----------------------------------------------------------------------

/// **The default is exactly `1.0`, at every age including zero** — Phase 2's
/// first done-when, and the reason [`super::spawn_ramp`] takes an equality
/// branch rather than dividing.
///
/// `age = 0` is the case that forces it: the natural arithmetic is `u / fade`,
/// which is `0/0` at the default and would give a NaN that propagates into a
/// colour. The exactness matters beyond that one point — the whole "no pixel
/// moves" claim of this plan is that the product picks up a factor of exactly
/// one, and `x * 1.0` is `x` in IEEE-754 while `x * 0.9999999` is not.
///
/// The hostile values are the same discipline every other param on this scene
/// gets: a binding is arbitrary arithmetic and may hand over anything.
#[test]
fn a_zero_spawn_fade_is_exactly_one_at_every_age() {
    for u in [0.0f32, 1e-9, 0.001, ATTACK_FRAC, 0.5, 1.0, 2.0] {
        assert_eq!(
            super::spawn_ramp(u, 0.0),
            1.0,
            "the ramp must be exactly 1 at u={u} with the fade off"
        );
        // Negative and NaN reach the ramp only through the draw site's
        // `finite(..).clamp(0, 1)`, so both arrive as the default — and the
        // branch below the clamp holds for a negative anyway.
        assert_eq!(super::spawn_ramp(u, -0.5), 1.0);
        assert_eq!(
            super::spawn_ramp(u, super::finite(f32::NAN, 0.0).clamp(0.0, 1.0)),
            1.0
        );
        // Above 1 there is no more life to ramp over, so it clamps to a ramp
        // spanning the whole of one.
        assert_eq!(
            super::spawn_ramp(u, super::finite(4.0, 0.0).clamp(0.0, 1.0)),
            u.clamp(0.0, 1.0)
        );
    }
}

/// **The ramp climbs, and it climbs the right way** — the pure half of Phase
/// 2's second done-when. An inverted ramp passes any single-cohort assertion,
/// so the claim is stated as an ordering between a young age and an old one.
#[test]
fn the_spawn_ramp_is_monotone_from_dark_to_full() {
    const FADE: f32 = 0.5;
    assert_eq!(super::spawn_ramp(0.0, FADE), 0.0, "it starts at black");
    assert_eq!(
        super::spawn_ramp(FADE, FADE),
        1.0,
        "and reaches full exactly at the end of the ramp"
    );
    let mut previous = -1.0f32;
    for i in 0..=200 {
        let u = i as f32 / 200.0;
        let ramp = super::spawn_ramp(u, FADE);
        assert!(
            ramp >= previous,
            "the ramp must never fall: {ramp} at u={u} after {previous}"
        );
        assert!((0.0..=1.0).contains(&ramp));
        previous = ramp;
    }
    assert!(
        super::spawn_ramp(0.1, FADE) < super::spawn_ramp(0.4, FADE),
        "a young object must be dimmer than an older one, which is the arm an \
         inverted ramp fails"
    );
}

/// **The fade is visible, and it dims the young cohort rather than the old
/// one** — the rendered half of Phase 2's second done-when.
///
/// The cohorts are read off the *geometry*, which is what makes this a
/// statement about two populations rather than about one number: with `spread`
/// and `lifetime_spread` closed, every object rides the same rising parabola,
/// so an object's height in frame **is** its age. The band nearest the source
/// therefore holds the objects inside the ramp and the far band holds the ones
/// past it.
///
/// Both directions are asserted, and each rules out a different mistake: the
/// young band must lose a fifth of its light (the ramp does something), and the
/// old band must keep nine tenths of its own (the ramp is a *fade-in*, not a
/// global dimmer — and an inverted ramp fails exactly here, since it would
/// darken the objects that have lived longest).
///
/// Needs a GPU adapter, so it skips where there is none (ADR-0016).
#[test]
fn a_spawn_fade_dims_the_young_cohort_and_leaves_the_old_one() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::CaptureImage;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const FIXTURE: &str = include_str!("../../../../tests/fixtures/emitter.toml");
    const SIZE: u32 = 96;
    /// Short enough that a `spawn_fade = 0.5` ramp — 0.3 s of a 0.6 s life —
    /// finishes well inside the capture, so there is an *old* cohort to compare
    /// against at all. Both distributions are closed so that height reads as age.
    const OVERRIDES: &str = "spread = \"0\"\nlifetime_spread = \"0\"\n";
    const LIFETIME: &str = "0.6";

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };

    // Mean level of the third of the frame nearest the source line (the young
    // cohort) and of the third furthest from it (the old one). Row 0 is the
    // top of the frame; the source is below it, so the young band is the last
    // third of the rows.
    fn cohorts(img: &CaptureImage, size: u32) -> (f32, f32) {
        let mut old = 0.0f64;
        let mut young = 0.0f64;
        for (i, p) in img.rgba.chunks_exact(4).enumerate() {
            let level = (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0;
            let row = i as u32 / size;
            if row < size / 3 {
                old += level;
            } else if row >= size - size / 3 {
                young += level;
            }
        }
        let per_band = (size * (size / 3)) as f64;
        ((old / per_band) as f32, (young / per_band) as f32)
    }

    // TOML has no override, so the fixture's own `lifetime` line is stripped
    // and rewritten — the same shape the lit-backdrop guard below uses.
    let body: Vec<&str> = FIXTURE
        .lines()
        .filter(|line| !line.trim_start().starts_with("lifetime"))
        .collect();
    let body = body.join("\n");

    let capture = |renderer: &mut Renderer, fade: &str| -> CaptureImage {
        let toml =
            format!("{body}\nlifetime = \"{LIFETIME}\"\n{OVERRIDES}spawn_fade = \"{fade}\"\n");
        let preset =
            Preset::from_toml_str(&toml).expect("the emitter fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(&name, &AnalysisFrame::default(), 45)
            .expect("capture the faded emitter")
    };

    let (base_old, base_young) = cohorts(&capture(&mut renderer, "0"), SIZE);
    let (faded_old, faded_young) = cohorts(&capture(&mut renderer, "0.5"), SIZE);
    eprintln!(
        "emitter spawn_fade: young {base_young:.4} -> {faded_young:.4}, \
         old {base_old:.4} -> {faded_old:.4} (of 255)"
    );

    assert!(
        base_young > 0.5 && base_old > 0.5,
        "both cohorts must be lit without the fade, or the comparison is two \
         dark bands agreeing: young {base_young:.4}, old {base_old:.4}"
    );
    assert!(
        faded_young < 0.8 * base_young,
        "the objects still inside the ramp must be visibly dimmer: \
         {base_young:.4} -> {faded_young:.4}"
    );
    assert!(
        faded_old > 0.9 * base_old,
        "the objects past the ramp must be untouched — a fade-in, not a \
         dimmer, and the arm an inverted ramp fails: {base_old:.4} -> \
         {faded_old:.4}"
    );
}

// -----------------------------------------------------------------------
// `prewarm` — the pool can start in its steady state
// (Plan 0090 Phase 3, ADR-0104)
// -----------------------------------------------------------------------

/// **A prewarmed pool is indistinguishable from one that has been running** —
/// Phase 3's first done-when, asserted as exact equality against a field
/// stepped normally from the back-dated instant.
///
/// This is available *because* of the shape ADR-0057 chose: the path is closed
/// form in `t - t0` and the death time derives from `t0`, so back-dating is a
/// substitution rather than a simulation. If it did not hold exactly, the
/// back-dating would be wrong and the honest response is to fix it rather than
/// to loosen this into a tolerance.
///
/// The comparison is by `(spawn time, position)` sorted on spawn time — the
/// same shape `the_trajectory_is_the_same_under_any_frame_cadence` uses —
/// because the two fields reach the same population through different free-list
/// histories, and a slot index is not part of the claim.
#[test]
fn a_prewarmed_pool_matches_one_that_actually_ran() {
    const T: f32 = 7.5;
    const PREWARM: f32 = 1.0;

    for (rate, gravity, speed, lifetime_spread) in
        [(60.0f32, 1.5f32, 1.9f32, 0.0f32), (200.0, 0.4, 0.9, 0.45)]
    {
        let warm = Spawn {
            prewarm: PREWARM,
            lifetime_spread,
            ..cfg(rate, gravity, speed)
        };
        let cold = Spawn {
            prewarm: 0.0,
            ..warm
        };

        // The prewarmed field, at its very first step.
        let mut prewarmed = Field::new(8192);
        prewarmed.step(T, &warm);

        // The control: the same config, stepped at 60 Hz from the instant the
        // prewarm back-dates to, landing on exactly the same scene time.
        let start = T - PREWARM * warm.lifetime;
        let mut times = vec![start];
        let mut t = start;
        while t < T {
            t += 1.0 / 60.0;
            times.push(t.min(T));
        }
        if times.last().copied() != Some(T) {
            times.push(T);
        }
        let mut ran = Field::new(8192);
        for &t in &times {
            ran.step(t, &cold);
        }

        let live = |field: &Field| -> Vec<(f32, [f32; 2])> {
            let mut out: Vec<(f32, [f32; 2])> = field
                .objects
                .iter()
                .filter(|o| o.alive)
                .map(|o| (o.t0, o.position(T)))
                .collect();
            out.sort_by(|a, b| a.0.total_cmp(&b.0));
            out
        };
        let (warm_pop, ran_pop) = (live(&prewarmed), live(&ran));
        assert!(
            warm_pop.len() > 50,
            "the comparison needs a population: {} objects at rate {rate}",
            warm_pop.len()
        );
        assert_eq!(
            warm_pop, ran_pop,
            "a pool prewarmed to {T} must hold exactly the objects a pool \
             stepped from {start} to {T} holds — same spawn instants, same \
             seeds, same closed-form positions"
        );
    }
}

/// **`prewarm = 0` leaves the pool starting empty**, which is today's behaviour
/// and the reason this is a param rather than a fix.
///
/// The rendered half of this claim is
/// [`a_spawn_rate_on_onset_bursts_and_then_idles`], which asserts an *empty*
/// frame before its transient and is left unmodified by this plan — a
/// default-on prewarm would break it. Here is the pool-level statement: nothing
/// is back-dated, so the first step holds only what that instant is due.
#[test]
fn a_zero_prewarm_starts_the_pool_empty() {
    let cold = cfg(600.0, 1.5, 1.9);
    let mut field = Field::new(4096);
    field.step(4.0, &cold);
    assert_eq!(
        field.live, 1,
        "an unprewarmed pool starts from the one spawn its first instant is \
         due, not from a population"
    );

    // ...and the same field one lifetime later is the steady state the prewarm
    // reaches immediately, which is what makes the claim above non-vacuous.
    let mut t = 4.0f32;
    while t < 4.0 + cold.lifetime {
        t += 1.0 / 60.0;
        field.step(t, &cold);
    }
    let settled = field.live;
    let mut warm = Field::new(4096);
    warm.step(
        4.0,
        &Spawn {
            prewarm: 1.0,
            ..cold
        },
    );
    let ratio = warm.live as f32 / settled as f32;
    eprintln!(
        "emitter prewarm: {} live at once against {settled} settled",
        warm.live
    );
    assert!(
        (0.95..=1.05).contains(&ratio),
        "a prewarmed pool must start at the population a lifetime of running \
         reaches: {} against {settled}",
        warm.live
    );
}

/// **A hostile `prewarm` costs bounded work and cannot outrun the pool** — the
/// same discipline `spawn_rate` gets from [`MAX_SPAWN_RATE`].
///
/// The ceiling is not a look value: back-dating past the longest life anything
/// can have would build objects only to drop them, so the window is clipped
/// there, and the loop is capped at one pool's worth of spawns however deep the
/// prewarm asked to go.
#[test]
fn a_hostile_prewarm_is_bounded() {
    for prewarm in [1e9f32, f32::INFINITY, f32::NAN, -4.0] {
        let mut field = Field::new(512);
        // The back-dated fill on its own, not a whole `step`: the live spawn
        // loop that follows it may leave an object that died inside the frame
        // it was spawned in, which is this scene's existing behaviour (the next
        // `retire` takes it) and not something the prewarm is answerable for.
        field.prewarm(
            9.0,
            &Spawn {
                prewarm: super::finite(prewarm, 0.0).clamp(0.0, super::MAX_PREWARM),
                ..cfg(4000.0, 1.5, 1.9)
            },
        );
        assert!(
            field.live <= field.capacity(),
            "prewarm = {prewarm} overran the pool: {} of {}",
            field.live,
            field.capacity()
        );
        for object in field.objects.iter().filter(|o| o.alive) {
            assert!(
                object.death_time > 9.0,
                "a prewarmed pool must hold no object that is already dead: \
                 death {} at time 9.0",
                object.death_time
            );
        }
    }
}

// -----------------------------------------------------------------------
// Individuation (Plan 0052 Phase 2)
// -----------------------------------------------------------------------

/// Run a field to `time` and return the live objects' launch angles,
/// measured the way `build` writes them: clockwise from straight up.
fn live_launch_angles(cfg: &Spawn, time: f32) -> Vec<f32> {
    let mut field = Field::new(4096);
    let mut t = 0.0f32;
    while t < time {
        t += 1.0 / 60.0;
        field.step(t, cfg);
    }
    field
        .objects
        .iter()
        .filter(|o| o.alive)
        .map(|o| o.v0[0].atan2(o.v0[1]))
        .collect()
}

/// **Objects alive at the same instant differ from each other** — Phase 2's
/// first done-when — **and `spread = 0` collapses it exactly**, which is its
/// second and what makes the first non-vacuous in both directions.
///
/// Stated as the property (the population varies, and stops varying when the
/// distribution is closed) rather than as a distribution statistic: this plan
/// measured neither the shape of the draw nor what a "correct" variance would
/// be, so pinning one would be inventing a number.
#[test]
fn objects_alive_at_one_instant_differ_and_a_zero_spread_collapses_them() {
    const SPREAD: f32 = 0.8;
    let open = Spawn {
        spread: SPREAD,
        ..cfg(200.0, 2.0, 1.9)
    };
    let closed = Spawn {
        spread: 0.0,
        ..open
    };

    let angles = live_launch_angles(&open, 1.5);
    assert!(
        angles.len() > 100,
        "the population must be worth measuring: {} objects",
        angles.len()
    );
    let lo = angles.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = angles.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    // The cone is `angle +/- spread/2` about the base, so the observed range
    // cannot exceed `spread` — and with a few hundred draws it should very
    // nearly fill it. Both directions, so a draw that ignored `spread` and a
    // draw that overshot it both fail.
    assert!(
        hi - lo > SPREAD * 0.8,
        "the live set must genuinely fan out: range {:.4} of a {SPREAD} cone",
        hi - lo
    );
    assert!(
        hi - lo <= SPREAD + 1e-5,
        "no object may launch outside the cone: range {:.4}",
        hi - lo
    );

    // Closed: every object on exactly the base angle, bit for bit. Not "within
    // a tolerance" — the draw is multiplied by zero.
    let collapsed = live_launch_angles(&closed, 1.5);
    assert!(!collapsed.is_empty());
    for angle in &collapsed {
        assert_eq!(
            *angle, 0.0,
            "at spread = 0 every object launches on the base angle"
        );
    }

    // The other two distributions collapse the same way, and open the same
    // way — asserted on the pure functions, since they are resolved at draw.
    let seeds: Vec<u32> = (0..512).map(unit_seed).collect();
    for &s in &seeds {
        assert_eq!(size_factor(s, 0.0), 1.0);
        assert_eq!(twinkle_factor(s, 3.7, 0.0), 1.0);
        assert_eq!(sprite_angle(s, 2.0, 0.0), sprite_angle(s, 5.0, 0.0));
    }
    let sizes: Vec<f32> = seeds.iter().map(|&s| size_factor(s, 0.7)).collect();
    let twinkles: Vec<f32> = seeds.iter().map(|&s| twinkle_factor(s, 3.7, 0.8)).collect();
    let spins: Vec<f32> = seeds
        .iter()
        .map(|&s| sprite_angle(s, 5.0, 1.0) - sprite_angle(s, 2.0, 1.0))
        .collect();
    for (label, series) in [("size", &sizes), ("twinkle", &twinkles), ("spin", &spins)] {
        let lo = series.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = series.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            hi - lo > 1e-3,
            "{label} must vary across the population: {lo} .. {hi}"
        );
    }
}

/// A spread of distinct seeds, mixed rather than sequential so the test does
/// not depend on `unit`'s behaviour on adjacent inputs.
fn unit_seed(i: u32) -> u32 {
    i.wrapping_mul(2_654_435_761) ^ 0x9E37_79B9
}

/// **A twinkling field does not flash as one sheet** — Phase 2's last
/// done-when, and the whole point of drawing the twinkle *rate* per object
/// rather than only its phase.
///
/// Dimensionless and self-normalizing: the whole-frame mean's swing over a
/// window, against the mean swing of its individual members over the same
/// window. A field whose members shared one rate would score ~1 however their
/// phases were scattered; independent rates make the mean nearly flat while
/// every member still swings its full amplitude. The factor asserted is 8x,
/// far inside the measured behaviour and far outside the failure it names.
#[test]
fn a_twinkling_field_does_not_flash_as_one_sheet() {
    const TWINKLE: f32 = 0.9;
    let seeds: Vec<u32> = (0..600).map(unit_seed).collect();
    let times: Vec<f32> = (0..600).map(|i| i as f32 / 60.0).collect();

    let swing = |series: &[f32]| {
        let lo = series.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = series.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        hi - lo
    };

    let field_mean: Vec<f32> = times
        .iter()
        .map(|&t| {
            seeds
                .iter()
                .map(|&s| twinkle_factor(s, t, TWINKLE))
                .sum::<f32>()
                / seeds.len() as f32
        })
        .collect();
    let member_swing: f32 = seeds
        .iter()
        .map(|&s| {
            let series: Vec<f32> = times
                .iter()
                .map(|&t| twinkle_factor(s, t, TWINKLE))
                .collect();
            swing(&series)
        })
        .sum::<f32>()
        / seeds.len() as f32;
    let field_swing = swing(&field_mean);

    // Non-vacuity first: the members must actually be twinkling, or a field
    // of frozen objects would pass trivially.
    assert!(
        member_swing > TWINKLE,
        "each object must swing over the window: mean swing {member_swing:.4}"
    );
    assert!(
        field_swing * 8.0 < member_swing,
        "the whole-frame mean swings {field_swing:.4} against a member's \
         {member_swing:.4} — the field is flashing as one sheet"
    );
}

/// **The scene reads no clock** (NFR §6), asserted against the source rather
/// than inferred from a pair of captures that happened to agree.
///
/// The reproducibility this plan claims is not "the seed is fixed" — it is
/// that the scene is a pure function of `(seed, scene time)`, and the one way
/// to break that without touching the seed is to reach for wall-clock time.
/// A capture comparison cannot see a clock read that is merely coarse.
#[test]
fn the_scene_reads_no_wall_clock() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render/scenes/emitter.rs");
    let text = std::fs::read_to_string(&path).expect("read the emitter source");
    // Everything before the test module: the scene proper.
    let scene = text
        .split_once("#[cfg(test)]")
        .map(|(before, _)| before)
        .unwrap_or(&text);
    for forbidden in ["Instant::now", "SystemTime", "elapsed()", "rand::"] {
        assert!(
            !scene.contains(forbidden),
            "the emitter scene names `{forbidden}` — its whole determinism \
             claim is that a frame is a pure function of (seed, scene time)"
        );
    }
}

/// **The whole scene is reproducible**: two captures of the same preset at
/// the same scene times are byte-identical.
///
/// End to end through the real renderer rather than through the pool, because
/// the claim covers the drawing too — the palette sample, the envelope, the
/// twinkle and the sprite orientation are all per-object functions of the
/// seed, and a stray frame counter in any of them would show up here.
///
/// Needs a GPU adapter, so it skips where there is none (ADR-0016).
#[test]
fn two_captures_of_the_same_preset_are_byte_identical() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const FIXTURE: &str = include_str!("../../../../tests/fixtures/emitter.toml");

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 96,
        height: 96,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    // Twinkle and spin on, so the reproducibility claim covers the two
    // per-object quantities that vary *with time* and not only the ones fixed
    // at spawn. Appended to the fixture's `[params]`, which is its last table.
    let toml = format!("{FIXTURE}\ntwinkle = \"0.7\"\nspin = \"2.5\"\n");
    let preset =
        Preset::from_toml_str(&toml).expect("the emitter golden fixture parses with overrides");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset(&name, &frame, 45)
        .expect("first capture");
    let b = renderer
        .capture_preset(&name, &frame, 45)
        .expect("second capture");
    assert_eq!(
        a.rgba, b.rgba,
        "two captures at the same scene times must be byte-identical"
    );
    // ...and the capture is not blank, or the equality above is two black
    // frames agreeing.
    assert!(
        a.rgba
            .chunks_exact(4)
            .any(|p| p[0] > 8 || p[1] > 8 || p[2] > 8),
        "the reproducibility capture drew nothing"
    );
}

// -----------------------------------------------------------------------
// Audio (Plan 0052 Phase 3)
// -----------------------------------------------------------------------

/// **A `spawn_rate` bound to `onset` emits in bursts and idles between them**
/// — Phase 3's first done-when, and the one claim `capture_preset` cannot
/// make: holding one analysis frame for every step converges the response
/// before the pixels are read, so a *sustained* onset shows that the binding
/// is live but says nothing about what happens when it stops.
///
/// So this drives `capture_preset_over` with a real time-varying stimulus —
/// three frames of transient, then a second of silence — and reads the whole
/// response. Three things have to hold and each rules out a different
/// mistake: the frame is dark before the hit (the source really is
/// `onset`-only), it lights up sharply after it (the binding reaches the
/// pool), and it goes dark again (objects are *retired*, which is what a
/// scene with lifetimes does and what the swarm cannot do at all).
///
/// The fixture binds no `trails`, so the decay measured here is the
/// population emptying and not a feedback stage fading.
///
/// Needs a GPU adapter, so it skips where there is none (ADR-0016).
#[test]
fn a_spawn_rate_on_onset_bursts_and_then_idles() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::CaptureImage;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const ONSET_FIXTURE: &str = include_str!("../../../../tests/fixtures/emitter_onset.toml");
    /// Frames of transient, then frames of silence. The tail is comfortably
    /// longer than the fixture's 0.55 s lifetime (33 frames). Six frames is
    /// a tenth of a second — still a transient, and enough marks that the
    /// burst is a picture rather than a handful of dots.
    const HIT: usize = 6;
    const QUIET: usize = 60;
    /// Frames of silence *before* the hit, so "dark before" is measured on
    /// the same capture rather than assumed.
    const LEAD: usize = 6;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 96,
        height: 96,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let preset = Preset::from_toml_str(ONSET_FIXTURE).expect("the onset fixture parses");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let silence = AnalysisFrame::default();
    let transient = AnalysisFrame {
        onset: 1.0,
        beat: true,
        ..Default::default()
    };
    let stimulus: Vec<AnalysisFrame> = std::iter::repeat_n(silence, LEAD)
        .chain(std::iter::repeat_n(transient, HIT))
        .chain(std::iter::repeat_n(silence, QUIET))
        .collect();

    let frames = renderer
        .capture_preset_over(&name, &stimulus)
        .expect("capture the onset burst");
    assert_eq!(frames.len(), LEAD + HIT + QUIET);

    /// Mean RGB level of a frame, 0..255.
    fn level(img: &CaptureImage) -> f32 {
        let sum: f32 = img
            .rgba
            .chunks_exact(4)
            .map(|p| (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0)
            .sum();
        sum / (img.rgba.len() / 4) as f32
    }

    let levels: Vec<f32> = frames.iter().map(level).collect();
    let before = levels.get(..LEAD).map(|s| s.to_vec()).unwrap_or_default();
    let after = levels
        .get(LEAD + HIT..)
        .map(|s| s.to_vec())
        .unwrap_or_default();
    let peak = after.iter().copied().fold(0.0f32, f32::max);
    let lead_peak = before.iter().copied().fold(0.0f32, f32::max);
    let tail = levels.last().copied().unwrap_or(0.0);
    eprintln!(
        "emitter onset burst: lead peak {lead_peak:.4}, burst peak {peak:.4}, \
         tail {tail:.4} (of 255)"
    );

    // Dark before the hit. A hair above zero rather than exactly it: the
    // capture is 8-bit and the fixture's palette is not black.
    assert!(
        lead_peak < 0.05,
        "the frame must be empty before the transient — `spawn_rate` has no \
         constant term, so anything here means the source is not `onset`: \
         {lead_peak:.4}"
    );
    // A real burst, not a flicker.
    assert!(
        peak > 2.0,
        "a transient must visibly fill the frame: peak {peak:.4}"
    );
    // ...and it empties again, by more than two orders of magnitude. This is
    // the retirement half: a scene that only spawned would hold its peak.
    assert!(
        tail * 100.0 < peak,
        "the shower must idle again once the transient passes: tail \
         {tail:.4} against peak {peak:.4} — objects are not being retired"
    );
}

/// **The pool's memory arithmetic, as NFR §12 states it.**
///
/// That section's requirement is "state the cost of what we add", and a
/// per-tier table in a markdown file is exactly the kind of number that goes
/// quietly wrong when a field is added to a struct. These two sizes are what
/// the table multiplies by, so a change to either has to be a deliberate edit
/// to both.
#[test]
fn the_pool_costs_what_the_nfr_says_it_does() {
    use std::mem::size_of;
    assert_eq!(
        size_of::<Object>(),
        40,
        "docs/nfr.md section 12 charges the emitter pool at 40 bytes an object"
    );
    assert_eq!(
        size_of::<Instance>(),
        28,
        "docs/nfr.md section 12 charges the emitter's instance buffers at 28 bytes"
    );
    // The floor tier's total — both CPU copies, the free list, and the GPU
    // buffer — against the ~200 KB the table quotes.
    let floor = crate::render::TierConfig::FLOOR.emitter_objects;
    let bytes = floor * (size_of::<Object>() + 4 + 2 * size_of::<Instance>());
    assert!(
        (190_000..210_000).contains(&bytes),
        "the floor pool costs {bytes} bytes, not the ~200 KB docs/nfr.md quotes"
    );
}

// -----------------------------------------------------------------------
// The third draw seam does not punch holes in the backdrop
// (Plan 0052 Phase 4, ADR-0056)
// -----------------------------------------------------------------------

/// The lit-backdrop fixture this guard captures three ways. Its `bg_bright`
/// and `size` lines are **stripped and rewritten** per capture — one scene at
/// three configurations — so the numbers are read back out of the file rather
/// than restated here, and editing the fixture moves the test with it.
const LIT_FIXTURE: &str = include_str!("../../../../tests/fixtures/emitter_lit_backdrop.toml");

/// The square capture size. Modest, because this reads back three whole float
/// frames; and an exact multiple of the post chain's 256 px grid step, so the
/// trails stage runs at the target size and its present is a 1:1 sample
/// rather than a resample that would blur the property being asserted.
const CAPTURE_SIZE: u32 = 256;

/// Frames per capture. Long enough for the population to fill (the fixture's
/// marks take ~0.5 s to cross the frame) and for the trail history to reach
/// its steady state.
const CAPTURE_FRAMES: u32 = 40;

/// A backdrop channel this bright counts as *present* for the non-vacuity arm
/// below — well above the half-precision floor, well below the fixture's own
/// `bg_bright`.
const BACKDROP_PRESENT: f32 = 0.05;

/// The value of a top-level `key = "<number>"` line in [`LIT_FIXTURE`], or
/// `NaN` when it is absent. Used so the fixture stays the single statement of
/// what this test captures.
fn fixture_value(key: &str) -> f32 {
    LIT_FIXTURE
        .lines()
        .find_map(|line| {
            let rest = line.trim_start().strip_prefix(key)?;
            let rest = rest.trim_start().strip_prefix('=')?;
            rest.trim().trim_matches('"').parse::<f32>().ok()
        })
        .unwrap_or(f32::NAN)
}

/// Slack for half-precision rounding, the same shape the swarm's guard uses.
/// The composite is `Rgba16Float`, so a value of magnitude `m` is stored to
/// roughly `m / 1024`, and the lit capture quantizes a different sum than the
/// backdrop-only one does.
///
/// It is slack, not a tolerance: the property below is **exact** in real
/// arithmetic. Upstream of the tonemap the composite is a plain premultiplied
/// OVER, so where the scene wrote nothing the backdrop must arrive unchanged.
fn half_slack(value: f32) -> f32 {
    (4.0 / 1024.0) * value.abs().max(1.0)
}

/// **Where the emitter drew no light, the backdrop arrives intact** — the
/// third of the per-seam guards ADR-0056 requires, and the reason this scene
/// owed one at all: it inherits the swarm's sprite shader, which is the one
/// that shipped `vec4(in.color * g, 1.0)` and held the backdrop out of the
/// four corners of every square quad.
///
/// # Both directions, measured
///
/// The property alone is easy to satisfy by accident, so the failing
/// direction was demonstrated rather than assumed: reverting this file's
/// fragment shader to a constant alpha —
/// `return vec4<f32>(in.color * g, 1.0);` — and running this test unchanged
/// gives **`worst |L - B|` = 0.3345 with 13 330 of 136 617 compared channels
/// violating** — the backdrop's own brightness discarded outright. With the
/// premultiplied alpha the scene ships, the same capture reads **0.0002 with
/// zero violations**. That is ~1670x between the defect and the noise, on the
/// same fixture, on the same adapter, and it is why the property is asserted
/// at bound 0 rather than inside a tolerance.
///
/// # Why this reads the linear composite and not the capture
///
/// Same reason the swarm's and the bloom's guards do: the capture's bytes are
/// downstream of the tonemap, which scales all three channels off the
/// brightest one (ADR-0046), so adding a backdrop under a mark changes every
/// channel by design and no byte-level tolerance separates that from the
/// defect. Upstream of the tonemap there is no confound — it is a plain
/// premultiplied OVER — so the bound is **0** rather than a tolerance. That
/// readback is `pub(crate)`, which is why this test lives here and not in
/// `core/tests/`.
#[test]
fn a_lit_backdrop_survives_where_the_emitter_drew_nothing() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::capture;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    // --- Non-vacuity, before any GPU work: the fixture must still describe
    // the configuration this guard exists for. ---
    let backdrop = fixture_value("bg_bright");
    let sprite = fixture_value("size");
    let trails = fixture_value("trails");
    let spawn = fixture_value("spawn_rate");
    assert!(
        backdrop > 0.0,
        "emitter_lit_backdrop.toml no longer ships a lit backdrop (bg_bright \
         = {backdrop}); on black this whole comparison is black against black"
    );
    assert!(
        sprite > 0.0,
        "emitter_lit_backdrop.toml no longer draws marks (size = {sprite})"
    );
    assert!(
        spawn > 0.0,
        "emitter_lit_backdrop.toml no longer emits (spawn_rate = {spawn}), so \
         the frame is empty and the seam under test never runs"
    );
    assert!(
        trails > 0.0,
        "emitter_lit_backdrop.toml no longer binds `trails` (= {trails}), so \
         no post stage is active. With an empty chain the scene draws \
         straight onto the backdrop and its additive colour cannot remove \
         light — the defect is unrepresentable and this test proves nothing"
    );

    /// The linear composite the tonemap is about to map, at a given backdrop
    /// brightness and mark size.
    ///
    /// Builds and drops **one** renderer per call rather than holding three:
    /// a second live device in a binary is what the software adapter falls
    /// over on, and building GPU resources mid-run shifts what the trails
    /// stage resolves to on WARP.
    fn linear_composite(bg_bright: f32, size: f32) -> Option<Vec<f32>> {
        let mut renderer = match Renderer::new_headless(HeadlessOptions {
            width: CAPTURE_SIZE,
            height: CAPTURE_SIZE,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return None;
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        };
        // Both keys live in `[params]`, which is the fixture's last table, so
        // stripping them and appending the overrides keeps them in it.
        let base: String = LIT_FIXTURE
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with("bg_bright") && !line.starts_with("size")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let toml = format!("{base}\nbg_bright = \"{bg_bright}\"\nsize = \"{size}\"\n");
        let preset = Preset::from_toml_str(&toml)
            .expect("the lit-backdrop emitter fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        // Every binding is a constant, so the analysis frame only has to be
        // well-formed — the emitter's `update` ignores it entirely.
        let frame = AnalysisFrame::default();
        renderer
            .capture_preset(&name, &frame, CAPTURE_FRAMES)
            .expect("capture the lit-backdrop emitter fixture");

        let device = renderer.ctx.device.clone();
        let queue = renderer.ctx.queue.clone();
        let src = renderer
            .tonemap
            .src_texture()
            .expect("the tonemap built its input while capturing")
            .clone();
        let (buffer, padded_bpr) =
            capture::create_linear_readback(&device, CAPTURE_SIZE, CAPTURE_SIZE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("emitter-backdrop-readback"),
        });
        capture::record_copy(
            &mut encoder,
            &src,
            &buffer,
            padded_bpr,
            CAPTURE_SIZE,
            CAPTURE_SIZE,
        );
        queue.submit(std::iter::once(encoder.finish()));
        Some(
            capture::read_back_linear(&device, &buffer, CAPTURE_SIZE, CAPTURE_SIZE, padded_bpr)
                .expect("read back the linear composite"),
        )
    }

    // `L`: the frame as shipped. `K`: the same scene over a black backdrop,
    // which is what "the scene wrote no light here" is read off. `B`: the
    // backdrop with the scene contributing nothing — zero-area sprite quads
    // rasterize no fragments, so the chain resolves fully transparent and
    // this is the backdrop alone, through the same pipeline as `L`.
    let Some(lit) = linear_composite(backdrop, sprite) else {
        return;
    };
    let Some(dark) = linear_composite(0.0, sprite) else {
        return;
    };
    let Some(backdrop_only) = linear_composite(backdrop, 0.0) else {
        return;
    };
    assert_eq!(dark.len(), lit.len(), "the captures differ in size");
    assert_eq!(
        dark.len(),
        backdrop_only.len(),
        "the captures differ in size"
    );

    let total = dark.len() / 4;
    let (mut untouched, mut drawn, mut over_backdrop) = (0usize, 0usize, 0usize);
    let (mut violations, mut worst, mut compared) = (0usize, 0.0f32, 0usize);
    for (pixel, texel) in dark.chunks_exact(4).enumerate() {
        if texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0 {
            drawn += 1;
            continue; // the scene put light here; the property says nothing
        }
        untouched += 1;
        let base = pixel * 4;
        if backdrop_only
            .get(base..base + 3)
            .is_some_and(|t| t.iter().any(|&c| c > BACKDROP_PRESENT))
        {
            over_backdrop += 1;
        }
        for channel in 0..3 {
            let (Some(&l), Some(&b)) = (lit.get(base + channel), backdrop_only.get(base + channel))
            else {
                continue;
            };
            compared += 1;
            let diff = (l - b).abs();
            if diff > worst {
                worst = diff;
            }
            if diff > half_slack(b) {
                violations += 1;
            }
        }
    }
    eprintln!(
        "emitter lit backdrop at {CAPTURE_SIZE}x{CAPTURE_SIZE}: {untouched} of \
         {total} pixels untouched by the scene ({over_backdrop} of those over \
         a lit backdrop), {drawn} lit by it; worst |L - B| {worst:.4} across \
         {compared} channels"
    );

    // --- Non-vacuity: the region the property speaks about is a substantial
    // part of the frame, the scene genuinely drew into the rest, and the
    // backdrop genuinely reached the frame underneath. A fixture edit that
    // quietly empties any of the three shows up here rather than passing. ---
    assert!(
        untouched * 4 > total,
        "only {untouched} of {total} pixels are untouched by the scene — the \
         fixture has filled the frame and the property covers almost nothing"
    );
    assert!(
        drawn * 20 > total,
        "only {drawn} of {total} pixels carry any scene light — the fixture \
         has stopped drawing, so the sprite corners this guards are not in \
         the frame"
    );
    assert!(
        over_backdrop * 2 > untouched,
        "only {over_backdrop} of the {untouched} untouched pixels sit over a \
         backdrop brighter than {BACKDROP_PRESENT} — comparing black against \
         black, which any alpha would pass"
    );

    // --- The property. ---
    assert_eq!(
        violations, 0,
        "{violations} channels differ between the lit frame and the backdrop \
         alone at pixels where the scene wrote NO light (worst {worst:.4}). \
         Upstream of the tonemap this is a plain premultiplied OVER, so where \
         nothing was drawn the backdrop must arrive intact — a difference \
         here is a sprite emitting coverage it does not have, holding the \
         backdrop out of pixels it never painted"
    );
}

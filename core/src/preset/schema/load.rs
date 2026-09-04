//! Load: raw TOML in, compiled [`Preset`] out.
//!
//! [`Preset::from_toml_str`] is the whole entry point. Everything else here is a
//! step of it -- the latch table, the per-vertex table, the `[layer]` sub-preset
//! and the structural `[generator]`/`[curve]`/`[particles]` config -- and each
//! rejects rather than panicking, so one malformed key never takes down a show.

// A continuation of one module split across several files, so it needs the
// names `preset/schema/mod.rs` has in scope.
use super::*;

impl Preset {
    /// Parse and compile a preset from a TOML source string.
    pub fn from_toml_str(src: &str) -> Result<Self, PresetError> {
        let raw: RawPreset = toml::from_str(src).map_err(PresetError::Toml)?;
        let system = SystemKind::from_name(&raw.system)
            .ok_or_else(|| PresetError::UnknownSystem(raw.system.clone()))?;
        let name = raw.name.unwrap_or_else(|| raw.system.clone());

        // The `[latch]` table (ADR-0137), resolved **before** the bindings that
        // may name a latch — the params below compile against these names, so
        // there is no other order. A preset declaring no table gets an empty
        // list and every expression below compiles exactly as it did before
        // latches existed.
        let latches = build_latches(&raw.latch)?;
        let latch_names: Vec<String> = latches.iter().map(|l| l.name.clone()).collect();

        let mut warnings = Vec::new();
        let mut params = compile_bindings(
            system,
            raw.params,
            &latch_names,
            Surface::Preset,
            &mut warnings,
        )?;

        // The salt the grammar's `hash()`/`noise()` mix in (ADR-0051), read from
        // the long-reserved `[generator] seed`. Read **before** `build_config`
        // consumes the table, and read for every system: only the L-system and
        // the star pattern care about the rest of `[generator]`, but any preset
        // may declare a seed, so a fragment or swarm preset can carry one table
        // holding nothing else.
        //
        // Entropy is drawn here, once per load, and only for `seed = "random"` —
        // never per frame, never from a clock inside evaluation (ADR-0051
        // Alternative B). The pinned twin is what the capture paths read.
        let (salt, pinned_salt) = match raw.generator.as_ref().and_then(|g| g.seed) {
            Some(RawSeed::Random) => (entropy_salt(), 0),
            declared => {
                let salt = salt_from_seed(declared.map_or(0, RawSeed::numeric));
                (salt, salt)
            }
        };

        // Structural config: validated once here (a bad family/grammar -> load
        // error, the caller keeps the last good preset), then trusted by the
        // scene. Built per system so each reads the right table.
        let config = build_config(
            system,
            raw.curve,
            raw.generator,
            raw.particles,
            raw.spectrum,
            raw.mesh,
            raw.milk,
            pinned_salt,
        )?;

        // The `[feedback]` table (ADR-0048): two closed rosters, validated here so
        // an unknown warp or blend is a surfaced load error rather than a preset
        // that quietly renders unwarped. Absent means both defaults.
        let feedback = raw.feedback.unwrap_or_default().into_config()?;

        // Easing time constants (ADR-0019, ADR-0035): validated non-negative +
        // finite at the load boundary, then folded into the bindings so the frame
        // loop reads the constants off the binding instead of hashing its name
        // into a `BTreeMap` once per binding per frame (Plan 0031 Phase 3). A bad
        // value is a surfaced load error, never a panic.
        fold_smoothing(&mut params, &raw.smoothing, Surface::Preset, &mut warnings)?;

        // A `thickness` resting inside the stroke floor's dead zone (Plan 0087
        // Phase 1b, design-backlog 0098). Every value below
        // `MIN_USEFUL_THICKNESS` clamps to the same half-width, so the whole
        // range renders identically and re-tuning inside it changes nothing —
        // which is what makes it expensive: the obvious experiment *disproves
        // the correct hypothesis*. `fragment_vitrail` shipped at 0.016, two
        // orders below the 1.5-3.2 every other line preset uses, and its Maurer
        // rose read as scattered dots for its whole shipped life while the
        // content lane swept chord count and sample count first.
        //
        // A warning rather than an error, in ADR-0020's shape and on its
        // surface: the value is in range and the preset is otherwise good.
        // Only a binding that *rests* at such a value is reported — see
        // `Expr::as_const`.
        if system.param_names().contains(&"thickness") {
            for binding in &params {
                if binding.name != "thickness" {
                    continue;
                }
                let Some(value) = binding.expr.as_const() else {
                    continue;
                };
                if value < crate::render::scenes::lines::MIN_USEFUL_THICKNESS {
                    warnings.push(format!(
                        "parameter 'thickness' rests at {value}, inside the stroke floor's dead \
                         zone: every value below {:.3} renders the identical hairline (about \
                         0.27 px at 1080p), so tuning within that range changes nothing. Line \
                         presets ship between 1.5 and 3.2",
                        crate::render::scenes::lines::MIN_USEFUL_THICKNESS
                    ));
                }
            }
        }

        // A `ring` asked for the scaled-copy coordinate (Plan 0098 Phase 4,
        // ADR-0111's one open behavioural choice). An annulus is the single arm
        // of the roster that is not star-shaped about its own centre — that
        // centre is in the hole, a ray from it crosses the boundary twice, and
        // `r / r_boundary` has no value there. The scene therefore renders the
        // distance instead, and this is what stops that from being silent.
        //
        // Announcing it is the whole point. The three defensible answers were
        // rendered before one was chosen, and the outer-edge definition came out
        // BYTE-IDENTICAL to a `disc`: the coordinate collapses to `length(p)`
        // and the hole stops existing. A preset would name one roster entry and
        // be shown another. The silent fallback renders exactly what this does
        // and only differs in whether anyone is told.
        //
        // A warning rather than an error, in ADR-0020's shape and on the
        // `thickness` dead-zone surface above: both values are legal, the
        // preset is otherwise good, and only a binding that *rests* on the
        // combination can be seen from here.
        if system.param_names().contains(&"coord_mode") {
            let resting = |name: &str| -> Option<f32> {
                params
                    .iter()
                    .find(|b| b.name == name)
                    .and_then(|b| b.expr.as_const())
            };
            let shape = resting("shape").map(crate::render::scenes::marks::mark_shape);
            let mode = resting("coord_mode");
            // The ceiling is `shape_field`'s own roster, not a literal `1.0`: a
            // third coordinate would leave a hardcoded bound quietly testing the
            // wrong thing, and this quantizes the way the scene does.
            let max_mode = (crate::render::scenes::shape_field::COORD_MODES.len() - 1) as f32;
            if shape == Some(crate::render::scenes::marks::RING_SHAPE)
                && mode.is_some_and(|m| m.is_finite() && m.clamp(0.0, max_mode).round() >= 1.0)
            {
                warnings.push(
                    "parameter 'coord_mode' is ignored on a `ring`: an annulus's centre lies in \
                     its hole, so a ray from there crosses the outline twice and the \
                     scaled-copy coordinate has no single value. The figure is drawn with the \
                     distance instead. Defining it against the outer rim was the alternative \
                     and it renders a `disc` — the hole stops existing"
                        .to_string(),
                );
            }
        }

        // A `shape_field` `color_span` narrow enough to starve the gradient
        // (design-backlog 0099). The scene hands the palette a FIGURE
        // coordinate whose `0..1` is the interior, so `color_span` is literally
        // the share of the 256-texel LUT the figure is drawn through: at 0.037
        // that is nine texels for the whole of it, linear-filtered across
        // however much of the frame the figure covers, which is an upscaled
        // gradient and reads as one.
        //
        // The reason this is worth a warning and not a note in a document is
        // that **the symptom names the wrong subsystem**: a soft, crawling
        // figure reads as a bad silhouette or bad shading, and the value is in
        // range and the preset is otherwise good. It cost one user look verdict
        // two misattributions.
        //
        // A warning rather than an error, on ADR-0020's surface, and only for a
        // binding that *rests* — a `color_span` sweeping through the range is a
        // different claim, and `Expr::as_const` is what separates them.
        if system == SystemKind::ShapeField {
            let resting = |name: &str| -> Option<f32> {
                params
                    .iter()
                    .find(|b| b.name == name)
                    .and_then(|b| b.expr.as_const())
            };
            // `palette_steps` snaps the coordinate to a band centre before the
            // LUT read, so every pixel samples one exact texel and nothing is
            // interpolated — the trap does not exist there. A band count that
            // is bound rather than resting is the author working in bands too,
            // so only a count resting BELOW the quantizer's own activation
            // threshold leaves this live.
            let banded = match params.iter().find(|b| b.name == "palette_steps") {
                Some(binding) => binding.expr.as_const().is_none_or(|steps| {
                    crate::render::palette::band_steps(steps)
                        > crate::render::palette::MIN_ACTIVE_STEPS
                }),
                None => false,
            };
            if let Some(span) = resting("color_span")
                && !banded
            {
                let texels = crate::render::scenes::shape_field::interior_texels(span);
                if span.is_finite()
                    && texels < crate::render::scenes::shape_field::MIN_INTERIOR_TEXELS
                {
                    warnings.push(format!(
                        "parameter 'color_span' rests at {span}, which draws the figure's whole \
                         interior through about {texels:.0} of the palette's {} texels. Below \
                         roughly {:.0} the LUT's linear filtering is interpolating more than it \
                         is reading and the figure comes back looking upscaled rather than \
                         shaded — the estimate is exact in the coordinate and approximate on \
                         screen, since how much of the frame the figure covers depends on its \
                         shape and framing. Bind or set `palette_steps` to remove the \
                         interpolation entirely",
                        crate::render::palette::LUT_SIZE,
                        crate::render::scenes::shape_field::MIN_INTERIOR_TEXELS,
                    ));
                }
            }
        }

        // Palette selection (ADR-0021): validated at this boundary into a
        // baked-ready `PaletteConfig`; a bad name/stop list is a surfaced load
        // error, never a panic. `None` -> the default `spectrum`. `[palette_b]`
        // (the crossfade target) validates the same way.
        let palette = raw.palette.map(RawPalette::into_config).transpose()?;
        let palette_b = raw.palette_b.map(RawPalette::into_config).transpose()?;

        // Saturation exemptions (ADR-0062). A name this preset does not bind is
        // a warning for the same reason a `[smoothing]` entry naming one is: it
        // silences nothing, so a typo here would leave the author believing a
        // gate was exempted while the gate goes on failing on the real name.
        let mut occupancy_exempt = raw.occupancy.unwrap_or_default().exempt;
        occupancy_exempt.sort();
        occupancy_exempt.dedup();
        for name in &occupancy_exempt {
            if !params.iter().any(|b| &b.name == name) {
                warnings.push(format!(
                    "[occupancy] exempt entry '{name}' is inert: this preset binds no such \
                     parameter"
                ));
            }
        }

        // The `[per_vertex]` table (Plan 0100 Phase 1): the warp mesh's
        // per-vertex program, compiled like any binding and never eased.
        let per_vertex = build_per_vertex(
            system,
            raw.per_vertex,
            &raw.smoothing,
            &latch_names,
            "",
            &mut warnings,
        )?;
        // A `[params]` binding reaching for a vertex variable reads a flat zero
        // there — the names are crate-wide because expression slots are
        // positional, and only a `[per_vertex]` table ever binds them.
        warn_vertex_use(&params, Surface::Preset, &mut warnings);

        // A latch nothing reads is inert, and inert-and-silent is what this file
        // exists to prevent — the same warning an `[occupancy] exempt` entry
        // naming an unbound param gets, for the same reason: the author believes
        // an event is wired up while nothing consumes it. Every surface that can
        // name a latch is searched, including the layer's, which is why this sits
        // after the layer is built.
        let layer = raw
            .layer
            .map(|l| build_layer(l, &latch_names, &mut warnings))
            .transpose()?;
        for (slot, latch) in latches.iter().enumerate() {
            let mut read = params
                .iter()
                .chain(&per_vertex)
                .any(|b| b.expr.uses_latch(slot));
            if let Some(l) = layer.as_ref() {
                read = read
                    || l.params
                        .iter()
                        .chain(&l.per_vertex)
                        .chain(l.mix.as_ref())
                        .any(|b| b.expr.uses_latch(slot));
            }
            if !read {
                warnings.push(format!(
                    "[latch] '{}' is inert: no binding in this preset names it",
                    latch.name
                ));
            }
        }

        Ok(Preset {
            name,
            system,
            params,
            per_vertex,
            latches,
            config,
            feedback,
            palette,
            palette_b,
            salt,
            pinned_salt,
            occupancy_exempt,
            layer,
            warnings,
            representative: raw.representative,
        })
    }
}

/// Compile and validate the `[latch]` table (ADR-0137).
///
/// Slot order is the `BTreeMap`'s, so it is the entries' **name order** and not
/// their order in the file: a preset's latch-to-slot mapping is then a function
/// of the set of names alone, and re-ordering the table in the TOML cannot move
/// a latch onto a different slot.
///
/// Everything here is a load-time check, in the shape the rest of this file
/// uses: a bad expression is a [`PresetError::Expr`] naming which key of which
/// latch, and every other failure is a [`PresetError::Config`]. The two
/// expressions compile with **no** latch names in scope — see [`Latch`] for why
/// that is the design rather than an omission.
pub(super) fn build_latches(raw: &BTreeMap<String, RawLatch>) -> Result<Vec<Latch>, PresetError> {
    if raw.len() > expr::LATCH_CAP {
        return Err(PresetError::Config(format!(
            "[latch] declares {} entries; a preset may hold at most {} (ADR-0137: the \
             reserved variable block is a fixed size, so this is a wall rather than a \
             slower path)",
            raw.len(),
            expr::LATCH_CAP,
        )));
    }
    let mut out = Vec::with_capacity(raw.len());
    for (name, entry) in raw {
        if !expr::is_identifier(name) {
            return Err(PresetError::Config(format!(
                "[latch] name '{name}' is not an identifier: a latch is referenced from \
                 an expression, so its name must start with a letter or underscore and \
                 hold only letters, digits and underscores"
            )));
        }
        if expr::is_reserved_ident(name) {
            return Err(PresetError::Config(format!(
                "[latch] name '{name}' is already a variable, constant or function in the \
                 expression grammar; a binding naming it would read that instead of the \
                 latch"
            )));
        }
        check_hold(name, entry.hold)?;
        let compile = |key: &str, source: &str| {
            expr::compile(source).map_err(|err| PresetError::Expr {
                param: format!("[latch] {name}.{key}"),
                err,
            })
        };
        out.push(Latch {
            name: name.clone(),
            arm: compile("arm", &entry.arm)?,
            fire: compile("fire", &entry.fire)?,
            hold: entry.hold,
        });
    }
    Ok(out)
}

/// Validate one `[latch] hold`, in `check_tau`'s shape and at the same boundary:
/// a non-negative, finite number of seconds, checked once here and trusted by
/// the render layer's countdown.
pub(super) fn check_hold(name: &str, seconds: f32) -> Result<(), PresetError> {
    if seconds.is_finite() && seconds >= 0.0 {
        return Ok(());
    }
    Err(PresetError::Config(format!(
        "[latch] '{name}' hold must be a non-negative number of seconds, got {seconds}"
    )))
}

/// Compile a `[per_vertex]` table into bindings (Plan 0100 Phase 1).
///
/// `label` prefixes the error and warning text (`""` at the top level,
/// `"[layer] "` inside one), and `smoothing` is that surface's own easing table
/// — consulted only to warn, since a per-vertex binding is never eased.
///
/// Unknown names warn and keep the binding, exactly like `[params]` (ADR-0020):
/// one typo must not discard an otherwise-good mesh program. A binding here for
/// a system that has no per-vertex surface warns too — the table is inert there,
/// and silently inert is the thing this file exists to prevent.
pub(super) fn build_per_vertex(
    system: SystemKind,
    raw: BTreeMap<String, String>,
    smoothing: &BTreeMap<String, RawSmoothing>,
    latch_names: &[String],
    label: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Binding>, PresetError> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    if system != SystemKind::WarpMesh {
        warnings.push(format!(
            "{label}[per_vertex] is inert for system '{}': only `warp_mesh` evaluates a \
             per-vertex program (bindings kept, but nothing reads them)",
            system.as_str()
        ));
    }
    let mut out = Vec::with_capacity(raw.len());
    for (param, source) in raw {
        let expr =
            expr::compile_with_latches(&source, latch_names).map_err(|err| PresetError::Expr {
                param: format!("{label}[per_vertex] {param}"),
                err,
            })?;
        if !crate::render::scenes::warp_mesh::PER_VERTEX_PARAMS.contains(&param.as_str()) {
            warnings.push(format!(
                "unknown {label}[per_vertex] parameter '{param}' (expected one of: {}) \
                 (binding kept, but nothing reads it)",
                crate::render::scenes::warp_mesh::PER_VERTEX_PARAMS.join(", ")
            ));
        }
        if smoothing.contains_key(&param) {
            warnings.push(format!(
                "{label}[smoothing] entry '{param}' is ignored: it names a [per_vertex] \
                 binding, which is evaluated once per mesh vertex and has no single \
                 value to ease"
            ));
        }
        out.push(Binding {
            name: param,
            expr,
            // Never eased — see `Preset::per_vertex`.
            tau: Easing::INSTANT,
        });
    }
    Ok(out)
}

/// Validate a `[layer]` table (ADR-0090 / Plan 0076). Structural keys —
/// `system`, `join`, `blend` — follow `[curve] family`'s rule: an unknown value
/// rejects the preset, because it selects a code path and a silent default
/// would render a look the author never asked for. Unknown *param* names warn
/// and keep the binding, exactly like the top level (ADR-0020).
pub(super) fn build_layer(
    raw: RawLayer,
    latch_names: &[String],
    warnings: &mut Vec<String>,
) -> Result<Layer, PresetError> {
    let system = SystemKind::from_name(&raw.system)
        .ok_or_else(|| PresetError::UnknownSystem(raw.system.clone()))?;
    // Any pair of systems is legal — including the same system twice, and two
    // line-family systems. The layer's scene is constructed **for the preset**
    // (`scenes::create_layer_scene`, ADR-0090 point 4 / Plan 0076 Phase 2), so
    // it shares no GPU state with the roster's instance of the same kind.

    let join = match raw.join.as_deref() {
        None => LayerJoin::default(),
        Some(name) => LayerJoin::from_name(name).ok_or_else(|| {
            PresetError::Config(format!(
                "unknown [layer] join '{name}' (expected one of: under, over)"
            ))
        })?,
    };
    let blend = match raw.blend.as_deref() {
        None => LayerBlend::default(),
        Some(name) => LayerBlend::from_name(name).ok_or_else(|| {
            PresetError::Config(format!(
                "unknown [layer] blend '{name}' (expected one of: {})",
                LayerBlend::ALL
                    .iter()
                    .map(|b| b.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?,
    };
    if raw.blend.is_some() && join == LayerJoin::Under {
        warnings.push(format!(
            "[layer] blend = '{}' is ignored on an under join: the layer shares the \
             main scene's composite, so there is no junction for a blend mode to \
             apply at (blend belongs to join = \"over\")",
            blend.as_str()
        ));
    }

    // The layer's bindings, name-sorted off the BTreeMap like the preset's own.
    let mut params = compile_bindings(system, raw.params, latch_names, Surface::Layer, warnings)?;

    // `[layer.smoothing]`: the same vocabulary, validation and fold as the top
    // level (ADR-0019 / ADR-0035), against the layer's own bindings.
    fold_smoothing(&mut params, &raw.smoothing, Surface::Layer, warnings)?;

    // The bindable mix (ADR-0090): compiled like any binding, eased through
    // `[layer.smoothing] mix`. Parsed now; the `over` blend consumes it in
    // Plan 0076 Phase 3.
    let mix = raw
        .mix
        .as_deref()
        .map(|source| {
            let expr = expr::compile_with_latches(source, latch_names).map_err(|err| {
                PresetError::Expr {
                    param: "[layer] mix".into(),
                    err,
                }
            })?;
            Ok(Binding {
                name: "mix".into(),
                expr,
                tau: raw
                    .smoothing
                    .get("mix")
                    .map_or(Easing::INSTANT, |entry| entry.to_easing()),
            })
        })
        .transpose()?;

    // `[layer.per_vertex]` — the same surface as the top level's, against the
    // layer's own system and its own smoothing table.
    let per_vertex = build_per_vertex(
        system,
        raw.per_vertex,
        &raw.smoothing,
        latch_names,
        "[layer] ",
        warnings,
    )?;
    warn_vertex_use(&params, Surface::Layer, warnings);

    // The layer's structural config, by the same per-system rules as the top
    // level (ADR-0007) — a layer L-system still requires its `[layer.generator]`
    // table, a layer attractor still defaults to De Jong.
    let config = build_config(
        system,
        raw.curve,
        raw.generator,
        raw.particles,
        raw.spectrum,
        raw.mesh,
        // A `[layer]` carries no `[milk]` table: a converted preset is a whole
        // preset, and layering one under another is a composition nothing in the
        // corpus asks for. A layer warp mesh drives its mesh from `[layer.params]`
        // and `[layer.per_vertex]` like any hand-authored one.
        None,
        0,
    )?;

    Ok(Layer {
        system,
        join,
        blend,
        mix,
        params,
        per_vertex,
        config,
    })
}

/// Fold a declared 64-bit `[generator] seed` into the 32-bit salt the grammar's
/// `hash()`/`noise()` mix in (ADR-0051).
///
/// XOR-folded rather than truncated, so two seeds differing only in their high
/// half still salt differently — `seed` is a `u64` in the schema and always has
/// been, and silently ignoring half of what an author typed is the kind of
/// surprise this file exists to prevent.
pub(super) fn salt_from_seed(seed: u64) -> u32 {
    (seed as u32) ^ ((seed >> 32) as u32)
}

/// A salt drawn from OS entropy — the `seed = "random"` path (ADR-0051). Called
/// **once per preset load**, in the live app only: no capture path reaches it,
/// because a capture reads [`Preset::pinned_salt`] instead.
///
/// `RandomState` is the standard library's own entropy: its keys are seeded from
/// the OS once per process and advance on every `new()`, so two loads of the same
/// preset — in one run or across two — draw different salts. Using it is what
/// keeps "sometimes crazy" from costing a dependency (`lightweight is a feature`).
pub(super) fn entropy_salt() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::BuildHasher;
    salt_from_seed(RandomState::new().hash_one(0u64))
}

/// Assemble the optional structural config for `system` from the raw tables,
/// validating at this boundary (ADR-0007). Non-line systems have no config.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_config(
    system: SystemKind,
    curve: Option<RawCurve>,
    generator: Option<RawGenerator>,
    particles: Option<RawParticles>,
    spectrum: Option<RawSpectrum>,
    mesh: Option<RawMesh>,
    milk: Option<RawMilk>,
    salt: u32,
) -> Result<Option<GeneratorConfig>, PresetError> {
    match system {
        // A curve preset without a `[curve]` table accepts the family default.
        SystemKind::ParametricCurve => curve.map(RawCurve::into_config).transpose(),
        // A generator preset must declare its `[generator]` table.
        SystemKind::LSystem => {
            let g = generator.ok_or_else(|| {
                PresetError::Config("lsystem requires a [generator] table".into())
            })?;
            Ok(Some(g.into_lsystem()?))
        }
        SystemKind::StarPattern => {
            let g = generator.ok_or_else(|| {
                PresetError::Config("star_pattern requires a [generator] table".into())
            })?;
            Ok(Some(g.into_star()?))
        }
        // The attractor scene selects its map via an optional `[particles]` table;
        // absent, it defaults to De Jong. Config is always `Some` so `configure`
        // runs on every preset switch (resetting the family — never stale).
        SystemKind::Attractor => {
            let (family, density, morph_to, tuple_path) = match particles {
                Some(p) => {
                    let family = AttractorFamily::from_name(&p.family).ok_or_else(|| {
                        PresetError::Config(format!("unknown attractor family '{}'", p.family))
                    })?;
                    (
                        family,
                        p.density()?,
                        p.morph_to(family)?,
                        p.tuple_path(family)?,
                    )
                }
                None => (AttractorFamily::DeJong, 1.0, None, None),
            };
            Ok(Some(GeneratorConfig::Particles {
                family,
                density,
                morph_to,
                tuple_path,
            }))
        }
        // The spectrum readout selects its element count, layout and per-element
        // easing through an optional `[spectrum]` table; absent, it takes the
        // defaults. Config is always `Some` so `configure` runs on every preset
        // switch (resizing the element buffer — never stale).
        SystemKind::Spectrum => Ok(Some(match spectrum {
            Some(s) => s.into_config()?,
            None => RawSpectrum::default().into_config()?,
        })),
        // The warp mesh's grid is structural for `[curve] family`'s reason: the
        // vertex and index buffers are built from it, and an eased grid would
        // rebuild them mid-frame. Config is always `Some` so `configure` runs on
        // every preset switch (resizing the mesh — never stale).
        SystemKind::WarpMesh => {
            let bundle = milk.map(|raw| raw.into_bundle()).transpose()?;
            Ok(Some(
                mesh.unwrap_or_default()
                    .into_config(bundle.map(Box::new), salt)?,
            ))
        }
        // Reaction-diffusion drives its regime through named params (feed/kill/
        // flow), not a declarative structural table. `shape_field` is here for a
        // sharper reason: its structure is the `marks` roster, which is a closed
        // list selected by a numeric `shape` param (ADR-0084/ADR-0105), so there
        // is nothing declarative for a table to carry.
        // `shape_collage` joins them at Plan 0113 Phase 1 with the same answer
        // and a different reason: its structure is an authored element list
        // compiled into the scene. Phase 4's seeded layout grammar is selected by
        // named params too, so this arm is expected to stay where it is.
        SystemKind::FragmentField
        | SystemKind::Swarm
        | SystemKind::ReactionDiffusion
        | SystemKind::Emitter
        | SystemKind::ShapeField
        | SystemKind::ShapeCollage => Ok(None),
    }
}

/// Which surface a binding table belongs to: the preset itself, or its
/// `[layer]`.
///
/// The two compile bindings, validate `[smoothing]` and warn about a stray
/// vertex variable by identical rules, and differ in exactly three ways -- how a
/// message names the parameter, which TOML table it names, and whether a
/// compositing parameter counts as known there. One value carries all three
/// rather than three flags at every call.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Surface {
    Preset,
    Layer,
}

impl Surface {
    /// What every message on this surface prefixes a parameter name with.
    fn prefix(self) -> &'static str {
        match self {
            Surface::Preset => "",
            Surface::Layer => "[layer] ",
        }
    }

    /// `table("smoothing")` is `[smoothing]` at the top level and
    /// `[layer.smoothing]` inside a layer.
    fn table(self, name: &str) -> String {
        match self {
            Surface::Preset => format!("[{name}]"),
            Surface::Layer => format!("[layer.{name}]"),
        }
    }
}

/// Compile a `[params]` table into bindings, warning about every name the
/// surface does not consume.
///
/// A name the system does not consume is a warning, not an error: one typo must
/// not discard the rest of an otherwise-good preset (ADR-0020 / NFR 10). The
/// binding is kept -- an unconsumed param is harmless at apply time, and
/// dropping it would turn a surfaced warning back into a silent loss.
///
/// `tau` is left [`Easing::INSTANT`] here and filled by [`fold_smoothing`] once
/// the `[smoothing]` table has been validated, which is what preserves which
/// error a preset with several problems reports first.
pub(super) fn compile_bindings(
    system: SystemKind,
    params: BTreeMap<String, String>,
    latch_names: &[String],
    surface: Surface,
    warnings: &mut Vec<String>,
) -> Result<Vec<Binding>, PresetError> {
    let prefix = surface.prefix();
    // The raw params come from a BTreeMap, so bindings land name-sorted:
    // evaluation is order-independent, but determinism is cheap to keep.
    let mut out = Vec::with_capacity(params.len());
    for (param, source) in params {
        let expr =
            expr::compile_with_latches(&source, latch_names).map_err(|err| PresetError::Expr {
                param: format!("{prefix}{param}"),
                err,
            })?;
        let known = match surface {
            Surface::Preset => is_known_param(system, &param),
            // A layer binds its own scene's params only, never the compositing
            // stages -- so a global here is a *different* mistake from a typo
            // and says so.
            Surface::Layer => system.param_names().contains(&param.as_str()),
        };
        if !known {
            if surface == Surface::Layer
                && GLOBAL_PARAMS
                    .iter()
                    .any(|stage| stage.contains(&param.as_str()))
            {
                warnings.push(format!(
                    "[layer] parameter '{param}' is a compositing parameter; a layer \
                     binds only its own scene's params — bind it at the top level, \
                     where it drives the whole preset (binding kept, but nothing \
                     reads it here)"
                ));
            } else {
                warnings.push(format!(
                    "unknown {prefix}parameter '{param}' for system '{}' (binding kept, but nothing reads it)",
                    system.as_str()
                ));
            }
        }
        out.push(Binding {
            name: param,
            expr,
            tau: Easing::INSTANT,
        });
    }
    Ok(out)
}

/// Validate the `[smoothing]` table, then fold it into the bindings' `tau`.
///
/// Validation runs over the whole table first so a preset with two bad entries
/// reports the same one it always did. An entry naming a per-element binding is
/// inert -- a series has no single value to ease -- and says so rather than
/// silently doing nothing.
pub(super) fn fold_smoothing(
    params: &mut [Binding],
    smoothing: &BTreeMap<String, RawSmoothing>,
    surface: Surface,
    warnings: &mut Vec<String>,
) -> Result<(), PresetError> {
    let prefix = surface.prefix();
    for (param, entry) in smoothing {
        let named = format!("{prefix}{param}");
        match *entry {
            RawSmoothing::Symmetric(seconds) => check_tau(&named, None, seconds)?,
            RawSmoothing::Asymmetric { attack, release } => {
                check_tau(&named, Some("attack"), attack)?;
                check_tau(&named, Some("release"), release)?;
            }
        }
    }
    for binding in params {
        binding.tau = smoothing
            .get(&binding.name)
            .map_or(Easing::INSTANT, |entry| entry.to_easing());
        if binding.expr.uses_index() && smoothing.contains_key(&binding.name) {
            warnings.push(format!(
                "{} entry '{}' is ignored: the binding names `index`, so it is \
                 evaluated per element and cannot be eased as one value \
                 (use {} smoothing for the element levels)",
                surface.table("smoothing"),
                binding.name,
                surface.table("spectrum"),
            ));
            binding.tau = Easing::INSTANT;
        }
    }
    Ok(())
}

/// Warn for every binding that reaches for a vertex variable outside a
/// `[per_vertex]` table, where it reads a flat zero.
///
/// The names are crate-wide because expression slots are positional, so nothing
/// stops a `[params]` binding from naming one; only a `[per_vertex]` table ever
/// binds them.
pub(super) fn warn_vertex_use(params: &[Binding], surface: Surface, warnings: &mut Vec<String>) {
    let prefix = surface.prefix();
    for binding in params {
        if binding.expr.uses_vertex() {
            warnings.push(format!(
                "{prefix}parameter '{}' names a per-vertex variable (x/y/rad/ang), \
                 which reads 0 outside a {} table",
                binding.name,
                surface.table("per_vertex"),
            ));
        }
    }
}

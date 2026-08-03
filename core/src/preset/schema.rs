//! TOML preset schema: which built-in system a preset drives and the
//! expression bound to each of its named parameters.
//!
//! Parsing happens once at load: the raw TOML is deserialized, each parameter
//! expression is compiled (a malformed one is rejected with a surfaced error),
//! and the result is an in-memory [`Preset`] whose bindings are ready to
//! evaluate. A bad preset returns `Err` — it never panics, so the caller can
//! degrade to the last good preset (ADR-0002 / NFR 10).

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use super::expr::{self, Expr, ExprError};
use crate::render::palette::{NamedPalette, PaletteConfig};
use crate::render::scenes::lines::{
    CurveFamily, GeneratorConfig, MAX_LSYSTEM_DEPTH, SpectrumLayout, hankin,
};
use crate::render::scenes::particles::AttractorFamily;

/// The built-in system a preset drives. Extend as Plan 0003 (and later plans)
/// add systems; unknown names are rejected at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKind {
    /// The fullscreen fragment-field scene.
    FragmentField,
    /// The CPU particle-swarm scene.
    Swarm,
    /// The parametric line-curve scene (Maurer rose, ...) — ADR-0007.
    ParametricCurve,
    /// The L-system generator scene — ADR-0007.
    LSystem,
    /// The Hankin star-pattern generator scene — ADR-0007.
    StarPattern,
    /// The Gray-Scott reaction-diffusion feedback scene — ADR-0012.
    ReactionDiffusion,
    /// The GPU compute-particle strange-attractor scene — ADR-0015.
    Attractor,
    /// The N-element spectrum readout — ADR-0036. A line scene like the three
    /// above (it draws through the same shared renderer), driven by the analysis
    /// frame's log-spaced band array rather than by a generator.
    Spectrum,
}

impl SystemKind {
    /// How many variants [`SystemKind`] has. Kept honest by the private
    /// `variant_roster_reminder` below: a new variant fails the build there until
    /// this is bumped, and the length of [`ALL`](SystemKind::ALL) is typed from
    /// it, so bumping it without rostering the variant does not compile either.
    pub const VARIANT_COUNT: usize = 8;

    /// **The** roster of built-in systems — every [`SystemKind`], in the order the
    /// engine builds their scenes. The single place the variant list lives: the
    /// scene factory (`render::scenes::create_all`) and the golden drift guard
    /// both iterate this rather than keeping lists of their own.
    ///
    /// Typed `[SystemKind; VARIANT_COUNT]`, so a roster that has drifted from the
    /// variant count is a compile error, not a test failure.
    pub const ALL: [SystemKind; Self::VARIANT_COUNT] = [
        SystemKind::FragmentField,
        SystemKind::Swarm,
        SystemKind::ParametricCurve,
        SystemKind::LSystem,
        SystemKind::StarPattern,
        SystemKind::ReactionDiffusion,
        SystemKind::Attractor,
        SystemKind::Spectrum,
    ];

    /// Parse a canonical system name (as written in a preset's `system = "..."`
    /// field) into its [`SystemKind`], or `None` if unknown. The inverse of
    /// [`SystemKind::as_str`]; together they are the single source for the
    /// name↔kind mapping, reused by the `shot` CLI so it declares no match of
    /// its own.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "fragment_field" => SystemKind::FragmentField,
            "swarm" => SystemKind::Swarm,
            "parametric_curve" => SystemKind::ParametricCurve,
            "lsystem" => SystemKind::LSystem,
            "star_pattern" => SystemKind::StarPattern,
            "reaction_diffusion" => SystemKind::ReactionDiffusion,
            "attractor" => SystemKind::Attractor,
            "spectrum" => SystemKind::Spectrum,
            _ => return None,
        })
    }

    /// The canonical name of this system — the exact string accepted by
    /// [`SystemKind::from_name`] and written in a preset's `system` field. The
    /// two functions are inverses and the one place the mapping lives.
    pub fn as_str(self) -> &'static str {
        match self {
            SystemKind::FragmentField => "fragment_field",
            SystemKind::Swarm => "swarm",
            SystemKind::ParametricCurve => "parametric_curve",
            SystemKind::LSystem => "lsystem",
            SystemKind::StarPattern => "star_pattern",
            SystemKind::ReactionDiffusion => "reaction_diffusion",
            SystemKind::Attractor => "attractor",
            SystemKind::Spectrum => "spectrum",
        }
    }

    /// The parameter names this system's scene consumes. Each list lives beside
    /// that scene's `set_param` match (the two are guarded against drift by
    /// `declared_params_match_set_param` in `core/tests/preset.rs`); this is the
    /// one place they are gathered for the loader's typo check (ADR-0020).
    ///
    /// Does **not** include the global compositing params — a preset for any
    /// system may bind those, so [`is_known_param`] unions them in.
    pub fn param_names(self) -> &'static [&'static str] {
        use crate::render::scenes;
        match self {
            SystemKind::FragmentField => scenes::fragment_field::PARAMS,
            SystemKind::Swarm => scenes::swarm::PARAMS,
            SystemKind::ParametricCurve => scenes::lines::parametric::PARAMS,
            SystemKind::LSystem => scenes::lines::lsystem::PARAMS,
            SystemKind::StarPattern => scenes::lines::star::PARAMS,
            SystemKind::ReactionDiffusion => scenes::reaction_diffusion::PARAMS,
            SystemKind::Attractor => scenes::particles::PARAMS,
            SystemKind::Spectrum => scenes::lines::spectrum::PARAMS,
        }
    }
}

/// Compile-time reminder (never called): adding a [`SystemKind`] variant makes
/// this exhaustive match non-exhaustive and fails the build, prompting the dev to
/// bump [`SystemKind::VARIANT_COUNT`] and add the variant to
/// [`SystemKind::ALL`] — which in turn forces a scene into the exhaustive factory
/// in `render::scenes` and a fixture into the golden drift guard.
#[allow(dead_code)]
fn variant_roster_reminder(system: SystemKind) {
    match system {
        SystemKind::FragmentField
        | SystemKind::Swarm
        | SystemKind::ParametricCurve
        | SystemKind::LSystem
        | SystemKind::StarPattern
        | SystemKind::ReactionDiffusion
        | SystemKind::Attractor
        | SystemKind::Spectrum => {}
    }
}

/// The parameter names any preset may bind regardless of its system: the five
/// compositing stages that run around the scene (`bg_*`, `trails`, `kaleido_*`,
/// `exposure`, `ink_*`/`paper_*`). Gathered from each stage's own declared
/// vocabulary so there is no third copy to drift.
///
/// "The renderer routes to" was true when this was written and is not any more:
/// `trails` and `kaleido_*` are offered by the `PostChain` (ADR-0031),
/// `exposure` by the tonemap (ADR-0046) and `ink_*`/`paper_*` by the terminal ink
/// pass (ADR-0032); only `bg_*` goes to a pass the renderer drives directly. The
/// *names* are what this const is about — see `render::ParamRoute` for who
/// actually owns each.
pub const GLOBAL_PARAMS: [&[&str]; 6] = [
    crate::render::background::PARAMS,
    crate::render::trails::PARAMS,
    crate::render::kaleidoscope::PARAMS,
    crate::render::bloom::PARAMS,
    crate::render::tonemap::PARAMS,
    crate::render::ink::PARAMS,
];

/// Whether `name` is a parameter `system` (or the global compositing layer)
/// actually consumes. An unknown name is a load-time **warning**, not an error:
/// the preset still loads and applies its good bindings (ADR-0020, NFR 10).
pub fn is_known_param(system: SystemKind, name: &str) -> bool {
    system.param_names().contains(&name) || GLOBAL_PARAMS.iter().any(|stage| stage.contains(&name))
}

/// A binding's easing time constants in **seconds** (ADR-0019, widened to a pair
/// by ADR-0035).
///
/// `attack` applies while the incoming value is **above** the held one and
/// `release` while it is at or below — so a percussive parameter can reach its
/// target in a frame or two and then glide back over most of a second, which no
/// single constant expresses at any value.
///
/// The scalar `[smoothing]` form builds [`Easing::symmetric`], which is the
/// low-pass ADR-0019 shipped: with both constants equal the direction test picks
/// the same number either way, so the arithmetic is bit-for-bit unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Easing {
    /// Constant used while the raw value is **above** the held value (rising).
    pub attack: f32,
    /// Constant used while the raw value is at or **below** the held value.
    pub release: f32,
}

impl Easing {
    /// No smoothing on either side: the value is applied instantly. The default
    /// for a parameter absent from `[smoothing]`.
    pub const INSTANT: Self = Self {
        attack: 0.0,
        release: 0.0,
    };

    /// One constant in both directions — the scalar `[smoothing]` form.
    pub const fn symmetric(tau: f32) -> Self {
        Self {
            attack: tau,
            release: tau,
        }
    }

    /// One frame of the one-pole envelope: ease `held` toward `raw` over `dt`
    /// real seconds, using whichever constant the direction of travel selects.
    ///
    /// **The single implementation of this vocabulary.** The render layer's
    /// per-binding smoother and the spectrum scene's per-element smoother both
    /// call it, so "smoothing in seconds, frame-rate independent, asymmetric by
    /// direction" means exactly one thing everywhere (ADR-0019 / ADR-0035, Plan
    /// 0034 Phase 3).
    ///
    /// The direction test is against the **held** value, not the raw signal's own
    /// derivative: a value already above its new target releases toward it even
    /// while the input is still rising. That is the envelope-follower convention,
    /// and it is what keeps the behavior stable under a noisy input.
    ///
    /// A selected constant of `<= 0` (the default) or non-finite, or a
    /// non-positive `dt`, passes `raw` through unchanged. Total and
    /// allocation-free — it runs per element per frame.
    ///
    /// **A non-finite `held` or `raw` also passes `raw` through** — a snap,
    /// which is what a smoother with no valid state should do (Plan 0038
    /// Phase 9). This is not a theoretical edge: `log(0)` is `-inf` and silence
    /// produces it every time the music stops, so a `[smoothing]`-listed binding
    /// reaches this on ordinary material. Without the guard the arithmetic below
    /// is `-inf + alpha * (-inf - -inf)` = `NaN`, and `NaN` is **absorbing**
    /// here — `raw > held` is false for every `raw`, so the release branch is
    /// taken and the state stays `NaN` forever. The binding would be dead for
    /// the rest of the preset's run, recovering only on a switch.
    ///
    /// Both operands are checked because guarding `raw` alone does not fix it:
    /// a stored `-inf` against a *finite* `raw` selects `attack` and computes
    /// `-inf + inf`, which is `NaN` on the very next frame.
    pub fn step(self, held: f32, raw: f32, dt: f32) -> f32 {
        if !held.is_finite() || !raw.is_finite() {
            return raw;
        }
        let tau = if raw > held {
            self.attack
        } else {
            self.release
        };
        if tau <= 0.0 || !tau.is_finite() || dt <= 0.0 {
            return raw;
        }
        // alpha = 1 - exp(-dt/tau): the fraction of the gap closed this frame,
        // frame-rate-independent because `dt` is real elapsed time (ADR-0019).
        let alpha = 1.0 - (-dt / tau).exp();
        held + alpha * (raw - held)
    }
}

impl Default for Easing {
    fn default() -> Self {
        Self::INSTANT
    }
}

/// A named parameter bound to a compiled expression.
#[derive(Debug)]
pub struct Binding {
    /// The system parameter this drives (e.g. `warp`, `hue`).
    pub name: String,
    /// The compiled expression producing its per-frame value.
    pub expr: Expr,
    /// This binding's easing constants (ADR-0019 / ADR-0035), read out of the
    /// preset's `[smoothing]` table **once, here at load**.
    /// [`Easing::INSTANT`] — the default for an unlisted param — means no
    /// smoothing. Resolved at parse time rather than looked up per binding per
    /// frame (Plan 0031 Phase 3); it is a fact about the preset, and the preset
    /// does not change while it renders.
    pub tau: Easing,
}

/// A loaded, ready-to-evaluate preset.
#[derive(Debug)]
pub struct Preset {
    /// Human-readable name (defaults to the system name if omitted).
    pub name: String,
    /// Which built-in system this preset drives.
    pub system: SystemKind,
    /// Parameter bindings, sorted by name for deterministic iteration.
    pub params: Vec<Binding>,
    /// Declarative structural config for a line scene (ADR-0007), applied once
    /// at preset load via `Scene::configure`. `None` for the fragment/swarm
    /// systems and for curve presets that accept the family default.
    pub config: Option<GeneratorConfig>,
    // The `[smoothing]` table itself is deliberately **not** kept: it is validated
    // at load and folded into each binding's `tau` there (Plan 0031 Phase 3), so
    // there is nothing left for a frame to look up. An entry naming a param this
    // preset does not bind was inert before and is inert now.
    /// Optional color palette selection (ADR-0021 / Plan 0020), from a `[palette]`
    /// table — a built-in `name` or custom `stops`, validated and baked-ready at
    /// this boundary. `None` means the default `spectrum` (the exact current
    /// cosine), so a preset without `[palette]` is visually unchanged. The
    /// renderer bakes it into a LUT and hands it to the active scene via
    /// `Scene::set_palette` on each preset switch.
    pub palette: Option<PaletteConfig>,
    /// Optional **second** palette (ADR-0021 / Plan 0020 Phase 4), from a
    /// `[palette_b]` table. When present, the renderer bakes an A/B pair and a
    /// bindable `palette_mix` param crossfades between them per frame. `None`
    /// means no crossfade (palette A only).
    pub palette_b: Option<PaletteConfig>,
    /// The salt this preset's `hash()`/`noise()` calls mix into their argument
    /// **in the live app** (ADR-0051): folded at load from the `[generator] seed`
    /// key that had been reserved and inert since Plan 0010, or drawn once from
    /// OS entropy where the preset declares `seed = "random"`. `0` when it
    /// declares nothing — a perfectly good salt, and the one the whole shipped
    /// library used before any preset asked for another.
    ///
    /// A load-time constant. Nothing per-frame recomputes it, and no expression
    /// can read it except through the two functions it salts.
    pub salt: u32,
    /// The salt every **capture** path uses in place of [`salt`](Self::salt):
    /// the declared number, or `0` for `seed = "random"`.
    ///
    /// Equal to `salt` unless the preset opted into per-run variety — the whole
    /// point of the pair (ADR-0051, following ADR-0045's tier pinning). The live
    /// app varies and the harness pins, so `shot`, the goldens, `--report` and
    /// the behavioral gates stay pure functions of their inputs while a preset
    /// can still be different every time the user starts the app.
    ///
    /// It is the *renderer* that chooses between the two, not the loader, and
    /// deliberately: `default_presets()` feeds both the live C-ABI path and the
    /// capture gates, so a decision taken at load would be wrong for one of them.
    pub pinned_salt: u32,
    /// Parameters whose `clamp()` bounds are **meant** to pin, from an
    /// `[occupancy] exempt = [...]` table (ADR-0062). Sorted and deduplicated at
    /// load.
    ///
    /// A safety rail exists to bind at peak, and the saturation gate would
    /// otherwise convict it of the defect it was written to prevent. The
    /// exemption silences `core/tests/saturation.rs`, and **only** that: the
    /// binding still appears in `--report`'s `occ` count and `SAT` lines,
    /// because an exemption is a place to hide and the one mitigation available
    /// is that it stays visible.
    ///
    /// A preset-level table naming params rather than a per-expression
    /// annotation, deliberately: the grammar stays a pure expression language
    /// (ADR-0020), and this is metadata *about* a binding rather than part of
    /// it. Harness-only — nothing per-frame reads it.
    pub occupancy_exempt: Vec<String>,
    /// Non-fatal problems found while loading — today, bindings naming a
    /// parameter this system does not consume (ADR-0020). The preset loaded and
    /// its good bindings apply; these are surfaced so a typo stops failing
    /// silently. Empty for a clean preset. Load-time only — never read per
    /// frame.
    pub warnings: Vec<String>,
}

impl Preset {
    /// Parse and compile a preset from a TOML source string.
    pub fn from_toml_str(src: &str) -> Result<Self, PresetError> {
        let raw: RawPreset = toml::from_str(src).map_err(PresetError::Toml)?;
        let system = SystemKind::from_name(&raw.system)
            .ok_or_else(|| PresetError::UnknownSystem(raw.system.clone()))?;
        let name = raw.name.unwrap_or_else(|| raw.system.clone());

        // The raw params come from a BTreeMap, so bindings land name-sorted:
        // evaluation is order-independent, but determinism is cheap to keep.
        let mut params = Vec::with_capacity(raw.params.len());
        let mut warnings = Vec::new();
        for (param, source) in raw.params {
            let expr = expr::compile(&source).map_err(|err| PresetError::Expr {
                param: param.clone(),
                err,
            })?;
            // A name the system does not consume is a warning, not an error:
            // one typo must not discard the rest of an otherwise-good preset
            // (ADR-0020 / NFR 10). The binding is kept — an unconsumed param is
            // harmless at apply time, and dropping it would turn a surfaced
            // warning back into a silent loss.
            if !is_known_param(system, &param) {
                warnings.push(format!(
                    "unknown parameter '{param}' for system '{}' (binding kept, but nothing reads it)",
                    system.as_str()
                ));
            }
            // `tau` is filled below, once the `[smoothing]` table has been
            // validated — keeping the validation where it is preserves which error
            // a preset with several problems reports first.
            params.push(Binding {
                name: param,
                expr,
                tau: Easing::INSTANT,
            });
        }

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
        )?;

        // Easing time constants (ADR-0019, ADR-0035): validated non-negative +
        // finite at the load boundary, then trusted by the render-layer smoother.
        // A bad value is a surfaced load error, never a panic. Both sides of an
        // `{ attack, release }` pair are checked, and the error names which one.
        for (param, entry) in &raw.smoothing {
            match *entry {
                RawSmoothing::Symmetric(seconds) => check_tau(param, None, seconds)?,
                RawSmoothing::Asymmetric { attack, release } => {
                    check_tau(param, Some("attack"), attack)?;
                    check_tau(param, Some("release"), release)?;
                }
            }
        }
        // Fold the validated table into the bindings, so the frame loop reads the
        // constants off the binding instead of hashing its name into a
        // `BTreeMap` once per binding per frame (Plan 0031 Phase 3).
        for binding in &mut params {
            binding.tau = raw
                .smoothing
                .get(&binding.name)
                .map_or(Easing::INSTANT, |entry| entry.to_easing());
            // A per-element binding (one naming `index`, Plan 0034 Phase 4) is
            // evaluated N times and delivered as a series; the smoother holds one
            // scalar and has no single value to ease. Rather than let the entry be
            // a silent no-op, say so — an author reaching for easing here wants
            // `[spectrum] smoothing`, which eases the element levels themselves.
            if binding.expr.uses_index() && raw.smoothing.contains_key(&binding.name) {
                warnings.push(format!(
                    "[smoothing] entry '{}' is ignored: the binding names `index`, so it is \
                     evaluated per element and cannot be eased as one value \
                     (use [spectrum] smoothing for the element levels)",
                    binding.name
                ));
                binding.tau = Easing::INSTANT;
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

        Ok(Preset {
            name,
            system,
            params,
            config,
            palette,
            palette_b,
            salt,
            pinned_salt,
            occupancy_exempt,
            warnings,
        })
    }
}

/// Fold a declared 64-bit `[generator] seed` into the 32-bit salt the grammar's
/// `hash()`/`noise()` mix in (ADR-0051).
///
/// XOR-folded rather than truncated, so two seeds differing only in their high
/// half still salt differently — `seed` is a `u64` in the schema and always has
/// been, and silently ignoring half of what an author typed is the kind of
/// surprise this file exists to prevent.
fn salt_from_seed(seed: u64) -> u32 {
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
fn entropy_salt() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::BuildHasher;
    salt_from_seed(RandomState::new().hash_one(0u64))
}

/// Assemble the optional structural config for `system` from the raw tables,
/// validating at this boundary (ADR-0007). Non-line systems have no config.
fn build_config(
    system: SystemKind,
    curve: Option<RawCurve>,
    generator: Option<RawGenerator>,
    particles: Option<RawParticles>,
    spectrum: Option<RawSpectrum>,
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
            let family = match particles {
                Some(p) => AttractorFamily::from_name(&p.family).ok_or_else(|| {
                    PresetError::Config(format!("unknown attractor family '{}'", p.family))
                })?,
                None => AttractorFamily::DeJong,
            };
            Ok(Some(GeneratorConfig::Particles { family }))
        }
        // The spectrum readout selects its element count, layout and per-element
        // easing through an optional `[spectrum]` table; absent, it takes the
        // defaults. Config is always `Some` so `configure` runs on every preset
        // switch (resizing the element buffer — never stale).
        SystemKind::Spectrum => Ok(Some(match spectrum {
            Some(s) => s.into_config()?,
            None => RawSpectrum::default().into_config()?,
        })),
        // Reaction-diffusion drives its regime through named params (feed/kill/
        // flow), not a declarative structural table.
        SystemKind::FragmentField | SystemKind::Swarm | SystemKind::ReactionDiffusion => Ok(None),
    }
}

/// The on-disk shape, before expressions are compiled.
#[derive(Deserialize)]
struct RawPreset {
    system: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    params: BTreeMap<String, String>,
    /// The optional `[curve]` structural-config table (ADR-0007), present on
    /// parametric-curve presets.
    #[serde(default)]
    curve: Option<RawCurve>,
    /// The optional `[generator]` structural-config table (ADR-0007), present on
    /// generator presets (L-system, star pattern).
    #[serde(default)]
    generator: Option<RawGenerator>,
    /// The optional `[particles]` structural-config table (Plan 0016), selecting
    /// the attractor family for the compute-particle scene.
    #[serde(default)]
    particles: Option<RawParticles>,
    /// The optional `[spectrum]` structural-config table (Plan 0034): the element
    /// count, layout and per-element easing of the spectrum readout.
    #[serde(default)]
    spectrum: Option<RawSpectrum>,
    /// The optional `[smoothing]` table (ADR-0019, ADR-0035): per-parameter
    /// easing time constants in seconds, each a scalar or an
    /// `{ attack, release }` pair. Absent means every param is applied instantly.
    #[serde(default)]
    smoothing: BTreeMap<String, RawSmoothing>,
    /// The optional `[palette]` color table (ADR-0021): a built-in `name` or
    /// custom `stops`. Absent means the default `spectrum` cosine.
    #[serde(default)]
    palette: Option<RawPalette>,
    /// The optional `[palette_b]` table (ADR-0021 / Phase 4): the crossfade
    /// target for a bindable `palette_mix`. Same shape as `[palette]`.
    #[serde(default)]
    palette_b: Option<RawPalette>,
    /// The optional `[occupancy]` table (ADR-0062): params whose `clamp()`
    /// bounds are meant to pin, exempted from the saturation gate. Absent means
    /// every clamp in this preset is held to it.
    #[serde(default)]
    occupancy: Option<RawOccupancy>,
}

/// The `[occupancy]` table, before validation.
#[derive(Debug, Default, Deserialize)]
struct RawOccupancy {
    /// Parameter names whose clamps may sit at their bound.
    #[serde(default)]
    exempt: Vec<String>,
}

/// One `[smoothing]` entry, before validation: today's scalar, or ADR-0035's
/// inline `{ attack = <seconds>, release = <seconds> }` pair.
///
/// Hand-deserialized rather than `#[serde(untagged)]` because an untagged enum
/// reports every failure as "data did not match any variant", which would make a
/// mistyped table strictly harder to diagnose than a mistyped float — the exact
/// regression ADR-0035 says not to ship.
#[derive(Debug, Clone, Copy)]
enum RawSmoothing {
    /// `hue = 0.4` — one constant in both directions.
    Symmetric(f32),
    /// `burst = { attack = 0.02, release = 0.7 }`.
    Asymmetric { attack: f32, release: f32 },
}

impl RawSmoothing {
    /// The validated pair this entry denotes. A scalar means both sides.
    fn to_easing(self) -> Easing {
        match self {
            Self::Symmetric(tau) => Easing::symmetric(tau),
            Self::Asymmetric { attack, release } => Easing { attack, release },
        }
    }
}

impl<'de> Deserialize<'de> for RawSmoothing {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(RawSmoothingVisitor)
    }
}

struct RawSmoothingVisitor;

impl<'de> serde::de::Visitor<'de> for RawSmoothingVisitor {
    type Value = RawSmoothing;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number of seconds, or a table { attack = <seconds>, release = <seconds> }")
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Ok(RawSmoothing::Symmetric(v as f32))
    }

    // TOML distinguishes `0.4` from `0`, and an author writing an instant
    // constant reaches for the integer.
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(RawSmoothing::Symmetric(v as f32))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(RawSmoothing::Symmetric(v as f32))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error;
        let mut attack = None;
        let mut release = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "attack" if attack.is_some() => return Err(A::Error::duplicate_field("attack")),
                "release" if release.is_some() => return Err(A::Error::duplicate_field("release")),
                "attack" => attack = Some(map.next_value::<f32>()?),
                "release" => release = Some(map.next_value::<f32>()?),
                // Naming both expected keys is the whole point: `atack = 0.02`
                // must not silently become an entry with a default attack.
                other => return Err(A::Error::unknown_field(other, &["attack", "release"])),
            }
        }
        match (attack, release) {
            (Some(attack), Some(release)) => Ok(RawSmoothing::Asymmetric { attack, release }),
            // Half a pair is a mistake, not a shorthand: silently defaulting the
            // missing side to instant would give the opposite of the requested
            // envelope on that direction.
            (None, _) => Err(A::Error::missing_field("attack")),
            (_, None) => Err(A::Error::missing_field("release")),
        }
    }
}

/// One easing constant, validated at the load boundary. `side` names which half
/// of an `{ attack, release }` pair failed; `None` is the scalar form, whose
/// message is unchanged from ADR-0019.
fn check_tau(param: &str, side: Option<&str>, seconds: f32) -> Result<(), PresetError> {
    if seconds.is_finite() && seconds >= 0.0 {
        return Ok(());
    }
    Err(PresetError::Config(match side {
        Some(side) => format!(
            "smoothing '{param}' {side} must be a non-negative number of seconds, got {seconds}"
        ),
        None => {
            format!("smoothing '{param}' must be a non-negative number of seconds, got {seconds}")
        }
    }))
}

/// The raw `[palette]` table: **either** a built-in palette `name` **or** custom
/// gradient `stops` (mutually exclusive). Validated at the load boundary.
#[derive(Deserialize)]
struct RawPalette {
    /// Built-in palette name (e.g. `"ember"`); validated at load.
    #[serde(default)]
    name: Option<String>,
    /// Custom gradient stops (`{ at = 0.0, color = "#rrggbb" }` or
    /// `{ at = 0.0, color = [r, g, b] }`); validated at load.
    #[serde(default)]
    stops: Option<Vec<RawStop>>,
}

/// One raw gradient stop: a position `at` in `0..=1` and a color.
#[derive(Deserialize)]
struct RawStop {
    at: f32,
    color: RawColor,
}

/// A raw stop color: a `#rrggbb` hex string or an `[r, g, b]` array of `0..1`
/// floats. `untagged` so either TOML form deserializes.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawColor {
    /// `"#rrggbb"` (the leading `#` optional).
    Hex(String),
    /// `[r, g, b]` with each channel a `0..1` float.
    Rgb([f32; 3]),
}

impl RawColor {
    /// Validate into an RGB triple, erroring (never panicking) on a malformed hex
    /// string or a non-finite channel.
    fn into_rgb(self) -> Result<[f32; 3], PresetError> {
        match self {
            RawColor::Hex(s) => parse_hex_color(&s),
            RawColor::Rgb(rgb) => {
                if rgb.iter().any(|c| !c.is_finite()) {
                    return Err(PresetError::Config(format!(
                        "[palette] stop color channels must be finite, got {rgb:?}"
                    )));
                }
                Ok([
                    rgb[0].clamp(0.0, 1.0),
                    rgb[1].clamp(0.0, 1.0),
                    rgb[2].clamp(0.0, 1.0),
                ])
            }
        }
    }
}

/// Parse a `#rrggbb` (or `rrggbb`) hex color into a `0..1` RGB triple. Every
/// failure is a surfaced load error, never a panic.
fn parse_hex_color(s: &str) -> Result<[f32; 3], PresetError> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(PresetError::Config(format!(
            "[palette] stop color '{s}' must be a #rrggbb hex string"
        )));
    }
    // All six chars are ASCII hex (checked above), so byte-slicing is safe.
    let channel = |lo: usize, hi: usize| -> f32 {
        u8::from_str_radix(&hex[lo..hi], 16)
            .map(|v| v as f32 / 255.0)
            .unwrap_or(0.0)
    };
    Ok([channel(0, 2), channel(2, 4), channel(4, 6)])
}

impl RawPalette {
    /// Validate the table into a [`PaletteConfig`], erroring (never panicking) on
    /// an unknown name, both selectors set, neither set, or a malformed stop list.
    /// `name` and `stops` are **mutually exclusive**: setting both is a load error
    /// (fail fast rather than silently pick one).
    fn into_config(self) -> Result<PaletteConfig, PresetError> {
        match (self.name, self.stops) {
            (Some(_), Some(_)) => Err(PresetError::Config(
                "[palette] sets both `name` and `stops`; use exactly one".into(),
            )),
            (Some(name), None) => {
                let named = NamedPalette::from_name(&name)
                    .ok_or_else(|| PresetError::Config(format!("unknown palette name '{name}'")))?;
                Ok(PaletteConfig::Named(named))
            }
            (None, Some(stops)) => Ok(PaletteConfig::Custom(validate_stops(stops)?)),
            (None, None) => Err(PresetError::Config(
                "[palette] needs a `name` or `stops`".into(),
            )),
        }
    }
}

/// Validate a custom stop list into the baked-ready `(at, rgb)` pairs: ≥2 stops,
/// each `at` finite in `0..=1` and non-decreasing (sorted), each color parseable.
/// Every failure is a surfaced load error (ADR-0021 / NFR 10).
fn validate_stops(stops: Vec<RawStop>) -> Result<Vec<(f32, [f32; 3])>, PresetError> {
    if stops.len() < 2 {
        return Err(PresetError::Config(format!(
            "[palette] needs at least 2 stops, got {}",
            stops.len()
        )));
    }
    let mut out = Vec::with_capacity(stops.len());
    let mut prev_at = f32::NEG_INFINITY;
    for stop in stops {
        if !stop.at.is_finite() || !(0.0..=1.0).contains(&stop.at) {
            return Err(PresetError::Config(format!(
                "[palette] stop `at` must be in 0..=1, got {}",
                stop.at
            )));
        }
        if stop.at < prev_at {
            return Err(PresetError::Config(
                "[palette] stops must be sorted by ascending `at`".into(),
            ));
        }
        prev_at = stop.at;
        out.push((stop.at, stop.color.into_rgb()?));
    }
    Ok(out)
}

/// The raw `[particles]` table: which strange-attractor family the
/// compute-particle scene iterates.
#[derive(Deserialize)]
struct RawParticles {
    /// Attractor family name (e.g. `"lorenz"`); validated at load.
    family: String,
}

/// The raw `[spectrum]` table: how the readout divides the frequency axis, what
/// figure the elements form, and how fast each element follows its band.
///
/// Every field is optional; an absent table is the same as an empty one, so
/// `system = "spectrum"` alone renders the default readout.
#[derive(Deserialize, Default)]
struct RawSpectrum {
    /// Element count; validated into `2..=SPECTRUM_BINS`.
    #[serde(default)]
    elements: Option<usize>,
    /// Layout name (`bars` / `polyline` / `radial_ring`); validated at load.
    #[serde(default)]
    layout: Option<String>,
    /// Per-element easing in seconds — a scalar or an `{ attack, release }`
    /// pair, exactly the `[smoothing]` vocabulary (ADR-0035).
    #[serde(default)]
    smoothing: Option<RawSmoothing>,
}

/// Default element count when a preset does not choose one — inside the "20-30
/// points" range the capability was asked for.
const DEFAULT_SPECTRUM_ELEMENTS: usize = 24;

impl RawSpectrum {
    /// Validate the table into a [`GeneratorConfig::Spectrum`], erroring (never
    /// panicking) on an out-of-range count, an unknown layout name, or a bad
    /// easing constant — the same load-boundary discipline every other
    /// declarative config follows (ADR-0007).
    fn into_config(self) -> Result<GeneratorConfig, PresetError> {
        let elements = self.elements.unwrap_or(DEFAULT_SPECTRUM_ELEMENTS);
        // The upper bound is the band count itself: above it the 64 -> N
        // reduction stops being a partition of the array (two elements would
        // have to share a band), and a readout finer than its own data is a lie
        // rather than a feature.
        if !(2..=crate::dsp::SPECTRUM_BINS).contains(&elements) {
            return Err(PresetError::Config(format!(
                "[spectrum] elements must be 2..={}, got {elements}",
                crate::dsp::SPECTRUM_BINS
            )));
        }
        let layout = match self.layout {
            Some(name) => SpectrumLayout::from_name(&name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [spectrum] layout '{name}' (expected one of: {})",
                    SpectrumLayout::NAMES.join(", ")
                ))
            })?,
            None => SpectrumLayout::default(),
        };
        let easing = match self.smoothing {
            Some(RawSmoothing::Symmetric(seconds)) => {
                check_tau("[spectrum] smoothing", None, seconds)?;
                Easing::symmetric(seconds)
            }
            Some(RawSmoothing::Asymmetric { attack, release }) => {
                check_tau("[spectrum] smoothing", Some("attack"), attack)?;
                check_tau("[spectrum] smoothing", Some("release"), release)?;
                Easing { attack, release }
            }
            None => Easing::INSTANT,
        };
        Ok(GeneratorConfig::Spectrum {
            elements,
            layout,
            easing,
        })
    }
}

/// One `[generator] seed` value, before resolution (ADR-0051): a number, or the
/// literal string `"random"`.
///
/// Hand-deserialized rather than `#[serde(untagged)]` for the same reason
/// [`RawSmoothing`] is: an untagged enum reports every failure as "data did not
/// match any variant", where a misspelled `seed = "randmo"` deserves to be told
/// what the accepted forms are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawSeed {
    /// `seed = 7` — a fixed salt, the same in the live app and in a capture.
    Fixed(u64),
    /// `seed = "random"` — drawn once per preset load in the live app, and
    /// pinned to the numeric fallback (`0`) on every capture path.
    Random,
}

impl RawSeed {
    /// The **declared** number: the value itself, or `0` for `"random"`. This is
    /// what a capture resolves to, and what the L-system's inert seed field keeps
    /// receiving.
    fn numeric(self) -> u64 {
        match self {
            Self::Fixed(n) => n,
            Self::Random => 0,
        }
    }
}

impl<'de> Deserialize<'de> for RawSeed {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(RawSeedVisitor)
    }
}

struct RawSeedVisitor;

impl serde::de::Visitor<'_> for RawSeedVisitor {
    type Value = RawSeed;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a non-negative integer, or the string \"random\"")
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(RawSeed::Fixed(v))
    }

    /// TOML has one integer type and it is **signed**, so a plain `seed = 7`
    /// arrives here rather than at `visit_u64`. A negative seed is rejected
    /// rather than reinterpreted as a huge unsigned one — the author meant
    /// something, and it was not `18446744073709551609`.
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        u64::try_from(v)
            .map(RawSeed::Fixed)
            .map_err(|_| E::invalid_value(serde::de::Unexpected::Signed(v), &self))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        match v {
            "random" => Ok(RawSeed::Random),
            other => Err(E::invalid_value(serde::de::Unexpected::Str(other), &self)),
        }
    }
}

/// The raw `[curve]` table: declarative structure for a parametric-curve scene.
#[derive(Deserialize)]
struct RawCurve {
    /// Curve family name (e.g. `"maurer_rose"`).
    family: String,
}

impl RawCurve {
    /// Validate the family name into a [`GeneratorConfig`], erroring (never
    /// panicking) on an unknown family.
    fn into_config(self) -> Result<GeneratorConfig, PresetError> {
        let family = CurveFamily::from_name(&self.family).ok_or_else(|| {
            PresetError::Config(format!("unknown curve family '{}'", self.family))
        })?;
        Ok(GeneratorConfig::Curve { family })
    }
}

/// The raw `[generator]` table: declarative structure for a generator scene.
/// Fields are optional at the serde layer and validated per system below, so
/// one table shape can serve the L-system (and, later, the star pattern).
#[derive(Deserialize)]
struct RawGenerator {
    /// L-system: starting string.
    #[serde(default)]
    axiom: Option<String>,
    /// L-system: production rules, each key a single predecessor character.
    #[serde(default)]
    rules: BTreeMap<String, String>,
    /// L-system: turn angle in degrees.
    #[serde(default)]
    angle_deg: Option<f32>,
    /// L-system: iterations to precompute.
    #[serde(default)]
    max_depth: Option<u32>,
    /// The preset's random salt — what the grammar's `hash()`/`noise()` mix into
    /// their argument (ADR-0051): a number, or `"random"` for a salt drawn per
    /// app launch. **Not** an L-system key despite living in the L-system's
    /// table: it was reserved here in Plan 0010 and stayed inert until Plan 0047
    /// gave it a meaning, and the expansion is still deterministic and still
    /// ignores it. Any system's preset may declare one.
    #[serde(default)]
    seed: Option<RawSeed>,
    /// Star pattern: the regular tiling (e.g. `"6.6.6"` / `"hexagon"` / `"12"`).
    #[serde(default)]
    tiling: Option<String>,
    /// Star pattern: contact angle in degrees.
    #[serde(default)]
    contact_angle_deg: Option<f32>,
}

impl RawGenerator {
    /// Validate the table as an L-system config: a non-empty axiom, single-char
    /// rule predecessors, a finite angle, and a depth in `1..=MAX_LSYSTEM_DEPTH`.
    /// Every failure is a surfaced load error, never a panic (ADR-0007).
    fn into_lsystem(self) -> Result<GeneratorConfig, PresetError> {
        let axiom = self
            .axiom
            .filter(|a| !a.is_empty())
            .ok_or_else(|| PresetError::Config("lsystem needs a non-empty axiom".into()))?;

        let mut rules = Vec::with_capacity(self.rules.len());
        for (pred, succ) in self.rules {
            let mut chars = pred.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return Err(PresetError::Config(format!(
                    "lsystem rule key '{pred}' must be a single character"
                )));
            };
            rules.push((c, succ));
        }
        if rules.is_empty() {
            return Err(PresetError::Config(
                "lsystem needs at least one rule".into(),
            ));
        }

        let angle_deg = self.angle_deg.unwrap_or(25.0);
        if !angle_deg.is_finite() {
            return Err(PresetError::Config(
                "lsystem angle_deg must be finite".into(),
            ));
        }

        let max_depth = self.max_depth.unwrap_or(4);
        if max_depth == 0 || max_depth > MAX_LSYSTEM_DEPTH {
            return Err(PresetError::Config(format!(
                "lsystem max_depth must be 1..={MAX_LSYSTEM_DEPTH}, got {max_depth}"
            )));
        }

        Ok(GeneratorConfig::LSystem {
            axiom,
            rules,
            angle_deg,
            max_depth,
            // Still inert (the expansion is deterministic and ignores it), so a
            // `"random"` seed reads as its numeric fallback here rather than
            // pulling entropy into a structural config.
            seed: self.seed.map_or(0, RawSeed::numeric),
        })
    }

    /// Validate the table as a star-pattern config: a known regular tiling and a
    /// finite contact angle. Every failure is a surfaced load error (ADR-0007).
    fn into_star(self) -> Result<GeneratorConfig, PresetError> {
        let tiling = self
            .tiling
            .ok_or_else(|| PresetError::Config("star_pattern needs a tiling".into()))?;
        let order = hankin::tiling_order(&tiling)
            .ok_or_else(|| PresetError::Config(format!("unknown tiling '{tiling}'")))?;

        let contact_angle_deg = self.contact_angle_deg.unwrap_or(30.0);
        if !contact_angle_deg.is_finite() {
            return Err(PresetError::Config(
                "star_pattern contact_angle_deg must be finite".into(),
            ));
        }

        Ok(GeneratorConfig::Star {
            order,
            contact_angle_deg,
        })
    }
}

/// Why a preset failed to load. Every variant is recoverable — the caller
/// keeps the previous good preset.
#[derive(Debug)]
pub enum PresetError {
    /// The TOML itself was malformed.
    Toml(toml::de::Error),
    /// `system` named a built-in that does not exist.
    UnknownSystem(String),
    /// A parameter's expression failed to compile.
    Expr {
        /// The parameter whose expression was invalid.
        param: String,
        /// The compile error.
        err: ExprError,
    },
    /// A structural-config table (`[curve]`/`[generator]`) was invalid — an
    /// unknown family, an out-of-range value, an undefined grammar symbol.
    Config(String),
    /// The preset file could not be read (message from the I/O error).
    Io(String),
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetError::Toml(e) => write!(f, "invalid preset TOML: {e}"),
            PresetError::UnknownSystem(s) => write!(f, "unknown system '{s}'"),
            PresetError::Expr { param, err } => {
                write!(f, "parameter '{param}' has an invalid expression: {err}")
            }
            PresetError::Config(msg) => write!(f, "invalid structural config: {msg}"),
            PresetError::Io(msg) => write!(f, "could not read preset file: {msg}"),
        }
    }
}

impl std::error::Error for PresetError {}

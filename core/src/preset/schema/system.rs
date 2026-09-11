//! [`SystemKind`]: which built-in system a preset drives, and the one roster
//! every other list of systems derives from.

use crate::render::scenes::{ParamKind, ParamSpec, declares, kind_of, spec_names};

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
    /// The mark roster drawn at frame scale as a signed-distance field —
    /// ADR-0105. The one scene whose palette coordinate is a *distance*,
    /// which is what makes `palette_steps` draw concentric offset contours
    /// of a shape.
    ShapeField,
    /// The ballistic emitter — objects that spawn, fall on a parabola and die
    /// (ADR-0057). The first scene whose population is not fixed.
    Emitter,
    /// The warp mesh — a per-vertex UV grid that resamples the previous frame
    /// (ADR-0113). Generalizes ADR-0048's single shared feedback transform to
    /// one transform *per vertex*, driven by a `[per_vertex]` table.
    WarpMesh,
    /// Flat opaque elements painted on their own paper, composited in painter
    /// order in one fullscreen distance-field pass (ADR-0123). The engine's
    /// first **graphic** world rather than a luminous one: the only system in
    /// which one object is genuinely in front of another.
    ShapeCollage,
    /// One fullscreen pass whose output is a closed-form function of position
    /// (ADR-0180 rule 1): the Chladni plate, and every later stateless
    /// per-pixel world, as a `[field] family` rather than a system each.
    AnalyticField,
}

/// **The** roster of built-in systems: every variant, its canonical name, and
/// the parameter names its scene consumes, in the order the engine builds their
/// scenes.
///
/// The single place all three lists live. [`SystemKind::ALL`],
/// [`SystemKind::from_name`], [`SystemKind::as_str`] and
/// [`SystemKind::param_names`] all read this, so they cannot disagree with each
/// other; what keeps *this* honest is [`SystemKind::row`], the one exhaustive
/// match over the enum, which fails the build when a variant has no entry.
///
/// The param lists themselves live beside each scene's own `set_param` match
/// (`declared_params_match_set_param` in `core/tests/preset.rs` guards that
/// pair); this is where they are gathered for the loader's typo check
/// (ADR-0020). They do **not** include the global compositing params, which any
/// preset may bind whatever its system -- [`is_known_param`] unions those in.
const TABLE: [(SystemKind, &str, &[ParamSpec]); SystemKind::VARIANT_COUNT] = {
    use crate::render::scenes;
    [
        (
            SystemKind::FragmentField,
            "fragment_field",
            scenes::fragment_field::PARAMS,
        ),
        (SystemKind::Swarm, "swarm", scenes::swarm::PARAMS),
        (
            SystemKind::ParametricCurve,
            "parametric_curve",
            scenes::lines::parametric::PARAMS,
        ),
        (
            SystemKind::LSystem,
            "lsystem",
            scenes::lines::lsystem::PARAMS,
        ),
        (
            SystemKind::StarPattern,
            "star_pattern",
            scenes::lines::star::PARAMS,
        ),
        (
            SystemKind::ReactionDiffusion,
            "reaction_diffusion",
            scenes::reaction_diffusion::PARAMS,
        ),
        (
            SystemKind::Attractor,
            "attractor",
            scenes::particles::PARAMS,
        ),
        (
            SystemKind::Spectrum,
            "spectrum",
            scenes::lines::spectrum::PARAMS,
        ),
        (SystemKind::Emitter, "emitter", scenes::emitter::PARAMS),
        (
            SystemKind::ShapeField,
            "shape_field",
            scenes::shape_field::PARAMS,
        ),
        (SystemKind::WarpMesh, "warp_mesh", scenes::warp_mesh::PARAMS),
        (
            SystemKind::ShapeCollage,
            "shape_collage",
            scenes::shape_collage::PARAMS,
        ),
        (
            SystemKind::AnalyticField,
            "analytic_field",
            scenes::analytic_field::PARAMS,
        ),
    ]
};

/// Every [`TABLE`] row sits at the index its own variant's [`SystemKind::row`]
/// names. Checked at compile time, because the two are written by hand and a
/// mismatch would silently give one system another's name and params.
const _: () = {
    let mut i = 0;
    while i < SystemKind::VARIANT_COUNT {
        assert!(
            TABLE[i].0.row() == i,
            "TABLE row order must match SystemKind::row"
        );
        i += 1;
    }
};

impl SystemKind {
    /// How many variants [`SystemKind`] has. Kept honest by `row`: a new
    /// variant fails the build there until it is rostered, and `TABLE` is
    /// typed off this count, so bumping the count without adding a row does not
    /// compile either. Both are module-private, so this names them rather than
    /// linking them.
    pub const VARIANT_COUNT: usize = 13;

    /// This variant's index into [`TABLE`].
    ///
    /// **The one exhaustive match over the enum, and the reason the roster
    /// cannot go stale**: a new variant makes this non-exhaustive and fails the
    /// build, which in turn forces a [`TABLE`] row, a scene into the exhaustive
    /// factory in `render::scenes`, and a fixture into the golden drift guard.
    const fn row(self) -> usize {
        match self {
            SystemKind::FragmentField => 0,
            SystemKind::Swarm => 1,
            SystemKind::ParametricCurve => 2,
            SystemKind::LSystem => 3,
            SystemKind::StarPattern => 4,
            SystemKind::ReactionDiffusion => 5,
            SystemKind::Attractor => 6,
            SystemKind::Spectrum => 7,
            SystemKind::Emitter => 8,
            SystemKind::ShapeField => 9,
            SystemKind::WarpMesh => 10,
            SystemKind::ShapeCollage => 11,
            SystemKind::AnalyticField => 12,
        }
    }

    /// Every [`SystemKind`], in the order the engine builds their scenes. The
    /// scene factory (`render::scenes::create_all`) and the golden drift guard
    /// both iterate this rather than keeping lists of their own.
    ///
    /// Typed `[SystemKind; VARIANT_COUNT]`, so a roster that has drifted from
    /// the variant count is a compile error, not a test failure.
    pub const ALL: [SystemKind; Self::VARIANT_COUNT] = {
        let mut out = [SystemKind::FragmentField; Self::VARIANT_COUNT];
        let mut i = 0;
        while i < Self::VARIANT_COUNT {
            out[i] = TABLE[i].0;
            i += 1;
        }
        out
    };

    /// Parse a canonical system name (as written in a preset's `system = "..."`
    /// field) into its [`SystemKind`], or `None` if unknown. The inverse of
    /// [`SystemKind::as_str`]; the `shot` CLI reuses the pair so it declares no
    /// match of its own.
    pub fn from_name(name: &str) -> Option<Self> {
        TABLE
            .iter()
            .find(|(_, canonical, _)| *canonical == name)
            .map(|(kind, _, _)| *kind)
    }

    /// The canonical name of this system -- the exact string
    /// [`SystemKind::from_name`] accepts and a preset writes in its `system`
    /// field.
    pub fn as_str(self) -> &'static str {
        TABLE[self.row()].1
    }

    /// The parameters this system's scene consumes, as the scene declares them
    /// (the module-private `TABLE`).
    ///
    /// The specs carry the default and the doc line as well as the name
    /// (ADR-0170), which is what lets the generated reference in
    /// `presets/README.md` be derived from the same declaration the loader
    /// checks a binding against.
    pub fn param_specs(self) -> &'static [ParamSpec] {
        TABLE[self.row()].2
    }

    /// Just the names, for a caller that wants to print or join them.
    ///
    /// Allocates. Use [`declares`] for a membership test, which is what almost
    /// every caller actually wants.
    pub fn param_names(self) -> Vec<&'static str> {
        spec_names(TABLE[self.row()].2)
    }
}

/// The parameter names any preset may bind regardless of its system: the five
/// compositing stages that run around the scene (`bg_*`, `trails`, `kaleido_*`,
/// `exposure`, `ink_*`/`paper_*`). Gathered from each stage's own declared
/// vocabulary so there is no third copy to drift.
///
/// **These do not all route through the renderer**, whatever the name suggests:
/// `trails` and `kaleido_*` are offered by the `PostChain` (ADR-0031),
/// `exposure` by the tonemap (ADR-0046) and `ink_*`/`paper_*` by the terminal ink
/// pass (ADR-0032); only `bg_*` goes to a pass the renderer drives directly. The
/// *names* are what this const is about — see `render::ParamRoute` for who
/// actually owns each.
pub const GLOBAL_PARAMS: [&[ParamSpec]; 7] = [
    crate::render::background::PARAMS,
    crate::render::trails::PARAMS,
    crate::render::kaleidoscope::PARAMS,
    crate::render::bloom::PARAMS,
    // The composite seam's own vocabulary (`occlude`, ADR-0085) — owned by the
    // chain rather than by any stage in it, which is why it is a seventh entry
    // and not part of one of the three above.
    crate::render::post::CHAIN_PARAMS,
    crate::render::tonemap::PARAMS,
    crate::render::ink::PARAMS,
];

/// Whether `name` is a parameter `system` (or the global compositing layer)
/// actually consumes. An unknown name is a load-time **warning**, not an error:
/// the preset still loads and applies its good bindings (ADR-0020, NFR 10).
pub fn is_known_param(system: SystemKind, name: &str) -> bool {
    declares(system.param_specs(), name) || GLOBAL_PARAMS.iter().any(|stage| declares(stage, name))
}

/// The [`ParamKind`] `name` is declared with, searching `system`'s own roster
/// first and then the global compositing stages — the same rosters in the same
/// order [`is_known_param`] tests, so a name that is known there has a kind
/// here.
///
/// [`ParamKind::Modal`] for a name no roster declares, which is the ADR-0020
/// warning case: the binding is kept, nothing reads it, and quantizing a value
/// no scene receives would be a decision about nothing.
pub fn kind_of_param(system: SystemKind, name: &str) -> ParamKind {
    kind_of(system.param_specs(), name)
        .or_else(|| GLOBAL_PARAMS.iter().find_map(|stage| kind_of(stage, name)))
        .unwrap_or_default()
}

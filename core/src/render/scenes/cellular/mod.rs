//! The cellular system: a discrete cellular automaton stepped on a ping-pong
//! grid (ADR-0012), with every rule space it runs as a named **family**
//! selected by `[cellular] family` (ADR-0180 rule 1).
//!
//! `life_like` is the birth/survival family: a dead cell with `k` live
//! neighbours is born when bit `k` of `birth` is set, and a live one survives
//! when bit `k` of `survive` is. Conway's Life is `birth = 8`, `survive = 12`.
//!
//! `larger_than_life` widens the neighbourhood to a box of side
//! `2 * radius + 1` and replaces the masks with two intervals over the box's
//! filled fraction: a dead cell is born inside `birth_lo..=birth_hi`, a live one
//! survives inside `survive_lo..=survive_hi`. The fractions are turned into
//! whole counts on the CPU (`interval_counts`), so the shader compares
//! integers. The box is summed **separated** — a row pass, then a column sum in
//! the step pass — at `2 * (2r + 1)` reads a cell rather than `(2r + 1)^2`, and
//! `radius` is capped by the tier on top of that
//! ([`TierConfig::cellular_radius`](crate::render::TierConfig::cellular_radius)).
//!
//! `cyclic` is the cyclic automaton: a cell holds one of `states` colours and
//! advances to the next round the cycle when at least `threshold` of its eight
//! neighbours already hold it. There is no dead state, so every cell is lit and
//! its palette coordinate is its state index over `states`, with no remap.
//!
//! # The grid is content, not a resolution
//!
//! `[cellular] grid` is how many cells a side holds, and a pattern is a fixed
//! number of cells — a glider is five — so the grid decides how large every
//! pattern looks. It is declared by the preset and never follows the window.
//! The tier caps it ([`TierConfig::cellular_grid`](crate::render::TierConfig::cellular_grid)),
//! and because the cap changes content rather than density, a grid clamped to
//! it is announced — returned from `configure` as a
//! [`CapOverflow`](super::CapOverflow) — never silently reduced. A bound
//! `radius` past the tier's cap is announced the same way, per frame, through
//! `Scene::mirror_overflow`.
//! The present is a plain normalized stretch of the grid over the target and
//! computes no screen-destined geometry, so there is no aspect for it to take
//! from the wrong place (ADR-0037).
//!
//! # Generations, not frames
//!
//! `step_rate` is in generations per second. `GenerationClock` integrates it
//! over the injected `dt` and hands `render` a whole number of generations to
//! run, so the automaton advances by the same count in the same wall time at
//! any refresh rate. Because an automaton is path-dependent, two runs at
//! different rates diverge for good — which is the meaning of the parameter,
//! not a defect of it.
//!
//! # Every cell is drawn from the seed
//!
//! The field is seeded, and every `reseed` refills a disc of it, from an
//! integer hash of the cell's coordinates and a seed derived from the preset's
//! pinned salt (ADR-0051) — never from a clock. The hash is `u32` arithmetic,
//! exact on every adapter, and each rule step is integer counting over exact
//! texel values, so a field is a pure function of its config and the sequence
//! of bound values and `dt`s it was driven with.
//!
//! # The age channel
//!
//! A binary field reads as noise; what reads as structure is its history. So
//! the state texture's second channel counts the generations since each cell
//! last changed state, and the present paints a dead cell by it: it fades out
//! over `trail` generations while sliding `age_tint` of the way along the
//! palette. `trail = 0` is the binary field exactly.
//!
//! **GPU resources are built lazily, on first render**, as the
//! reaction-diffusion scene's are and for its reason: a capture that never
//! activates this scene never builds them, so the WARP software adapter the
//! golden suite captures on never holds them beside another scene's.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

mod shader;

use super::common;
use super::lines::GeneratorConfig;
use super::{FamilyParam, FamilyRange, Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::feedback::PingPongField;
use crate::render::gpu;
use crate::render::palette::{self, Palette};
use crate::render::scenes::{ParamKind, ParamSpec, default_of};

/// Which rule space the automaton runs (ADR-0180 rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellularFamily {
    /// Birth and survival as two bitmasks over the eight-neighbour count —
    /// Conway's Life and every rule in its space.
    #[default]
    LifeLike,
    /// Birth and survival as two intervals over the filled fraction of a box of
    /// side `2 * radius + 1` — Evans's Larger than Life, whose rules carry
    /// travelling bugs and blobs where a radius-1 rule settles.
    LargerThanLife,
    /// The cyclic automaton: `states` colours in a cycle, a cell advancing to
    /// the next when at least `threshold` of its eight neighbours hold it.
    /// From noise it organizes into interlocking rotating spirals.
    Cyclic,
}

impl CellularFamily {
    /// Every family, in the order the step shader's family index numbers them
    /// and the generated reference lists them.
    pub const ALL: [CellularFamily; 3] = [
        CellularFamily::LifeLike,
        CellularFamily::LargerThanLife,
        CellularFamily::Cyclic,
    ];

    /// Parse the canonical `[cellular] family = "..."` value, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.as_str() == name)
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            CellularFamily::LifeLike => "life_like",
            CellularFamily::LargerThanLife => "larger_than_life",
            CellularFamily::Cyclic => "cyclic",
        }
    }

    /// The integer the step shader's family `switch` selects on: its position
    /// in [`ALL`](Self::ALL).
    fn index(self) -> u32 {
        match self {
            CellularFamily::LifeLike => 0,
            CellularFamily::LargerThanLife => 1,
            CellularFamily::Cyclic => 2,
        }
    }

    /// The fraction of cells a seed makes live. A sparse soup dies out under a
    /// larger neighbourhood's thresholds, so that family seeds denser. Unread
    /// for `cyclic`, whose seed is uniform over its colours.
    fn density(self) -> f32 {
        match self {
            CellularFamily::LifeLike | CellularFamily::Cyclic => LIFE_DENSITY,
            CellularFamily::LargerThanLife => LTL_DENSITY,
        }
    }

    /// Whether a generation needs the row pass first.
    fn sums_rows(self) -> bool {
        match self {
            CellularFamily::LifeLike | CellularFamily::Cyclic => false,
            CellularFamily::LargerThanLife => true,
        }
    }
}

/// `[cellular]` — the structural configuration, fixed while the preset is
/// loaded and delivered through `Scene::configure`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellularConfig {
    /// Which automaton runs.
    pub family: CellularFamily,
    /// Cells per side of the square grid. A content value — see the module
    /// docs — validated at load into [`MIN_GRID`]`..=`[`MAX_GRID`].
    pub grid: u32,
    /// `true`: the grid is a torus. `false`: every cell past the border is
    /// dead, and the present leaves the frame outside the grid empty.
    pub wrap: bool,
    /// The preset's pinned salt (ADR-0051), which every seeded cell is drawn
    /// from.
    pub salt: u32,
}

impl Default for CellularConfig {
    fn default() -> Self {
        Self {
            family: CellularFamily::default(),
            grid: DEFAULT_GRID,
            wrap: true,
            salt: 0,
        }
    }
}

/// The grid an absent `[cellular] grid` means: `reaction_diffusion`'s own 256,
/// for the same reason that scene pins it — pattern scale is content.
pub const DEFAULT_GRID: u32 = 256;
/// The smallest grid the loader accepts. Below it a glider's wake meets its own
/// head on a torus before it has read as motion.
pub const MIN_GRID: u32 = 16;
/// The largest grid the loader accepts: a million cells, two 8-byte texels each
/// across the ping-pong pair, 16 MB.
pub const MAX_GRID: u32 = 1024;

/// The largest rule bitmask: one bit for each neighbour count `0..=8`.
pub const MAX_RULE: f32 = 511.0;

/// The fastest `step_rate` the clock integrates, in generations per second.
/// Well past the declared range's top, so it bounds a runaway binding rather
/// than an author.
pub const MAX_STEP_RATE: f32 = 240.0;

/// The most generations one frame encodes. A stall would otherwise queue
/// unbounded passes (the accumulator spiral ADR-0012 names); past this the
/// backlog is dropped and the automaton briefly slows rather than racing. At
/// 30 fps it still carries 240 generations a second.
pub const MAX_GENERATIONS_PER_FRAME: u32 = 8;

/// How far below a whole generation the clock still counts one. `dt` arrives as
/// an `f32`, and `1/144` summed 144 times lands a few ulps either side of 1;
/// without this slack a rate that should run 12 generations in a second runs
/// 11 at one refresh rate and 12 at another.
const WHOLE_SLACK: f64 = 1e-6;

/// `reseed` rises past this to refill one disc — edge-triggered, so a value held
/// high refills once, not once per frame.
const RESEED_THRESHOLD: f32 = 0.5;
/// The refilled disc's radius, as a fraction of the grid's side.
const RESEED_RADIUS: f32 = 0.2;

/// The fraction of cells a `life_like` seed makes live.
const LIFE_DENSITY: f32 = 0.35;
/// The fraction of cells a `larger_than_life` seed makes live.
const LTL_DENSITY: f32 = 0.5;

/// The largest `radius` any tier lets a `larger_than_life` preset ask for —
/// the top of its declared range, and the constant the row and column loops run
/// to.
pub const MAX_RADIUS: f32 = 10.0;

/// The most colours a `cyclic` cycle may hold. A half float holds every state
/// index exactly far past this; the bound is where a cycle stops reading as a
/// cycle across a 256-texel palette.
pub const MAX_STATES: f32 = 24.0;
/// The most neighbours a `cyclic` threshold can ask for: all eight.
pub const MAX_THRESHOLD: f32 = 8.0;

/// How much slack an interval bound gets when it is turned into a whole count,
/// so a bound written as an exact fraction — `3/8` at radius 1 — keeps the
/// count it names on both ends despite `f32` rounding.
const INTERVAL_SLACK: f32 = 1e-4;

/// Where the age channel stops counting: generations since a cell last
/// changed, saturating here. A half float holds every whole number to 2048
/// exactly, so the count stays exact below this, and it is past any `trail`
/// the scene reads, so a saturated cell is simply "long ago".
pub(crate) const AGE_CAP: f32 = 1023.0;
/// The longest `trail` the present reads, in generations: the age channel's
/// own ceiling, since a wake cannot outlast the count it is read from.
pub const MAX_TRAIL: f32 = AGE_CAP;

/// Mixed into the preset's salt before it seeds the field, so a salt of `0`
/// still hashes to a field rather than to the hash's fixed point. The ASCII
/// bytes of "CELL"; opaque, since changing it moves every seeded field.
const FIELD_SEED_MIX: u32 = 0x4345_4C4C;
/// Mixed into the salt for the stream of reseed discs, so the discs are not
/// drawn from the same sequence as the field. The ASCII bytes of "RSEEDCL1".
const STAMP_SEED_MIX: u64 = 0x5253_4545_4443_4C31;

const DEFAULT_BIRTH: f32 = default_of(PARAMS, "birth");
const DEFAULT_SURVIVE: f32 = default_of(PARAMS, "survive");
const DEFAULT_STEP_RATE: f32 = default_of(PARAMS, "step_rate");
const DEFAULT_TRAIL: f32 = default_of(PARAMS, "trail");
const DEFAULT_RADIUS: f32 = default_of(PARAMS, "radius");
const DEFAULT_BIRTH_LO: f32 = default_of(PARAMS, "birth_lo");
const DEFAULT_BIRTH_HI: f32 = default_of(PARAMS, "birth_hi");
const DEFAULT_SURVIVE_LO: f32 = default_of(PARAMS, "survive_lo");
const DEFAULT_SURVIVE_HI: f32 = default_of(PARAMS, "survive_hi");
const DEFAULT_STATES: f32 = default_of(PARAMS, "states");
const DEFAULT_THRESHOLD: f32 = default_of(PARAMS, "threshold");
const DEFAULT_AGE_TINT: f32 = default_of(PARAMS, "age_tint");
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_ZOOM: f32 = 1.0;

/// The seed the whole field is hashed from, for a preset whose salt is `salt`.
pub(crate) fn field_seed(salt: u32) -> u32 {
    salt ^ FIELD_SEED_MIX
}

/// A density as the step shader compares it: the fraction of the 24-bit hash
/// range under which a cell is seeded live.
pub(crate) fn live_threshold(density: f32) -> u32 {
    (density.clamp(0.0, 1.0) * 16_777_216.0) as u32
}

/// A bound rule bitmask as the step shader reads it: clamped into
/// `0..=`[`MAX_RULE`] and rounded, so every neighbour count names one bit. A
/// non-finite value falls back to the declared default.
pub(crate) fn applied_rule(value: f32, default: f32) -> u32 {
    let v = if value.is_finite() { value } else { default };
    v.clamp(0.0, MAX_RULE).round() as u32
}

/// A bound `step_rate` as the clock integrates it: clamped into
/// `0..=`[`MAX_STEP_RATE`], the declared default where it is not finite.
pub(crate) fn applied_step_rate(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, MAX_STEP_RATE)
    } else {
        DEFAULT_STEP_RATE
    }
}

/// A bound `trail` as the present reads it: clamped into `0..=`[`MAX_TRAIL`],
/// the declared default where it is not finite. The shader divides by
/// `trail + 1`, which this keeps at least 1.
pub(crate) fn applied_trail(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, MAX_TRAIL)
    } else {
        DEFAULT_TRAIL
    }
}

/// A bound `radius` as the step shader reads it: clamped into `1..=cap` and
/// rounded, where `cap` is the tier's
/// [`cellular_radius`](crate::render::TierConfig::cellular_radius) held inside
/// [`MAX_RADIUS`]. A non-finite value falls back to the declared default,
/// itself capped.
pub(crate) fn applied_radius(value: f32, cap: f32) -> u32 {
    let cap = cap.clamp(1.0, MAX_RADIUS);
    let v = if value.is_finite() {
        value
    } else {
        DEFAULT_RADIUS
    };
    v.clamp(1.0, cap).round() as u32
}

/// The clamp to report for a bound radius of `value` under `cap`, or `None`
/// when the preset asked within it — or runs a family that reads no radius,
/// where there is nothing to clamp. `value` is compared as the shader would
/// take it before the cap, so a bound 6.4 under a cap of 6 is not a clamp.
pub(crate) fn radius_clamp(
    family: CellularFamily,
    value: f32,
    cap: f32,
) -> Option<super::CapOverflow> {
    if family != CellularFamily::LargerThanLife {
        return None;
    }
    let asked = applied_radius(value, MAX_RADIUS);
    let applied = applied_radius(value, cap);
    (asked > applied).then_some(super::CapOverflow {
        dropped: (asked - applied) as usize,
        context: super::OverflowContext::Radius(asked),
        cap: applied as usize,
    })
}

/// `config` with its grid held to `cap`, and the clamp to report if that moved
/// it. A grid is structural, so this runs once, at `configure`.
pub(crate) fn grid_clamp(
    config: CellularConfig,
    cap: u32,
) -> (CellularConfig, Option<super::CapOverflow>) {
    if config.grid <= cap {
        return (config, None);
    }
    let overflow = super::CapOverflow {
        dropped: (config.grid - cap) as usize,
        context: super::OverflowContext::Grid(config.grid),
        cap: cap as usize,
    };
    (
        CellularConfig {
            grid: cap,
            ..config
        },
        Some(overflow),
    )
}

/// A bound `states` as the step and present passes read it: clamped into
/// `2..=`[`MAX_STATES`] and rounded — a cycle of one colour never advances,
/// and a fractional count names no cycle. A non-finite value falls back to the
/// declared default.
pub(crate) fn applied_states(value: f32) -> u32 {
    let v = if value.is_finite() {
        value
    } else {
        DEFAULT_STATES
    };
    v.clamp(2.0, MAX_STATES).round() as u32
}

/// A bound `threshold` as the step pass reads it: clamped into
/// `1..=`[`MAX_THRESHOLD`] and rounded. A non-finite value falls back to the
/// declared default.
pub(crate) fn applied_threshold(value: f32) -> u32 {
    let v = if value.is_finite() {
        value
    } else {
        DEFAULT_THRESHOLD
    };
    v.clamp(1.0, MAX_THRESHOLD).round() as u32
}

/// How many cells the box of `radius` holds besides its centre: the
/// denominator of every `larger_than_life` fraction.
pub(crate) fn neighbourhood(radius: u32) -> u32 {
    let side = 2 * radius + 1;
    side * side - 1
}

/// A `larger_than_life` interval of filled fractions `[lo, hi]` as the whole
/// neighbour counts it admits, `[first, last]` inclusive, over a box of
/// `radius`. An empty interval — `hi` below `lo`, or one that holds no whole
/// count — comes out with `first > last`, which the shader's comparison admits
/// nothing through. Non-finite bounds fall back to `fallback`.
pub(crate) fn interval_counts(lo: f32, hi: f32, radius: u32, fallback: [f32; 2]) -> [u32; 2] {
    let n = neighbourhood(radius) as f32;
    let lo = if lo.is_finite() { lo } else { fallback[0] };
    let hi = if hi.is_finite() { hi } else { fallback[1] };
    let first = (lo.clamp(0.0, 1.0) * n - INTERVAL_SLACK).ceil().max(0.0);
    let last = (hi.clamp(0.0, 1.0) * n + INTERVAL_SLACK).floor();
    if last < first {
        // `u32::MAX` then `0`: no count is both at least one and at most the
        // other.
        return [u32::MAX, 0];
    }
    [first as u32, last as u32]
}

/// Integrates a generation rate over injected `dt` into whole generations.
///
/// **No clock**: `dt` is handed in, as every accumulator in this engine takes
/// it, so a capture stepping a fixed `dt` sequence is reproducible. The sum is
/// `f64`, because a rate times an `f32` `dt` summed over minutes loses the
/// fraction that decides whether a generation is owed.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct GenerationClock {
    /// Generations owed and not yet run, in `[0, 1)` between calls.
    owed: f64,
}

impl GenerationClock {
    /// Add `dt` seconds at `rate` generations per second and return how many
    /// whole generations to run now, at most [`MAX_GENERATIONS_PER_FRAME`].
    /// The fraction carries; a backlog past the cap is dropped rather than
    /// carried, so the automaton slows instead of racing to catch up.
    pub(crate) fn advance(&mut self, rate: f32, dt: f32) -> u32 {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.owed += f64::from(applied_step_rate(rate)) * f64::from(dt);
        let whole = (self.owed + WHOLE_SLACK).floor();
        let cap = f64::from(MAX_GENERATIONS_PER_FRAME);
        if whole > cap {
            self.owed = 0.0;
            return MAX_GENERATIONS_PER_FRAME;
        }
        self.owed = (self.owed - whole).max(0.0);
        whole as u32
    }
}

/// One scheduled `reseed`: the disc's centre in cells, its radius squared in
/// cells², and the seed its cells are hashed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub(crate) centre: [u32; 2],
    pub(crate) radius_sq: u32,
    pub(crate) seed: u32,
}

/// The stream a preset's reseed discs are drawn from.
fn stamp_rng(salt: u32) -> SeededRng {
    SeededRng::new(u64::from(salt) ^ STAMP_SEED_MIX)
}

/// Draw the next disc for a grid of `grid` cells a side.
fn next_stamp(rng: &mut SeededRng, grid: u32) -> Stamp {
    let side = grid.max(1);
    let cell = |u: f32| ((u * side as f32) as u32).min(side - 1);
    let x = cell(rng.next_f32());
    let y = cell(rng.next_f32());
    let seed = (rng.next_f32() * 16_777_216.0) as u32;
    let radius = RESEED_RADIUS * side as f32;
    Stamp {
        centre: [x, y],
        radius_sq: (radius * radius) as u32,
        seed,
    }
}

/// The parameter names this scene consumes — the vocabulary a preset binding is
/// checked against at load (ADR-0020). **Keep in sync with `set_param` below**;
/// `declared_params_match_set_param` in `core/tests/preset.rs` fails if the two
/// drift.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "birth",
        default: 8.0,
        range: Some([0.0, MAX_RULE]),
        doc: "Which live-neighbour counts bring a dead cell to life, as a bitmask over the \
              counts 0-8: bit k set means k neighbours give birth. 8 (bit 3) is Conway's.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "survive",
        default: 12.0,
        range: Some([0.0, MAX_RULE]),
        doc: "Which live-neighbour counts keep a live cell alive, as a bitmask over the counts \
              0-8. 12 (bits 2 and 3) is Conway's.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "radius",
        default: 5.0,
        range: Some([1.0, MAX_RADIUS]),
        doc: "How far the neighbourhood reaches, in cells: a square of side 2 x radius + 1 \
              about each cell. Capped by the quality tier.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "birth_lo",
        default: 0.28,
        range: Some([0.0, 1.0]),
        doc: "The least filled fraction of its neighbourhood at which a dead cell is born.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "birth_hi",
        default: 0.385,
        range: Some([0.0, 1.0]),
        doc: "The most filled fraction of its neighbourhood at which a dead cell is born.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "survive_lo",
        default: 0.26,
        range: Some([0.0, 1.0]),
        doc: "The least filled fraction of its neighbourhood at which a live cell survives.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "survive_hi",
        default: 0.47,
        range: Some([0.0, 1.0]),
        doc: "The most filled fraction of its neighbourhood at which a live cell survives.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "states",
        default: 3.0,
        range: Some([2.0, MAX_STATES]),
        doc: "How many colours the cycle holds; each cell advances to the next one round it.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "threshold",
        default: 3.0,
        range: Some([1.0, MAX_THRESHOLD]),
        doc: "How many of its eight neighbours must already hold the next colour before a cell \
              advances to it.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "step_rate",
        default: 10.0,
        range: Some([0.0, 60.0]),
        doc: "How many generations the automaton runs per second, whatever the frame rate; 0 \
              freezes it.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "reseed",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "A rise past 0.5 refills one disc of the grid with fresh seeded cells, once per \
              rise; bind a beat or a latch to it.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "trail",
        default: 12.0,
        range: Some([0.0, 64.0]),
        doc: "How many generations a dead cell keeps glowing, fading as it goes; 0 draws only \
              the live cells.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "age_tint",
        default: 0.35,
        range: Some([0.0, 1.0]),
        doc: "How far along the palette a dead cell's glow travels as it fades; 0 keeps the \
              wake the live cells' colour.",
        kind: ParamKind::Modal,
    },
    common::brightness(common::DEFAULT_BRIGHTNESS),
    common::hue(DEFAULT_HUE),
    common::zoom(DEFAULT_ZOOM),
    common::PAN_X,
    common::PAN_Y,
    common::SATURATION,
    common::PALETTE_MIX,
    common::PALETTE_STEPS,
    common::PALETTE_CONTOUR,
];

/// One row of [`FAMILY_PARAMS`], its ranges in [`CellularFamily::ALL`]'s
/// order. `None` is a family that does not read the parameter.
macro_rules! per_family {
    ($name:literal: $life_like:expr, $larger_than_life:expr, $cyclic:expr $(,)?) => {
        FamilyParam {
            name: $name,
            ranges: &[
                FamilyRange {
                    family: "life_like",
                    range: $life_like,
                },
                FamilyRange {
                    family: "larger_than_life",
                    range: $larger_than_life,
                },
                FamilyRange {
                    family: "cyclic",
                    range: $cyclic,
                },
            ],
        }
    };
}

/// Every parameter only some families read (ADR-0180 rule 4), with the range
/// that reads there — so the generated reference prints `birth` as
/// `life_like`'s and inert elsewhere. A parameter missing from here reads the
/// same on every family.
pub const FAMILY_PARAMS: &[FamilyParam] = &[
    per_family!("birth": Some([0.0, MAX_RULE]), None, None),
    per_family!("survive": Some([0.0, MAX_RULE]), None, None),
    per_family!("radius": None, Some([1.0, MAX_RADIUS]), None),
    per_family!("birth_lo": None, Some([0.0, 1.0]), None),
    per_family!("birth_hi": None, Some([0.0, 1.0]), None),
    per_family!("survive_lo": None, Some([0.0, 1.0]), None),
    per_family!("survive_hi": None, Some([0.0, 1.0]), None),
    per_family!("states": None, None, Some([2.0, MAX_STATES])),
    per_family!("threshold": None, None, Some([1.0, MAX_THRESHOLD])),
    // Every cyclic cell is lit and coloured by its state, so the wake reads
    // nothing there.
    per_family!("trail": Some([0.0, 64.0]), Some([0.0, 64.0]), None),
    per_family!("age_tint": Some([0.0, 1.0]), Some([0.0, 1.0]), None),
];

/// The one uniform every step-shader pass reads, written once a frame.
///
/// **One buffer for all three passes, and that is load-bearing.** Which pass is
/// running is a constant compiled into its pipeline ([`StepPass`]), never a
/// field here: on the DX12 WARP software adapter, bind groups built on one
/// layout that differ only in their uniform buffer are not told apart, and a
/// seed pass bound to a uniform of its own read the step pass's instead. With
/// one buffer and one texture pair, every pass is bound to identical resources,
/// so there is nothing to confuse.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StepParams {
    /// x: family index, y: wrap (1 torus), z: grid (cells per side), w: live
    /// threshold against the hash's top 24 bits.
    a: [u32; 4],
    /// x: birth mask, y: survive mask, z: radius (cells), w: cyclic's states.
    b: [u32; 4],
    /// x: the field's seed, y: the stamp's seed, z: stamp radius squared
    /// (cells²), w: unused.
    c: [u32; 4],
    /// xy: stamp centre (cells), z: cyclic's threshold, w: unused.
    d: [u32; 4],
    /// `larger_than_life`'s two intervals as whole neighbour counts,
    /// inclusive: xy births a dead cell, zw keeps a live one.
    e: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PresentParams {
    /// x: hue, y: brightness, z: saturation, w: palette_mix.
    a: [f32; 4],
    /// x: palette_steps, y: palette_contour, z: occlude, w: grid.
    b: [f32; 4],
    /// x: zoom, yz: pan, w: wrap (1 torus).
    c: [f32; 4],
    /// x: trail (generations), y: age_tint, z: 1 for `cyclic`, w: its states.
    d: [f32; 4],
}

/// The two bind groups of one pass over the ping-pong pair — reading texture A
/// and reading texture B — so nothing is rebuilt on the hot path.
struct ReadPair {
    a: wgpu::BindGroup,
    b: wgpu::BindGroup,
}

impl ReadPair {
    fn for_field(&self, field: &PingPongField) -> &wgpu::BindGroup {
        if field.reading_a() { &self.a } else { &self.b }
    }
}

/// The GPU-side state, built lazily on first render (see the module docs) and
/// rebuilt when a preset asks for a different grid.
struct Resources {
    /// The grid these textures were built for.
    grid: u32,
    field: PingPongField,
    /// Each cell's live count along its row, written by the row pass and summed
    /// down the column by the step pass — `larger_than_life`'s separated box.
    /// Kept alive so its view stays valid, as `PingPongField` keeps its pair.
    _rows: wgpu::Texture,
    rows_view: wgpu::TextureView,
    /// The step shader compiled once per [`StepPass`], all three over one
    /// layout and bound to the same resources.
    seed_pipeline: wgpu::RenderPipeline,
    stamp_pipeline: wgpu::RenderPipeline,
    step_pipeline: wgpu::RenderPipeline,
    rows_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    step_uniform: wgpu::Buffer,
    present_uniform: wgpu::Buffer,
    step_bg: ReadPair,
    rows_bg: ReadPair,
    present_bg: ReadPair,
    /// The shared gradient LUT pair (ADR-0021). A fresh pair is dirty, so a
    /// (re)build uploads on its first frame.
    luts: palette::LutPair,
}

impl Resources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat, grid: u32) -> Self {
        let present_shader = gpu::fullscreen_shader(
            device,
            "cellular-present-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            shader::PRESENT_SHADER,
        );

        let field = PingPongField::new(device, grid, grid);

        let step_uniform = gpu::uniform_buffer(
            device,
            "cellular-step-params",
            std::mem::size_of::<StepParams>(),
        );
        let present_uniform = gpu::uniform_buffer(
            device,
            "cellular-present-params",
            std::mem::size_of::<PresentParams>(),
        );

        let rows = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cellular-rows"),
            size: wgpu::Extent3d {
                width: grid,
                height: grid,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // The field's own format: a row count is a whole number no larger
            // than `2 * MAX_RADIUS + 1`, which a half float holds exactly.
            format: PingPongField::FORMAT,
            // `COPY_SRC` only so a probe can read the counts back, as the
            // field's own textures carry it.
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let rows_view = rows.create_view(&wgpu::TextureViewDescriptor::default());

        // `[Texture, Texture, Uniform]` — the field, the row counts, the
        // uniform — is a shape no other layout in the crate has (ADR-0058).
        let step_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cellular-step-layout"),
            entries: &[
                gpu::texture(0, false),
                gpu::texture(1, false),
                gpu::uniform(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let step_bg = ReadPair {
            a: step_bind_group(
                device,
                &step_layout,
                field.view_a(),
                &rows_view,
                &step_uniform,
            ),
            b: step_bind_group(
                device,
                &step_layout,
                field.view_b(),
                &rows_view,
                &step_uniform,
            ),
        };
        // The row pass reads the field and the same uniform. `[Texture,
        // Uniform]` is unique too; the reaction-diffusion sim binds the same
        // two entries the other way round, so swapping these would collide.
        let rows_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cellular-rows-layout"),
            entries: &[
                gpu::texture(0, false),
                gpu::uniform(1, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let rows_bg = ReadPair {
            a: rows_bind_group(device, &rows_layout, field.view_a(), &step_uniform),
            b: rows_bind_group(device, &rows_layout, field.view_b(), &step_uniform),
        };
        let rows_shader = gpu::fullscreen_shader(
            device,
            "cellular-rows",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &format!(
                "{}{}{}",
                radius_const(),
                shader::STEP_COMMON,
                shader::ROWS_SHADER
            ),
        );
        let rows_pipeline = gpu::fullscreen_pipeline(
            device,
            &rows_shader,
            &[&rows_layout],
            PingPongField::FORMAT,
            wgpu::BlendState::REPLACE,
            "cellular-rows",
        );
        let step_pipeline_for = |pass: StepPass| {
            let label = pass.label();
            let shader = gpu::fullscreen_shader(
                device,
                label,
                gpu::FULLSCREEN_VS_UV_FLIPPED,
                &pass.source(),
            );
            gpu::fullscreen_pipeline(
                device,
                &shader,
                &[&step_layout],
                PingPongField::FORMAT,
                wgpu::BlendState::REPLACE,
                label,
            )
        };
        let seed_pipeline = step_pipeline_for(StepPass::Seed);
        let stamp_pipeline = step_pipeline_for(StepPass::Stamp);
        let step_pipeline = step_pipeline_for(StepPass::Step);

        let luts = palette::LutPair::new(device, "cellular");
        // `[Texture, Texture, Texture, Sampler, Uniform]`: the field, the LUT
        // pair and its sampler, then the uniform — unique in the crate
        // (ADR-0058).
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cellular-present-layout"),
            entries: &[
                gpu::texture(0, false),
                gpu::texture(1, true),
                gpu::texture(2, true),
                gpu::sampler(3),
                gpu::uniform(4, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let present_bg = ReadPair {
            a: present_bind_group(
                device,
                &present_layout,
                field.view_a(),
                &luts,
                &present_uniform,
            ),
            b: present_bind_group(
                device,
                &present_layout,
                field.view_b(),
                &luts,
                &present_uniform,
            ),
        };
        let present_pipeline = gpu::fullscreen_pipeline(
            device,
            &present_shader,
            &[&present_layout],
            surface_format,
            // Premultiplied OVER the backdrop (ADR-0026): a dead cell emits no
            // light and no coverage, so the `bg_*` backdrop shows through it.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "cellular-present",
        );

        Self {
            grid,
            field,
            _rows: rows,
            rows_view,
            seed_pipeline,
            stamp_pipeline,
            step_pipeline,
            rows_pipeline,
            present_pipeline,
            step_uniform,
            present_uniform,
            step_bg,
            rows_bg,
            present_bg,
            luts,
        }
    }

    /// Encode one `pass` of the step shader: read the current field, write the
    /// other texture, swap.
    fn encode_step(&mut self, encoder: &mut wgpu::CommandEncoder, pass: StepPass) {
        let pipeline = match pass {
            StepPass::Seed => &self.seed_pipeline,
            StepPass::Stamp => &self.stamp_pipeline,
            StepPass::Step => &self.step_pipeline,
        };
        {
            // The step pass writes every texel, so what it clears to is never
            // read.
            let mut rpass = gpu::color_pass(
                encoder,
                pass.label(),
                self.field.write_view(),
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            );
            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, self.step_bg.for_field(&self.field), &[]);
            rpass.draw(0..3, 0..1);
        }
        self.field.swap();
    }

    /// Encode the row pass over the current field — the first half of a
    /// `larger_than_life` generation. Writes the row counts and leaves the
    /// field where it was.
    fn encode_rows(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut rpass = gpu::color_pass(
            encoder,
            "cellular-rows",
            &self.rows_view,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        rpass.set_pipeline(&self.rows_pipeline);
        rpass.set_bind_group(0, self.rows_bg.for_field(&self.field), &[]);
        rpass.draw(0..3, 0..1);
    }
}

/// The `MAX_RADIUS` constant the row and step shaders' loops run to.
fn radius_const() -> String {
    format!("const MAX_RADIUS: i32 = {};\n", MAX_RADIUS as i32)
}

/// What one pass of the step shader does: its `MODE` constant, compiled in.
#[derive(Clone, Copy)]
enum StepPass {
    /// Hash every cell from the field's seed.
    Seed,
    /// Hash the cells inside the scheduled disc from the stamp's seed.
    Stamp,
    /// Advance every cell one generation by the family's rule.
    Step,
}

impl StepPass {
    /// The `MODE` the step shader's `switch` reads.
    fn mode(self) -> u32 {
        match self {
            StepPass::Step => 0,
            StepPass::Seed => 1,
            StepPass::Stamp => 2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            StepPass::Seed => "cellular-seed",
            StepPass::Stamp => "cellular-stamp",
            StepPass::Step => "cellular-step",
        }
    }

    /// This pass's step shader after the vertex prelude: its constants, the
    /// hash, the shared block and the body. Built at construction only.
    fn source(self) -> String {
        format!(
            "const MODE: u32 = {}u;\nconst AGE_CAP: f32 = {:?};\n{}{}{}{}",
            self.mode(),
            AGE_CAP,
            radius_const(),
            gpu::HASH_WGSL,
            shader::STEP_COMMON,
            shader::STEP_SHADER
        )
    }
}

fn step_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    input: &wgpu::TextureView,
    rows: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cellular-step-bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(input),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(rows),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

fn rows_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    input: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cellular-rows-bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(input),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

fn present_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    input: &wgpu::TextureView,
    luts: &palette::LutPair,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let [lut_a, lut_b, lut_sampler] = luts.bind_entries(1, 2, 3);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cellular-present-bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(input),
            },
            lut_a,
            lut_b,
            lut_sampler,
            wgpu::BindGroupEntry {
                binding: 4,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

/// A discrete cellular automaton on a ping-pong grid, driven by named preset
/// parameters and one `[cellular]` table.
pub struct CellularScene {
    /// Cloned device handle that builds [`Resources`] lazily on first render.
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    /// The `[cellular]` table of the preset last configured.
    config: CellularConfig,
    /// Whether the next `render` seeds the field before stepping it — set by a
    /// build and by every `configure`.
    needs_seed: bool,
    clock: GenerationClock,
    /// This frame's `dt`, stored by `advance` and integrated in `update`, where
    /// this frame's `step_rate` has landed.
    dt: f32,
    /// Generations `update` scheduled for the next `render` to encode.
    pending_generations: u32,
    /// The stream reseed discs are drawn from, reset by `configure` so a
    /// preset replays its discs identically.
    stamp_rng: SeededRng,
    /// A disc scheduled by a `reseed` rising edge for the next `render`.
    pending_stamp: Option<Stamp>,
    /// Last frame's `reseed`, for the rising edge.
    prev_reseed: f32,
    /// The tier's [`cellular_radius`](crate::render::TierConfig::cellular_radius):
    /// the widest neighbourhood a bound `radius` reaches the shader with.
    radius_cap: f32,
    /// The tier's [`cellular_grid`](crate::render::TierConfig::cellular_grid):
    /// the largest grid a preset runs on.
    grid_cap: u32,
    /// This frame's clamp of a bound `radius` to [`radius_cap`](Self::radius_cap),
    /// or `None` when the preset asked within it — read back through
    /// [`Scene::mirror_overflow`] so the frontend announces it. A `Copy` value
    /// rewritten each frame, so reporting it allocates nothing.
    clamp: Option<super::CapOverflow>,
    birth: f32,
    survive: f32,
    radius: f32,
    birth_lo: f32,
    birth_hi: f32,
    survive_lo: f32,
    survive_hi: f32,
    states: f32,
    threshold: f32,
    step_rate: f32,
    reseed: f32,
    trail: f32,
    age_tint: f32,
    colour: common::PaletteParams,
    pan: common::PanParams,
    zoom: f32,
    /// How much of this scene's coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame through
    /// [`Scene::set_occlude`] — not a named param, so `reset_params` leaves it
    /// alone.
    occlude: f32,
    /// The active baked palette, held here because the resources build lazily
    /// and `set_palette` can arrive before they exist.
    palette: Palette,
}

impl CellularScene {
    /// The CPU-side state, holding a bound `larger_than_life` radius to
    /// `radius_cap` and a preset's grid to `grid_cap` — the tier's
    /// [`cellular_radius`](crate::render::TierConfig::cellular_radius) and
    /// [`cellular_grid`](crate::render::TierConfig::cellular_grid). GPU
    /// resources are deferred to the first render (module docs).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        radius_cap: u32,
        grid_cap: u32,
    ) -> Self {
        let config = CellularConfig::default();
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            config,
            needs_seed: true,
            clock: GenerationClock::default(),
            dt: 0.0,
            pending_generations: 0,
            stamp_rng: stamp_rng(config.salt),
            pending_stamp: None,
            prev_reseed: 0.0,
            radius_cap: (radius_cap as f32).clamp(1.0, MAX_RADIUS),
            grid_cap: grid_cap.clamp(MIN_GRID, MAX_GRID),
            clamp: None,
            birth: DEFAULT_BIRTH,
            survive: DEFAULT_SURVIVE,
            radius: DEFAULT_RADIUS,
            birth_lo: DEFAULT_BIRTH_LO,
            birth_hi: DEFAULT_BIRTH_HI,
            survive_lo: DEFAULT_SURVIVE_LO,
            survive_hi: DEFAULT_SURVIVE_HI,
            states: DEFAULT_STATES,
            threshold: DEFAULT_THRESHOLD,
            step_rate: DEFAULT_STEP_RATE,
            reseed: 0.0,
            trail: DEFAULT_TRAIL,
            age_tint: DEFAULT_AGE_TINT,
            colour: common::PaletteParams::new(DEFAULT_HUE, common::DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            zoom: DEFAULT_ZOOM,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
            palette: Palette::default_spectrum(),
        }
    }

    /// This frame's step uniform: the rule, the field's seed, and the disc
    /// `stamp` refills if one is scheduled.
    fn step_params(&self, stamp: Option<Stamp>) -> StepParams {
        let stamp = stamp.unwrap_or(Stamp {
            centre: [0, 0],
            radius_sq: 0,
            seed: 0,
        });
        let radius = applied_radius(self.radius, self.radius_cap);
        let birth = interval_counts(
            self.birth_lo,
            self.birth_hi,
            radius,
            [DEFAULT_BIRTH_LO, DEFAULT_BIRTH_HI],
        );
        let survive = interval_counts(
            self.survive_lo,
            self.survive_hi,
            radius,
            [DEFAULT_SURVIVE_LO, DEFAULT_SURVIVE_HI],
        );
        StepParams {
            a: [
                self.config.family.index(),
                u32::from(self.config.wrap),
                self.config.grid,
                live_threshold(self.config.family.density()),
            ],
            b: [
                applied_rule(self.birth, DEFAULT_BIRTH),
                applied_rule(self.survive, DEFAULT_SURVIVE),
                radius,
                applied_states(self.states),
            ],
            c: [field_seed(self.config.salt), stamp.seed, stamp.radius_sq, 0],
            d: [
                stamp.centre[0],
                stamp.centre[1],
                applied_threshold(self.threshold),
                0,
            ],
            e: [birth[0], birth[1], survive[0], survive[1]],
        }
    }
}

impl Scene for CellularScene {
    fn name(&self) -> &'static str {
        "cellular"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        if let Some(res) = self.res.as_mut() {
            res.luts.set(palette);
        }
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<super::CapOverflow> {
        let GeneratorConfig::Cellular(config) = cfg else {
            return None;
        };
        // A switch starts the incoming preset from its own seed: its family
        // may read the state channel differently, and its discs are its own
        // stream. The grid is held to the tier here, once, and the clamp is
        // returned for the renderer to announce with the preset.
        let (config, overflow) = grid_clamp(*config, self.grid_cap);
        self.config = config;
        self.needs_seed = true;
        self.clock = GenerationClock::default();
        self.pending_generations = 0;
        self.stamp_rng = stamp_rng(config.salt);
        self.pending_stamp = None;
        self.prev_reseed = 0.0;
        // The outgoing preset's radius clamp is not this one's to report.
        self.clamp = None;
        overflow
    }

    fn mirror_overflow(&self) -> Option<&super::CapOverflow> {
        self.clamp.as_ref()
    }

    fn reset_params(&mut self) {
        self.birth = DEFAULT_BIRTH;
        self.survive = DEFAULT_SURVIVE;
        self.radius = DEFAULT_RADIUS;
        self.birth_lo = DEFAULT_BIRTH_LO;
        self.birth_hi = DEFAULT_BIRTH_HI;
        self.survive_lo = DEFAULT_SURVIVE_LO;
        self.survive_hi = DEFAULT_SURVIVE_HI;
        self.states = DEFAULT_STATES;
        self.threshold = DEFAULT_THRESHOLD;
        self.step_rate = DEFAULT_STEP_RATE;
        self.reseed = 0.0;
        self.trail = DEFAULT_TRAIL;
        self.age_tint = DEFAULT_AGE_TINT;
        self.colour.reset();
        self.pan.reset();
        self.zoom = DEFAULT_ZOOM;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            "birth" => self.birth = value,
            "survive" => self.survive = value,
            "radius" => self.radius = value,
            "birth_lo" => self.birth_lo = value,
            "birth_hi" => self.birth_hi = value,
            "survive_lo" => self.survive_lo = value,
            "survive_hi" => self.survive_hi = value,
            "states" => self.states = value,
            "threshold" => self.threshold = value,
            "step_rate" => self.step_rate = value,
            "reseed" => self.reseed = value,
            "trail" => self.trail = value,
            "age_tint" => self.age_tint = value,
            "zoom" => self.zoom = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        self.pending_generations = self.clock.advance(self.step_rate, self.dt);
        // Rising edge only, so a beat flag held for several frames — or a
        // latch's hold — refills one disc rather than one per frame. A NaN
        // compares false both ways and is stored as zero, so it neither fires
        // nor arms a spurious edge on the next finite value.
        let reseed = if self.reseed.is_finite() {
            self.reseed
        } else {
            0.0
        };
        if reseed >= RESEED_THRESHOLD && self.prev_reseed < RESEED_THRESHOLD {
            self.pending_stamp = Some(next_stamp(&mut self.stamp_rng, self.config.grid));
        }
        self.prev_reseed = reseed;
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        _aspect: f32,
    ) {
        let grid = self.config.grid;
        if self.res.as_ref().is_none_or(|res| res.grid != grid) {
            let mut built = Resources::build(&self.device, self.surface_format, grid);
            built.luts.set(&self.palette);
            self.res = Some(built);
            self.needs_seed = true;
        }

        let seed = std::mem::replace(&mut self.needs_seed, false);
        let stamp = self.pending_stamp.take();
        let generations = std::mem::take(&mut self.pending_generations);
        let step = self.step_params(stamp);
        self.clamp = radius_clamp(self.config.family, self.radius, self.radius_cap);
        let present = PresentParams {
            a: [
                self.colour.hue,
                self.colour.brightness,
                self.colour.saturation,
                self.colour.mix,
            ],
            b: [
                palette::band_steps(self.colour.steps),
                palette::band_contour(self.colour.contour),
                self.occlude,
                grid as f32,
            ],
            c: [
                if self.zoom.is_finite() && self.zoom > 1e-3 {
                    self.zoom
                } else {
                    DEFAULT_ZOOM
                },
                if self.pan.x.is_finite() {
                    self.pan.x
                } else {
                    0.0
                },
                if self.pan.y.is_finite() {
                    self.pan.y
                } else {
                    0.0
                },
                if self.config.wrap { 1.0 } else { 0.0 },
            ],
            d: [
                applied_trail(self.trail),
                if self.age_tint.is_finite() {
                    self.age_tint
                } else {
                    DEFAULT_AGE_TINT
                },
                if self.config.family == CellularFamily::Cyclic {
                    1.0
                } else {
                    0.0
                },
                applied_states(self.states) as f32,
            ],
        };

        let Some(res) = self.res.as_mut() else {
            return;
        };
        res.luts.flush(queue);
        queue.write_buffer(&res.present_uniform, 0, bytemuck::bytes_of(&present));

        // One write serves every pass below: the seed, then the disc, then the
        // generations, in that order, so a disc scheduled on a preset's first
        // frame lands on its seeded field rather than under it.
        if seed || stamp.is_some() || generations > 0 {
            queue.write_buffer(&res.step_uniform, 0, bytemuck::bytes_of(&step));
        }
        if seed {
            res.encode_step(encoder, StepPass::Seed);
        }
        if stamp.is_some() {
            res.encode_step(encoder, StepPass::Stamp);
        }
        let sums_rows = self.config.family.sums_rows();
        for _ in 0..generations {
            if sums_rows {
                res.encode_rows(encoder);
            }
            res.encode_step(encoder, StepPass::Step);
        }

        // Load over the engine backdrop (ADR-0018): dead cells write no
        // coverage, so the backdrop survives wherever nothing lives.
        let mut pass = gpu::color_pass(encoder, "cellular-present-pass", view, wgpu::LoadOp::Load);
        pass.set_pipeline(&res.present_pipeline);
        pass.set_bind_group(0, res.present_bg.for_field(&res.field), &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod mirror;
#[cfg(test)]
mod tests;

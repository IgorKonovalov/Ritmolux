//! The voxel system's rules: a 3D automaton in the Generations sense, and the
//! closed roster of named ones a preset lists in `[voxel] rules` (ADR-0268).
//!
//! A rule is a pair of masks over the live-neighbour count — bit `k` of `birth`
//! set means a dead cell with `k` live neighbours is born, bit `k` of `survive`
//! that a live one with `k` stays live — a count of states, and a
//! neighbourhood. A cell is dead (0), live (1), or in one of the decay stages
//! `2..states`; a live cell that does not survive enters the first decay stage,
//! a decaying cell advances one stage a generation and falls back to dead past
//! the last, and only a live cell counts as a neighbour. With `states = 2` there
//! are no decay stages, and the rule is a plain 3D life-like one.
//!
//! **The masks are integers end to end.** A Moore count reaches 26, so a mask
//! is 27 bits, past what an `f32` holds exactly; that is why a rule is chosen
//! from a list by a held index rather than bound as a mask (ADR-0268).

// Hot-path panic-denial pragma (the hygiene guard scans `render/`). The rule
// table is read every frame a generation runs.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// Which cells count as a cell's neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Neighbourhood {
    /// The 26 cells sharing a face, an edge or a corner.
    #[default]
    Moore,
    /// The 6 cells sharing a face.
    VonNeumann,
}

impl Neighbourhood {
    /// The largest live-neighbour count this neighbourhood reaches.
    pub fn max_count(self) -> u32 {
        match self {
            Neighbourhood::Moore => 26,
            Neighbourhood::VonNeumann => 6,
        }
    }

    /// The integer the step shader's neighbourhood `select` reads.
    pub(crate) fn index(self) -> u32 {
        match self {
            Neighbourhood::Moore => 0,
            Neighbourhood::VonNeumann => 1,
        }
    }
}

/// The most states a rule may hold: dead, live and 253 decay stages, so a
/// state fits the 8 bits the state texel gives it.
pub const MAX_STATES: u32 = 255;

/// One compiled rule, as the step shader reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// Bit `k`: a dead cell with `k` live neighbours is born.
    pub birth: u32,
    /// Bit `k`: a live cell with `k` live neighbours stays live.
    pub survive: u32,
    /// Dead, live and the decay stages: `2..=`[`MAX_STATES`].
    pub states: u32,
    /// Which cells are counted.
    pub neighbourhood: Neighbourhood,
}

/// A mask with bit `k` set for every `k` in each inclusive `lo..=hi` span.
#[allow(
    clippy::indexing_slicing,
    reason = "a const fn cannot call `slice::get`; the index is bounded by the loop condition"
)]
const fn mask(spans: &[(u32, u32)]) -> u32 {
    let mut out = 0u32;
    let mut i = 0;
    while i < spans.len() {
        let (lo, hi) = spans[i];
        let mut k = lo;
        while k <= hi {
            out |= 1 << k;
            k += 1;
        }
        i += 1;
    }
    out
}

/// The closed roster of named rules. Each entry's notation in the comment is
/// survive/birth/states/neighbourhood, as the 3D automaton catalogues write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterRule {
    /// `4/4/5/M`: a sparse crystal that grows from a small dense core.
    R445,
    /// `13-26/13-14,17-19/2/M`: a dense random field that settles into
    /// rounded clouds.
    Clouds,
}

impl RosterRule {
    /// Every roster rule, in the order the generated reference lists them.
    pub const ALL: [RosterRule; 2] = [RosterRule::R445, RosterRule::Clouds];

    /// Parse a roster name, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.as_str() == name)
    }

    /// The name a preset writes — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            RosterRule::R445 => "445",
            RosterRule::Clouds => "clouds",
        }
    }

    /// The seed this rule lives from, as `(seed_radius, seed_fill)`: what an
    /// absent `[voxel] seed_radius` and `seed_fill` mean when the list opens
    /// with this rule. A 3D rule is particular about its seed — `clouds`
    /// survives only a dense field, and dies from a sparse one.
    pub fn seed(self) -> (f32, f32) {
        match self {
            RosterRule::R445 => (0.35, 0.2),
            RosterRule::Clouds => (1.0, 0.6),
        }
    }

    /// The compiled rule.
    pub fn rule(self) -> Rule {
        match self {
            RosterRule::R445 => Rule {
                birth: mask(&[(4, 4)]),
                survive: mask(&[(4, 4)]),
                states: 5,
                neighbourhood: Neighbourhood::Moore,
            },
            RosterRule::Clouds => Rule {
                birth: mask(&[(13, 14), (17, 19)]),
                survive: mask(&[(13, 26)]),
                states: 2,
                neighbourhood: Neighbourhood::Moore,
            },
        }
    }
}

/// The most rules one `[voxel] rules` list may hold.
pub const MAX_RULES: usize = 8;

/// A preset's rule list, compiled: one to [`MAX_RULES`] rules in a fixed array,
/// so the config stays `Copy` and nothing per frame allocates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleList {
    rules: [Rule; MAX_RULES],
    len: usize,
}

/// The rule an absent `[voxel] rules` key means.
pub const DEFAULT_RULE: RosterRule = RosterRule::Clouds;

impl Default for RuleList {
    /// [`DEFAULT_RULE`] alone — what an absent `rules` key means.
    fn default() -> Self {
        Self::one(DEFAULT_RULE.rule())
    }
}

impl RuleList {
    /// A list of `rule` alone.
    pub fn one(rule: Rule) -> Self {
        Self {
            rules: [rule; MAX_RULES],
            len: 1,
        }
    }

    /// `rules` as a list, or `None` when it is empty or longer than
    /// [`MAX_RULES`].
    pub fn new(rules: &[Rule]) -> Option<Self> {
        let first = *rules.first()?;
        if rules.len() > MAX_RULES {
            return None;
        }
        let mut out = Self::one(first);
        for (slot, rule) in out.rules.iter_mut().zip(rules) {
            *slot = *rule;
        }
        out.len = rules.len();
        Some(out)
    }

    /// How many rules the list holds, at least one.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Never true: a list holds at least one rule. Present for the `len`
    /// convention.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The list's first rule.
    pub fn first(&self) -> Rule {
        let [first, ..] = self.rules;
        first
    }
}

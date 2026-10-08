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

impl Rule {
    /// An inline rule from its birth and survive counts, or the reason it is not
    /// one: a count past the neighbourhood's [`max_count`](Neighbourhood::max_count)
    /// (or below zero), or `states` outside `2..=`[`MAX_STATES`].
    pub fn inline(
        birth: &[i64],
        survive: &[i64],
        states: i64,
        neighbourhood: Neighbourhood,
    ) -> Result<Self, String> {
        let max = i64::from(neighbourhood.max_count());
        let masked = |counts: &[i64], what: &str| -> Result<u32, String> {
            counts.iter().try_fold(0u32, |m, &k| {
                if (0..=max).contains(&k) {
                    Ok(m | (1 << k))
                } else {
                    Err(format!(
                        "{what} count {k} is outside 0..={max} for this neighbourhood"
                    ))
                }
            })
        };
        if !(2..=i64::from(MAX_STATES)).contains(&states) {
            return Err(format!("states {states} is outside 2..={MAX_STATES}"));
        }
        Ok(Self {
            birth: masked(birth, "birth")?,
            survive: masked(survive, "survive")?,
            states: states as u32,
            neighbourhood,
        })
    }
}

impl Neighbourhood {
    /// Every neighbourhood, in the order the reference lists them.
    pub const ALL: [Neighbourhood; 2] = [Neighbourhood::Moore, Neighbourhood::VonNeumann];

    /// Parse an inline rule's `neighbourhood`, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.as_str() == name)
    }

    /// The name an inline rule writes.
    pub fn as_str(self) -> &'static str {
        match self {
            Neighbourhood::Moore => "moore",
            Neighbourhood::VonNeumann => "von_neumann",
        }
    }
}

/// The closed roster of named rules. Each entry's notation in the comment is
/// survive/birth/states/neighbourhood, as the 3D automaton catalogues write it;
/// every one is held by `every_roster_rule_lives_from_its_own_seed` to a live
/// fraction strictly inside `(0, 0.9)` at generations 100 and 400.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterRule {
    /// `4/4/5/M`: a sparse ball that burns out to a few still cells.
    R445,
    /// `9-26/5-7,12-13,15/5/M`: a sparse field that swells into a mass filling
    /// most of the cube.
    Amoeba,
    /// `2,6,9/4,6,8-9/10/M`: a burst of long-decaying growth that leaves a
    /// still scatter of cells behind.
    Builder,
    /// `13-26/13-14,17-19/2/M`: a dense field that settles into rounded
    /// clouds.
    Clouds,
    /// `5-8/6-7,9,12/4/M`: a slowly churning reef with a skin of decaying
    /// cells.
    Coral,
    /// `0-6/1,3/2/VN`: a small seed growing into a lattice that fills most of
    /// the cube.
    Crystal,
    /// `4-7/6-8/10/M`: a restless, never-settling foam of cells and long
    /// decay.
    Pyroclastic,
    /// `1,4,8,11,13-26/13-26/5/M`: a dense field that freezes almost at once.
    SlowDecay,
}

impl RosterRule {
    /// Every roster rule, in the order the generated reference lists them.
    pub const ALL: [RosterRule; 8] = [
        RosterRule::R445,
        RosterRule::Amoeba,
        RosterRule::Builder,
        RosterRule::Clouds,
        RosterRule::Coral,
        RosterRule::Crystal,
        RosterRule::Pyroclastic,
        RosterRule::SlowDecay,
    ];

    /// Parse a roster name, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.as_str() == name)
    }

    /// The name a preset writes — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            RosterRule::R445 => "445",
            RosterRule::Amoeba => "amoeba",
            RosterRule::Builder => "builder",
            RosterRule::Clouds => "clouds",
            RosterRule::Coral => "coral",
            RosterRule::Crystal => "crystal",
            RosterRule::Pyroclastic => "pyroclastic",
            RosterRule::SlowDecay => "slow_decay",
        }
    }

    /// The seed this rule lives from, as `(seed_radius, seed_fill)`: what an
    /// absent `[voxel] seed_radius` and `seed_fill` mean when the list opens
    /// with this rule. A 3D rule is particular about its seed — `clouds`
    /// survives only a dense field and dies from a sparse one, where `crystal`
    /// grows from a small sparse ball.
    pub fn seed(self) -> (f32, f32) {
        match self {
            RosterRule::R445 => (0.35, 0.2),
            RosterRule::Amoeba => (2.0, 0.5),
            RosterRule::Builder => (0.5, 0.5),
            RosterRule::Clouds => (1.0, 0.6),
            RosterRule::Coral => (0.5, 0.5),
            RosterRule::Crystal => (0.2, 0.3),
            RosterRule::Pyroclastic => (0.5, 0.3),
            RosterRule::SlowDecay => (1.0, 0.5),
        }
    }

    /// The compiled rule.
    pub fn rule(self) -> Rule {
        let moore = |birth: &[(u32, u32)], survive: &[(u32, u32)], states: u32| Rule {
            birth: mask(birth),
            survive: mask(survive),
            states,
            neighbourhood: Neighbourhood::Moore,
        };
        match self {
            RosterRule::R445 => moore(&[(4, 4)], &[(4, 4)], 5),
            RosterRule::Amoeba => moore(&[(5, 7), (12, 13), (15, 15)], &[(9, 26)], 5),
            RosterRule::Builder => moore(&[(4, 4), (6, 6), (8, 9)], &[(2, 2), (6, 6), (9, 9)], 10),
            RosterRule::Clouds => moore(&[(13, 14), (17, 19)], &[(13, 26)], 2),
            RosterRule::Coral => moore(&[(6, 7), (9, 9), (12, 12)], &[(5, 8)], 4),
            RosterRule::Crystal => Rule {
                birth: mask(&[(1, 1), (3, 3)]),
                survive: mask(&[(0, 6)]),
                states: 2,
                neighbourhood: Neighbourhood::VonNeumann,
            },
            RosterRule::Pyroclastic => moore(&[(6, 8)], &[(4, 7)], 10),
            RosterRule::SlowDecay => moore(
                &[(13, 26)],
                &[(1, 1), (4, 4), (8, 8), (11, 11), (13, 26)],
                5,
            ),
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

    /// The rule a bound `rule` index of `index` picks: rounded, held inside the
    /// list, and the first rule where it is not finite. The engine has already
    /// rounded a structural value, so the round here only matters to a caller
    /// that has not.
    pub fn pick(&self, index: f32) -> Rule {
        let last = self.len.saturating_sub(1) as f32;
        let i = if index.is_finite() {
            index.round().clamp(0.0, last) as usize
        } else {
            0
        };
        self.rules.get(i).copied().unwrap_or_else(|| self.first())
    }
}

//! The raw `[voxel]` table: the voxel system's grid, its rule list, the ball
//! it is seeded with, whether its faces wrap, and how many shells the spectrum
//! lights.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;
use crate::render::scenes::voxel::{
    DEFAULT_GRID, DEFAULT_RULE, MAX_GRID, MAX_SHELLS, MIN_GRID, Neighbourhood, RosterRule, Rule,
    RuleList, VoxelConfig, rules::MAX_RULES,
};

/// The raw `[voxel]` table (ADR-0268). An absent table is [`DEFAULT_RULE`] on
/// the default grid, seeded from the preset's own salt.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawVoxel {
    /// Cells per side of the cube. Absent means [`DEFAULT_GRID`].
    #[serde(default)]
    pub(in crate::preset::schema) grid: Option<u32>,
    /// The rules the structural `rule` parameter picks among: roster names and
    /// inline rules, in any mix. Absent means [`DEFAULT_RULE`] alone.
    #[serde(default)]
    pub(in crate::preset::schema) rules: Option<Vec<RawRule>>,
    /// The seeded ball's radius, as a fraction of the cube's half-side. Absent
    /// means the first rule's own ([`RosterRule::seed`]).
    #[serde(default)]
    pub(in crate::preset::schema) seed_radius: Option<f32>,
    /// The probability a cell inside the seeded ball is live. Absent means the
    /// first rule's own.
    #[serde(default)]
    pub(in crate::preset::schema) seed_fill: Option<f32>,
    /// `true`: the cube is a 3-torus. `false`: every cell past a face is dead.
    /// Absent means `false`.
    #[serde(default)]
    pub(in crate::preset::schema) wrap: Option<bool>,
    /// How many radial shells the spectrum lights. Absent means `0`: none.
    #[serde(default)]
    pub(in crate::preset::schema) shells: Option<u32>,
}

/// One `[voxel] rules` entry: a roster name, or an inline rule.
#[derive(Deserialize)]
#[serde(untagged)]
pub(in crate::preset::schema) enum RawRule {
    /// A name from [`RosterRule::ALL`].
    Named(String),
    /// `{ birth = [..], survive = [..], states = N, neighbourhood = ".." }`.
    Inline(RawInlineRule),
}

/// An inline rule, before validation. The counts are signed so a negative one
/// reaches the range check, which names the entry, rather than failing as a
/// type error that cannot.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawInlineRule {
    /// The live-neighbour counts at which a dead cell is born.
    pub(in crate::preset::schema) birth: Vec<i64>,
    /// The live-neighbour counts at which a live cell stays live.
    pub(in crate::preset::schema) survive: Vec<i64>,
    /// Dead, live and the decay stages. Absent means `2`: no decay.
    #[serde(default)]
    pub(in crate::preset::schema) states: Option<i64>,
    /// `"moore"` or `"von_neumann"`. Absent means `"moore"`.
    #[serde(default)]
    pub(in crate::preset::schema) neighbourhood: Option<String>,
}

impl RawRule {
    /// The compiled rule, and the roster entry it names if it names one; or the
    /// load error, naming entry `i`.
    fn compile(self, i: usize) -> Result<(Rule, Option<RosterRule>), PresetError> {
        let entry =
            |reason: String| PresetError::Config(format!("[voxel] rules entry {i}: {reason}"));
        match self {
            RawRule::Named(name) => {
                let rule = RosterRule::from_name(&name).ok_or_else(|| {
                    entry(format!(
                        "unknown rule '{name}' (expected one of: {})",
                        RosterRule::ALL
                            .iter()
                            .map(|r| r.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })?;
                Ok((rule.rule(), Some(rule)))
            }
            RawRule::Inline(raw) => {
                let neighbourhood = match raw.neighbourhood {
                    None => Neighbourhood::default(),
                    Some(name) => Neighbourhood::from_name(&name).ok_or_else(|| {
                        entry(format!(
                            "unknown neighbourhood '{name}' (expected one of: {})",
                            Neighbourhood::ALL
                                .iter()
                                .map(|n| n.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?,
                };
                let rule = Rule::inline(
                    &raw.birth,
                    &raw.survive,
                    raw.states.unwrap_or(2),
                    neighbourhood,
                )
                .map_err(entry)?;
                Ok((rule, None))
            }
        }
    }
}

impl RawVoxel {
    /// Validate the table into a [`VoxelConfig`], erroring (never panicking) on
    /// a grid outside [`MIN_GRID`]`..=`[`MAX_GRID`], an empty or over-long rule
    /// list, an unknown roster name — which would run an automaton the author
    /// never asked for — an inline rule out of range, a seed outside its range,
    /// and more shells than [`MAX_SHELLS`]. A rule's error names its entry.
    ///
    /// `salt` is the preset's pinned seed (ADR-0051): every cell the volume is
    /// seeded with is drawn from it.
    pub(in crate::preset::schema) fn into_config(
        self,
        salt: u32,
    ) -> Result<VoxelConfig, PresetError> {
        let grid = self.grid.unwrap_or(DEFAULT_GRID);
        if !(MIN_GRID..=MAX_GRID).contains(&grid) {
            return Err(PresetError::Config(format!(
                "[voxel] grid must be in {MIN_GRID}..={MAX_GRID} cells per side, got {grid}"
            )));
        }
        // An inline rule at the head of the list has no seed of its own, so it
        // takes the default rule's.
        let mut first = Some(DEFAULT_RULE);
        let rules = match self.rules {
            None => RuleList::default(),
            Some(entries) => {
                if entries.is_empty() || entries.len() > MAX_RULES {
                    return Err(PresetError::Config(format!(
                        "[voxel] rules must list 1..={MAX_RULES} rules, got {}",
                        entries.len()
                    )));
                }
                let mut compiled = Vec::with_capacity(entries.len());
                for (i, raw) in entries.into_iter().enumerate() {
                    let (rule, named) = raw.compile(i)?;
                    if i == 0 {
                        first = named;
                    }
                    compiled.push(rule);
                }
                RuleList::new(&compiled)
                    .ok_or_else(|| PresetError::Config("[voxel] rules must not be empty".into()))?
            }
        };
        let (rule_radius, rule_fill) = first.unwrap_or(DEFAULT_RULE).seed();
        let seed_radius = self.seed_radius.unwrap_or(rule_radius);
        if !(seed_radius.is_finite() && (0.0..=2.0).contains(&seed_radius)) {
            return Err(PresetError::Config(format!(
                "[voxel] seed_radius must be in 0..=2 half-sides, got {seed_radius}"
            )));
        }
        let seed_fill = self.seed_fill.unwrap_or(rule_fill);
        if !(seed_fill.is_finite() && (0.0..=1.0).contains(&seed_fill)) {
            return Err(PresetError::Config(format!(
                "[voxel] seed_fill must be in 0..=1, got {seed_fill}"
            )));
        }
        let shells = self.shells.unwrap_or(0);
        if shells > MAX_SHELLS {
            return Err(PresetError::Config(format!(
                "[voxel] shells must be in 0..={MAX_SHELLS}, got {shells}"
            )));
        }
        Ok(VoxelConfig {
            grid,
            rules,
            seed_radius,
            seed_fill,
            wrap: self.wrap.unwrap_or(false),
            shells,
            salt,
        })
    }
}

// ---------------------------------------------------------------------------
// The schema descriptor: the second statement of this table's shape that an
// editor reads, held to the serde one by `schema::tests`.
// ---------------------------------------------------------------------------

/// The `[voxel]` table.
pub(in crate::preset::schema) const VOXEL: TableDesc = TableDesc {
    name: "voxel",
    doc: "The voxel automaton's grid, the rules it runs, the ball it is seeded with, whether \
          its faces wrap, and how many shells the spectrum lights.",
    keys: &[
        KeyDesc {
            name: "grid",
            kind: KeyKind::Int,
            default: "64",
            doc: "Cells per side of the cube. A content value, not a resolution: doubling it \
                  halves how large every structure looks. The quality tier may run fewer, and \
                  says so.",
        },
        KeyDesc {
            name: "rules",
            kind: KeyKind::List(&KeyKind::RosterOrTable(Roster::VoxelRule, "voxel_rule")),
            default: "[\"clouds\"]",
            doc: "The rules the structural rule parameter picks among, by index: roster names, \
                  or inline rules.",
        },
        KeyDesc {
            name: "seed_radius",
            kind: KeyKind::Float,
            default: "",
            doc: "The radius of the ball of cells the volume is seeded with, as a fraction of \
                  the cube's half-side; absent, the first rule's own.",
        },
        KeyDesc {
            name: "seed_fill",
            kind: KeyKind::Float,
            default: "",
            doc: "The chance each cell inside the seeded ball starts live; absent, the first \
                  rule's own.",
        },
        KeyDesc {
            name: "wrap",
            kind: KeyKind::Bool,
            default: "false",
            doc: "Whether the cube is a torus; false makes every cell past a face dead.",
        },
        KeyDesc {
            name: "shells",
            kind: KeyKind::Int,
            default: "0",
            doc: "How many radial shells the spectrum lights, bass at the centre; 0 lights none.",
        },
    ],
};

/// One inline `[voxel] rules` entry.
pub(in crate::preset::schema) const VOXEL_RULE: TableDesc = TableDesc {
    name: "voxel_rule",
    doc: "An inline 3D rule: which live-neighbour counts give birth and survival, how many \
          states a cell passes through, and which neighbours count.",
    keys: &[
        KeyDesc {
            name: "birth",
            kind: KeyKind::List(&KeyKind::Int),
            default: "",
            doc: "The live-neighbour counts at which a dead cell is born, 0-26 (0-6 for \
                  von_neumann). Required.",
        },
        KeyDesc {
            name: "survive",
            kind: KeyKind::List(&KeyKind::Int),
            default: "",
            doc: "The live-neighbour counts at which a live cell stays live. Required.",
        },
        KeyDesc {
            name: "states",
            kind: KeyKind::Int,
            default: "2",
            doc: "Dead, live and the decay stages a dying cell passes through; 2 has none.",
        },
        KeyDesc {
            name: "neighbourhood",
            kind: KeyKind::Roster(Roster::VoxelNeighbourhood),
            default: "moore",
            doc: "Which cells count: the 26 around a cell, or the 6 sharing its faces.",
        },
    ],
};

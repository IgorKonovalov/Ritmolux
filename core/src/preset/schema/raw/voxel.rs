//! The raw `[voxel]` table: the voxel system's grid, its rule list, the ball
//! it is seeded with, and whether its faces wrap.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;
use crate::render::scenes::voxel::{
    DEFAULT_GRID, DEFAULT_RULE, MAX_GRID, MIN_GRID, RosterRule, RuleList, VoxelConfig,
    rules::MAX_RULES,
};

/// The raw `[voxel]` table (ADR-0268). An absent table is [`DEFAULT_RULE`] on
/// the default grid, seeded from the preset's own salt.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawVoxel {
    /// Cells per side of the cube. Absent means [`DEFAULT_GRID`].
    #[serde(default)]
    pub(in crate::preset::schema) grid: Option<u32>,
    /// The rules the structural `rule` parameter picks among, as roster names.
    /// Absent means [`DEFAULT_RULE`] alone.
    #[serde(default)]
    pub(in crate::preset::schema) rules: Option<Vec<String>>,
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
}

impl RawVoxel {
    /// Validate the table into a [`VoxelConfig`], erroring (never panicking) on
    /// a grid outside [`MIN_GRID`]`..=`[`MAX_GRID`], an empty or over-long rule
    /// list, an unknown roster name — which would run an automaton the author
    /// never asked for — and a seed outside its range.
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
        let mut first = DEFAULT_RULE;
        let rules = match self.rules {
            None => RuleList::default(),
            Some(names) => {
                if names.is_empty() || names.len() > MAX_RULES {
                    return Err(PresetError::Config(format!(
                        "[voxel] rules must list 1..={MAX_RULES} rules, got {}",
                        names.len()
                    )));
                }
                let mut compiled = Vec::with_capacity(names.len());
                for (i, name) in names.iter().enumerate() {
                    let rule = RosterRule::from_name(name).ok_or_else(|| {
                        PresetError::Config(format!(
                            "[voxel] rules entry {i}: unknown rule '{name}' (expected one of: {})",
                            RosterRule::ALL
                                .iter()
                                .map(|r| r.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?;
                    if i == 0 {
                        first = rule;
                    }
                    compiled.push(rule.rule());
                }
                RuleList::new(&compiled)
                    .ok_or_else(|| PresetError::Config("[voxel] rules must not be empty".into()))?
            }
        };
        let (rule_radius, rule_fill) = first.seed();
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
        Ok(VoxelConfig {
            grid,
            rules,
            seed_radius,
            seed_fill,
            wrap: self.wrap.unwrap_or(false),
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
    doc: "The voxel automaton's grid, the rules it runs, the ball it is seeded with, and \
          whether its faces wrap.",
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
            kind: KeyKind::List(&KeyKind::Roster(Roster::VoxelRule)),
            default: "[\"clouds\"]",
            doc: "The rules the structural rule parameter picks among, by index.",
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
    ],
};

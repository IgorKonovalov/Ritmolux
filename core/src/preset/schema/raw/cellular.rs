//! The raw `[cellular]` table: which automaton the cellular system runs, on how
//! large a grid, and whether its edges wrap.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;

/// The raw `[cellular]` table (ADR-0180 rule 1). An absent table is the default
/// family on the default grid, the way an absent `[field]` table is Chladni.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawCellular {
    /// Family name (`"life_like"`); validated at load. Absent means the
    /// default family.
    #[serde(default)]
    pub(in crate::preset::schema) family: Option<String>,
    /// Cells per side of the square grid. Absent means
    /// [`DEFAULT_GRID`](crate::render::scenes::cellular::DEFAULT_GRID).
    #[serde(default)]
    pub(in crate::preset::schema) grid: Option<u32>,
    /// `true`: the grid is a torus. `false`: every cell past the border is dead.
    /// Absent means `true`.
    #[serde(default)]
    pub(in crate::preset::schema) wrap: Option<bool>,
}

impl RawCellular {
    /// Validate the table into a [`CellularConfig`], erroring (never panicking)
    /// on an unknown family — which selects a rule, so a silent default would run
    /// an automaton the author never asked for — and on a grid outside
    /// [`MIN_GRID`](crate::render::scenes::cellular::MIN_GRID)`..=`[`MAX_GRID`](crate::render::scenes::cellular::MAX_GRID).
    ///
    /// `salt` is the preset's pinned seed (ADR-0051): every cell the automaton
    /// is seeded with, and every region a `reseed` refills, is drawn from it.
    pub(in crate::preset::schema) fn into_config(
        self,
        salt: u32,
    ) -> Result<CellularConfig, PresetError> {
        use crate::render::scenes::cellular::{DEFAULT_GRID, MAX_GRID, MIN_GRID};
        let family = match self.family {
            None => CellularFamily::default(),
            Some(name) => CellularFamily::from_name(&name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [cellular] family '{name}' (expected one of: {})",
                    CellularFamily::ALL
                        .iter()
                        .map(|f| f.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
        };
        let grid = self.grid.unwrap_or(DEFAULT_GRID);
        if !(MIN_GRID..=MAX_GRID).contains(&grid) {
            return Err(PresetError::Config(format!(
                "[cellular] grid must be in {MIN_GRID}..={MAX_GRID} cells per side, got {grid}"
            )));
        }
        Ok(CellularConfig {
            family,
            grid,
            wrap: self.wrap.unwrap_or(true),
            salt,
        })
    }
}

// ---------------------------------------------------------------------------
// The schema descriptor (Plan 0158 Phase 4). A second statement of this table's
// shape, and one on purpose: serde carries a field's name and type and none of
// what an editor needs -- the closed roster a string is drawn from, the default
// the loader substitutes, the sentence saying what the key does. What holds the
// two together is `schema::tests`, which reads the field roster serde derived
// and asserts it is exactly what the rows below name.
// ---------------------------------------------------------------------------

/// The `[cellular]` table.
pub(in crate::preset::schema) const CELLULAR: TableDesc = TableDesc {
    name: "cellular",
    doc: "Which automaton the cellular system runs, on how large a grid, and whether its \
          edges wrap.",
    keys: &[
        KeyDesc {
            name: "family",
            kind: KeyKind::Roster(Roster::CellularFamily),
            default: "life_like",
            doc: "The automaton family.",
        },
        KeyDesc {
            name: "grid",
            kind: KeyKind::Int,
            default: "256",
            doc: "Cells per side. A content value, not a resolution: doubling it halves how \
                  large every pattern looks.",
        },
        KeyDesc {
            name: "wrap",
            kind: KeyKind::Bool,
            default: "true",
            doc: "Whether the grid is a torus; false makes every cell past the border dead.",
        },
    ],
};

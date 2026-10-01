//! The raw `[plexus]` table: which arrangement the plexus system's points take,
//! how many there are, and what they are seeded from.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;

/// The raw `[plexus]` table (ADR-0180 rule 1, ADR-0257). An absent table is the
/// default layout at the default point count, seeded from the preset's own salt.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawPlexus {
    /// Layout name (`"cloud"`); validated at load. Absent means the default.
    #[serde(default)]
    pub(in crate::preset::schema) layout: Option<String>,
    /// How many points. Absent means
    /// [`DEFAULT_POINTS`](crate::render::scenes::plexus::DEFAULT_POINTS).
    #[serde(default)]
    pub(in crate::preset::schema) points: Option<u32>,
    /// What the points and their flow are drawn from. Absent means the preset's
    /// pinned salt (ADR-0051), so a preset declaring only `[generator] seed`
    /// still gets a network of its own.
    #[serde(default)]
    pub(in crate::preset::schema) seed: Option<u64>,
}

impl RawPlexus {
    /// Validate the table into a [`PlexusConfig`], erroring (never panicking)
    /// on an unknown layout — which decides where every point is, so a silent
    /// default would draw a network the author never asked for — and on a point
    /// count outside
    /// [`MIN_POINTS`](crate::render::scenes::plexus::MIN_POINTS)`..=`[`MAX_POINTS`](crate::render::scenes::plexus::MAX_POINTS).
    pub(in crate::preset::schema) fn into_config(
        self,
        salt: u32,
    ) -> Result<PlexusConfig, PresetError> {
        use crate::render::scenes::plexus::{DEFAULT_POINTS, MAX_POINTS, MIN_POINTS};
        let layout = match self.layout {
            None => PlexusLayout::default(),
            Some(name) => PlexusLayout::from_name(&name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [plexus] layout '{name}' (expected one of: {})",
                    PlexusLayout::ALL
                        .iter()
                        .map(|l| l.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
        };
        let points = self.points.unwrap_or(DEFAULT_POINTS);
        if !(MIN_POINTS..=MAX_POINTS).contains(&points) {
            return Err(PresetError::Config(format!(
                "[plexus] points must be in {MIN_POINTS}..={MAX_POINTS}, got {points}"
            )));
        }
        Ok(PlexusConfig {
            layout,
            points,
            seed: self.seed.unwrap_or(u64::from(salt)),
        })
    }
}

// ---------------------------------------------------------------------------
// The schema descriptor: the second statement of this table's shape that an
// editor reads, held to the serde one by `schema::tests`.
// ---------------------------------------------------------------------------

/// The `[plexus]` table.
pub(in crate::preset::schema) const PLEXUS: TableDesc = TableDesc {
    name: "plexus",
    doc: "Which arrangement the plexus system's points take, how many there are, and what \
          they are seeded from.",
    keys: &[
        KeyDesc {
            name: "layout",
            kind: KeyKind::Roster(Roster::PlexusLayout),
            default: "cloud",
            doc: "The arrangement of the points.",
        },
        KeyDesc {
            name: "points",
            kind: KeyKind::Int,
            default: "300",
            doc: "How many points; the quality tier may draw fewer, and says so.",
        },
        KeyDesc {
            name: "seed",
            kind: KeyKind::Int,
            default: "",
            doc: "What the points and their drift are drawn from; absent, the preset's own seed.",
        },
    ],
};

//! The raw `[mesh]` table: the warp mesh's grid, in cells.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// The raw `[mesh]` table (Plan 0100 Phase 1): the warp mesh's grid, in cells.
///
/// Both keys are optional. Absent — and an absent table — means
/// [`DEFAULT_MESH`](crate::render::scenes::warp_mesh::DEFAULT_MESH), which is a
/// grid every tier can carry.
#[derive(Debug, Default, Deserialize)]
pub(in crate::preset::schema) struct RawMesh {
    #[serde(default)]
    pub(in crate::preset::schema) x: Option<u32>,
    #[serde(default)]
    pub(in crate::preset::schema) y: Option<u32>,
}

impl RawMesh {
    /// Validate into the structural config, clamping to the `.milk` format's own
    /// maximum (`meshx <= 128`, `meshy <= 96`) and refusing a degenerate grid.
    ///
    /// The **tier** clamp is deliberately not applied here: the loader does not
    /// know which tier will render the preset, and a preset authored at the rich
    /// grid must still load on the floor. `warp_mesh::clamp_grid` applies it at
    /// both consumers — the scene and the renderer's per-vertex scratch — from
    /// the one tier they share.
    pub(in crate::preset::schema) fn into_config(
        self,
        milk: Option<Box<crate::milk::MilkBundle>>,
        salt: u32,
    ) -> Result<GeneratorConfig, PresetError> {
        use crate::render::scenes::warp_mesh::{DEFAULT_MESH, MAX_MESH, MIN_MESH};
        let axis = |name: &str, value: Option<u32>, default: u32, max: u32| {
            let v = value.unwrap_or(default);
            if !(MIN_MESH..=max).contains(&v) {
                return Err(PresetError::Config(format!(
                    "[mesh] {name} must be in {MIN_MESH}..={max} cells, got {v}"
                )));
            }
            Ok(v)
        };
        Ok(GeneratorConfig::WarpMesh {
            mesh: (
                axis("x", self.x, DEFAULT_MESH.0, MAX_MESH.0)?,
                axis("y", self.y, DEFAULT_MESH.1, MAX_MESH.1)?,
            ),
            milk,
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

/// The `[mesh]` table.
pub(in crate::preset::schema) const MESH: TableDesc = TableDesc {
    name: "mesh",
    doc: "The warp mesh's grid, in cells. Clamped to the tier at both consumers rather \
          than at load.",
    keys: &[
        KeyDesc {
            name: "x",
            kind: KeyKind::Int,
            default: "32",
            doc: "Cells across.",
        },
        KeyDesc {
            name: "y",
            kind: KeyKind::Int,
            default: "24",
            doc: "Cells down.",
        },
    ],
};

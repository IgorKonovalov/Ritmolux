//! The raw `[particles]` table: which strange-attractor family, the tuple to
//! walk towards, and the population density.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// The raw `[particles]` table: which strange-attractor family the
/// compute-particle scene iterates, and how much of the tier's particle budget
/// it draws.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawParticles {
    /// Attractor family name (e.g. `"lorenz"`); validated at load.
    pub(in crate::preset::schema) family: String,
    /// Fraction of the tier's particle budget to draw (ADR-0069). Optional —
    /// absent means the whole budget, which is byte-identical to the behaviour
    /// before the key existed.
    pub(in crate::preset::schema) density: Option<f32>,
    /// The IFS figure the bindable `morph` param travels towards (ADR-0075).
    /// Optional; absent pins the figure and makes `morph` inert.
    pub(in crate::preset::schema) morph_to: Option<String>,
    /// The near end of the tuple path `morph` walks on a map family (ADR-0093),
    /// as a roster index. Optional; defaults to entry `0` when `tuple_to` names
    /// a far end.
    pub(in crate::preset::schema) tuple_from: Option<u32>,
    /// The far end of that path. Optional — and it is the key that turns the
    /// walk on: absent, there is no path and `morph` is inert.
    pub(in crate::preset::schema) tuple_to: Option<u32>,
}

impl RawParticles {
    /// Validate `morph_to` against the family it was written next to.
    ///
    /// **Two ways to be wrong, and both are load errors** (the project's
    /// validate-at-the-boundary rule): an unknown figure name, and a `morph_to`
    /// on one of the four *map* families, which have no table to interpolate.
    /// The second is the one worth erroring on rather than ignoring — a silent
    /// no-op would leave an author binding `morph` to audio and watching nothing
    /// happen, with the preset loading cleanly.
    pub(in crate::preset::schema) fn morph_to(
        &self,
        family: AttractorFamily,
    ) -> Result<Option<IfsFigure>, PresetError> {
        let Some(name) = self.morph_to.as_deref() else {
            return Ok(None);
        };
        if !matches!(family, AttractorFamily::Ifs(_)) {
            return Err(PresetError::Config(format!(
                "[particles] morph_to is only meaningful for an IFS figure, but family is '{}'",
                self.family
            )));
        }
        let figure = IfsFigure::from_name(name).ok_or_else(|| {
            PresetError::Config(format!("unknown [particles] morph_to figure '{name}'"))
        })?;
        Ok(Some(figure))
    }

    /// Validate the tuple path against the family it was written next to
    /// (ADR-0093), into the `(from, to)` pair the scene measures across.
    ///
    /// **Four ways to be wrong, all load errors**, for `morph_to`'s reason — a
    /// silent no-op leaves an author binding `morph` to audio and watching
    /// nothing happen:
    ///
    /// - a path on an **IFS**, which travels between figures through `morph_to`
    ///   instead and has no coefficient tuple to walk;
    /// - either end **past the family's roster**;
    /// - a `tuple_from` with **no `tuple_to`**, which reads like a path and is
    ///   not one;
    /// - both ends the **same entry**, which is a path of zero length and almost
    ///   certainly a typo.
    pub(in crate::preset::schema) fn tuple_path(
        &self,
        family: AttractorFamily,
    ) -> Result<Option<(u32, u32)>, PresetError> {
        let Some(to) = self.tuple_to else {
            if self.tuple_from.is_some() {
                return Err(PresetError::Config(
                    "[particles] tuple_from names the near end of a path, but there is no \
                     tuple_to naming the far end"
                        .to_string(),
                ));
            }
            return Ok(None);
        };
        if matches!(family, AttractorFamily::Ifs(_)) {
            return Err(PresetError::Config(format!(
                "[particles] tuple_to is only meaningful for a map family, but family is \
                 '{}' — an IFS travels between figures through morph_to instead",
                self.family
            )));
        }
        let from = self.tuple_from.unwrap_or(0);
        let len = crate::render::scenes::particles::roster_len(family) as u32;
        for (label, index) in [("tuple_from", from), ("tuple_to", to)] {
            if index >= len {
                return Err(PresetError::Config(format!(
                    "[particles] {label} = {index} is past '{}'s roster, which has {len} \
                     entries (0..={})",
                    self.family,
                    len.saturating_sub(1)
                )));
            }
        }
        if from == to {
            return Err(PresetError::Config(format!(
                "[particles] tuple_from and tuple_to are both {from} — a path needs two \
                 different entries"
            )));
        }
        Ok(Some((from, to)))
    }

    /// Validate `density` into the fraction the scene will resolve against the
    /// tier budget. Erroring rather than clamping, like every other structural
    /// key: a preset asking for a count the engine will not give it should say so
    /// at load, not render something the author did not ask for.
    pub(in crate::preset::schema) fn density(&self) -> Result<f32, PresetError> {
        let d = self.density.unwrap_or(1.0);
        if !(crate::render::scenes::particles::MIN_PARTICLE_DENSITY..=1.0).contains(&d) {
            return Err(PresetError::Config(format!(
                "[particles] density must be {}..=1.0, got {d}",
                crate::render::scenes::particles::MIN_PARTICLE_DENSITY
            )));
        }
        Ok(d)
    }
}

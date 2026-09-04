//! The raw `[curve]` and `[generator]` tables: the declarative structure a line
//! scene builds its geometry from, and the ring roster beside it.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;
use super::*;

/// The raw `[curve]` table: declarative structure for a parametric-curve scene.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawCurve {
    /// Curve family name (e.g. `"maurer_rose"`).
    pub(in crate::preset::schema) family: String,
}

impl RawCurve {
    /// Validate the family name into a [`GeneratorConfig`], erroring (never
    /// panicking) on an unknown family.
    pub(in crate::preset::schema) fn into_config(self) -> Result<GeneratorConfig, PresetError> {
        let family = CurveFamily::from_name(&self.family).ok_or_else(|| {
            PresetError::Config(format!("unknown curve family '{}'", self.family))
        })?;
        Ok(GeneratorConfig::Curve { family })
    }
}

/// The raw `[generator]` table: declarative structure for a generator scene.
/// Fields are optional at the serde layer and validated per system below, so
/// one table shape can serve the L-system (and, later, the star pattern).
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawGenerator {
    /// L-system: starting string.
    #[serde(default)]
    pub(in crate::preset::schema) axiom: Option<String>,
    /// L-system: production rules, each key a single predecessor character.
    #[serde(default)]
    pub(in crate::preset::schema) rules: BTreeMap<String, String>,
    /// L-system: turn angle in degrees.
    #[serde(default)]
    pub(in crate::preset::schema) angle_deg: Option<f32>,
    /// L-system: iterations to precompute.
    #[serde(default)]
    pub(in crate::preset::schema) max_depth: Option<u32>,
    /// The preset's random salt — what the grammar's `hash()`/`noise()` mix into
    /// their argument (ADR-0051): a number, or `"random"` for a salt drawn per
    /// app launch. **Not** an L-system key despite living in the L-system's
    /// table, where Plan 0010 reserved it and Plan 0047 gave it a meaning. The
    /// expansion is deterministic and ignores it. Any system's preset may
    /// declare one.
    #[serde(default)]
    pub(in crate::preset::schema) seed: Option<RawSeed>,
    /// Star pattern: the regular tiling (e.g. `"6.6.6"` / `"hexagon"` / `"12"`).
    #[serde(default)]
    pub(in crate::preset::schema) tiling: Option<String>,
    /// Star pattern: contact angle in degrees.
    #[serde(default)]
    pub(in crate::preset::schema) contact_angle_deg: Option<f32>,
    /// Star pattern: the ring roster that fills the interior (ADR-0079). Absent
    /// — the default — is the Hankin interlace alone, i.e. the scene as it was.
    #[serde(default)]
    pub(in crate::preset::schema) rings: Vec<RawRing>,
}

/// One raw `[generator] rings` entry, before validation. `count` is an `i64`
/// rather than a `u32` so a negative literal reaches
/// [`into_star`](RawGenerator::into_star) and is rejected with a message about
/// ring counts, instead of failing as an anonymous serde type error.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawRing {
    /// Which motif to repeat — a name from the closed roster.
    pub(in crate::preset::schema) motif: String,
    /// Copies around the ring.
    pub(in crate::preset::schema) count: i64,
    /// Distance from the frame centre to each copy's centre.
    pub(in crate::preset::schema) radius: f32,
    /// Motif size multiplier.
    #[serde(default)]
    pub(in crate::preset::schema) scale: Option<f32>,
    /// Angular offset of copy 0, in radians.
    #[serde(default)]
    pub(in crate::preset::schema) phase: Option<f32>,
}

impl RawGenerator {
    /// Validate the table as an L-system config: a non-empty axiom, single-char
    /// rule predecessors, a finite angle, and a depth in `1..=MAX_LSYSTEM_DEPTH`.
    /// Every failure is a surfaced load error, never a panic (ADR-0007).
    pub(in crate::preset::schema) fn into_lsystem(self) -> Result<GeneratorConfig, PresetError> {
        let axiom = self
            .axiom
            .filter(|a| !a.is_empty())
            .ok_or_else(|| PresetError::Config("lsystem needs a non-empty axiom".into()))?;

        let mut rules = Vec::with_capacity(self.rules.len());
        for (pred, succ) in self.rules {
            let mut chars = pred.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return Err(PresetError::Config(format!(
                    "lsystem rule key '{pred}' must be a single character"
                )));
            };
            rules.push((c, succ));
        }
        if rules.is_empty() {
            return Err(PresetError::Config(
                "lsystem needs at least one rule".into(),
            ));
        }

        let angle_deg = self.angle_deg.unwrap_or(25.0);
        if !angle_deg.is_finite() {
            return Err(PresetError::Config(
                "lsystem angle_deg must be finite".into(),
            ));
        }

        let max_depth = self.max_depth.unwrap_or(4);
        if max_depth == 0 || max_depth > MAX_LSYSTEM_DEPTH {
            return Err(PresetError::Config(format!(
                "lsystem max_depth must be 1..={MAX_LSYSTEM_DEPTH}, got {max_depth}"
            )));
        }

        Ok(GeneratorConfig::LSystem {
            axiom,
            rules,
            angle_deg,
            max_depth,
            // Still inert (the expansion is deterministic and ignores it), so a
            // `"random"` seed reads as its numeric fallback here rather than
            // pulling entropy into a structural config.
            seed: self.seed.map_or(0, RawSeed::numeric),
        })
    }

    /// Validate the table as a star-pattern config: a known regular tiling (or
    /// `"none"`), a finite contact angle, and a well-formed ring roster. Every
    /// failure is a surfaced load error (ADR-0007).
    ///
    /// **The roster is validated exactly once, here** — the project's
    /// validate-at-the-boundary rule. Downstream, `build_rings` trusts every
    /// motif, count, radius, scale and phase it is handed.
    pub(in crate::preset::schema) fn into_star(self) -> Result<GeneratorConfig, PresetError> {
        let tiling = self
            .tiling
            .ok_or_else(|| PresetError::Config("star_pattern needs a tiling".into()))?;
        // `"none"` is the rings-only composition (ADR-0079): no interlace at all,
        // which the star construction expresses as order 0.
        let order = if tiling.trim().eq_ignore_ascii_case("none") {
            0
        } else {
            hankin::tiling_order(&tiling)
                .ok_or_else(|| PresetError::Config(format!("unknown tiling '{tiling}'")))?
        };

        let contact_angle_deg = self.contact_angle_deg.unwrap_or(30.0);
        if !contact_angle_deg.is_finite() {
            return Err(PresetError::Config(
                "star_pattern contact_angle_deg must be finite".into(),
            ));
        }

        let mut rings = Vec::with_capacity(self.rings.len());
        for (i, raw) in self.rings.into_iter().enumerate() {
            let motif = Motif::from_name(&raw.motif).ok_or_else(|| {
                let roster: Vec<&str> = Motif::ALL.iter().map(|m| m.name()).collect();
                PresetError::Config(format!(
                    "ring {i}: unknown motif '{}' (the roster is closed: {})",
                    raw.motif,
                    roster.join(", ")
                ))
            })?;
            if raw.count < 1 || raw.count > i64::from(MAX_RING_COUNT) {
                return Err(PresetError::Config(format!(
                    "ring {i}: count must be 1..={MAX_RING_COUNT}, got {}",
                    raw.count
                )));
            }
            let scale = raw.scale.unwrap_or(DEFAULT_RING_SCALE);
            let phase = raw.phase.unwrap_or(0.0);
            if !raw.radius.is_finite() || !scale.is_finite() || !phase.is_finite() {
                return Err(PresetError::Config(format!(
                    "ring {i}: radius, scale and phase must all be finite"
                )));
            }
            // A `scallop` reads its ring's `scale` as the lobe's **depth**, and a
            // negative one is not a shallower dimple: past
            // `depth = -R * (cos(s) + sin(s) - 1)` the arc's two ends cross to
            // the far side of its centre, the counter-clockwise sweep runs the
            // long way round, and the lobe bulges outward to roughly twice the
            // ring radius. Nothing downstream warns — it is a well-formed arc,
            // inside the cap, drawn wrong. Refused here rather than drawn,
            // because whether an inward scallop is a look anyone wants is a
            // question for the content lane and not a default (ADR-0158's plan,
            // design-backlog 0136).
            if motif.is_scallop() && scale < 0.0 {
                return Err(PresetError::Config(format!(
                    "ring {i}: a scallop's scale is its lobe depth and must be \
                     >= 0, got {scale}"
                )));
            }
            rings.push(RingSpec {
                motif,
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                count: raw.count as u32,
                radius: raw.radius,
                scale,
                phase,
            });
        }

        // Without an interlace *and* without rings there is no figure at all, and
        // a preset that draws nothing is a mistake worth naming rather than a
        // black frame the sanity gate reports later.
        if order == 0 && rings.is_empty() {
            return Err(PresetError::Config(
                "star_pattern tiling = \"none\" draws no interlace, so it needs at \
                 least one entry in rings"
                    .into(),
            ));
        }

        Ok(GeneratorConfig::Star {
            order,
            contact_angle_deg,
            rings,
        })
    }
}

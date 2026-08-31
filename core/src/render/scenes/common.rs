//! The parameter blocks the scenes share.
//!
//! Twelve systems implement [`Scene`](super::Scene) and most of them accept the
//! same colour and framing names — `palette_mix`, `palette_steps`,
//! `palette_contour`, `saturation`, `hue`, `brightness`, `pan_x`, `pan_y`. Each
//! spelled its own field, its own `set_param` arm, its own `reset_params` line
//! and its own `DEFAULT_*` const for them. The names are the preset grammar's,
//! not any one scene's, so their storage and their resting values belong in one
//! place; `presets/README.md` is still the roster of **which** system accepts
//! which name, and this module changes none of it.
//!
//! # These hold raw values
//!
//! A `set_param` stores what the expression evaluated to, and the reading side —
//! the uniform packing, or the shader — decides what is in range. That split is
//! deliberate and predates this module: a `[smoothing]` entry sweeps a binding
//! *continuously* toward its target, so clamping on the way in would quantize the
//! sweep rather than the destination. Nothing here clamps, rounds, or rejects.
//!
//! # Two of the six are not universal
//!
//! `saturation` rests at 1.0 and `palette_mix`, `palette_steps`,
//! `palette_contour`, `pan_x` and `pan_y` at their off values on every system
//! that has them, so those resting values are stated once below. `hue` and
//! `brightness` are not: the line families each open on a different hue (0.3
//! l-system, 0.5 star, 0.55 spectrum, 0.6 parametric, 0.0 elsewhere) and the
//! swarm rests at `brightness = 0.8` where everything else rests at 1.0. Those
//! are the family's signature look, so [`PaletteParams::new`] takes them from the
//! scene rather than this module inventing one answer.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set).
// `set_param` runs once per bound param per frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::palette;

/// `saturation` at rest: fully saturated, the palette's own colour.
pub(crate) const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` at rest: 0, palette A alone, so the B crossfade is off.
pub(crate) const DEFAULT_PALETTE_MIX: f32 = 0.0;
/// `pan_x` / `pan_y` at rest: centred.
pub(crate) const DEFAULT_PAN: f32 = 0.0;
/// `brightness` at rest on every system **but the swarm**, which rests at 0.8.
/// Not a universal default, so [`PaletteParams::new`] takes it as an argument and
/// this is only the value eleven of the twelve pass — including the three whose
/// roster has no `brightness` at all, where it is inert.
pub(crate) const DEFAULT_BRIGHTNESS: f32 = 1.0;

/// The names [`PaletteParams::set`] answers, as a roster.
///
/// **Test-only, and deliberately not what `set` matches against.** The setter
/// keeps its `match` — it runs once per bound param per frame and a linear scan
/// of a slice is the wrong shape for that — so this is a second statement of the
/// vocabulary, and `each_block_answers_exactly_its_roster` below is what holds
/// the two together. The source-text scan in `core/tests/preset.rs` carries a
/// third copy, because that is an integration test and this module is
/// `pub(crate)`.
#[cfg(test)]
const PALETTE_PARAMS: &[&str] = &[
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "saturation",
    "hue",
    "brightness",
];

/// The names [`PanParams::set`] answers, as a roster. Test-only, like
/// [`PALETTE_PARAMS`].
#[cfg(test)]
const PAN_PARAMS: &[&str] = &["pan_x", "pan_y"];

/// The palette-facing params a shader-coloured scene reads.
///
/// [`set`](PaletteParams::set) recognizes all six names. A scene whose roster is
/// narrower — `shape_collage` accepts only `saturation` and `palette_mix`,
/// `shape_field` adds the two banding names, `fragment_field` has no
/// `brightness` — still delegates the whole block: the roster that decides what a
/// preset may bind is the scene's own `PARAMS`, which `is_known_param` reads and
/// this type does not touch, and a field nothing packs into a uniform is read by
/// nothing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaletteParams {
    /// `palette_mix` — the A/B crossfade position.
    pub mix: f32,
    /// `palette_steps` — the band count (ADR-0078); quantized at the read side.
    pub steps: f32,
    /// `palette_contour` — the contour line drawn at each band edge.
    pub contour: f32,
    /// `saturation` — 0 desaturates toward grey, 1 is the palette's own colour.
    pub saturation: f32,
    /// `hue` — the palette coordinate the scene starts from.
    pub hue: f32,
    /// `brightness` — the scene's overall light level.
    pub brightness: f32,
    /// What `reset` returns `hue` to. Per scene: see the module docs.
    hue_rest: f32,
    /// What `reset` returns `brightness` to. Per scene: see the module docs.
    brightness_rest: f32,
}

impl PaletteParams {
    /// A block at rest, with the scene's own resting `hue` and `brightness`.
    pub(crate) fn new(hue_rest: f32, brightness_rest: f32) -> Self {
        Self {
            mix: DEFAULT_PALETTE_MIX,
            steps: palette::DEFAULT_PALETTE_STEPS,
            contour: palette::DEFAULT_PALETTE_CONTOUR,
            saturation: DEFAULT_SATURATION,
            hue: hue_rest,
            brightness: brightness_rest,
            hue_rest,
            brightness_rest,
        }
    }

    /// Store `value` if `name` is one of this block's, and say whether it was.
    ///
    /// A scene's `set_param` calls this first and matches its own names when it
    /// returns `false`, so the shared names cannot drift apart across twelve
    /// files.
    pub(crate) fn set(&mut self, name: &str, value: f32) -> bool {
        match name {
            "palette_mix" => self.mix = value,
            "palette_steps" => self.steps = value,
            "palette_contour" => self.contour = value,
            "saturation" => self.saturation = value,
            "hue" => self.hue = value,
            "brightness" => self.brightness = value,
            _ => return false,
        }
        true
    }

    /// Back to rest — what a preset switch leaves behind for any name the
    /// incoming preset does not bind.
    pub(crate) fn reset(&mut self) {
        self.mix = DEFAULT_PALETTE_MIX;
        self.steps = palette::DEFAULT_PALETTE_STEPS;
        self.contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.saturation = DEFAULT_SATURATION;
        self.hue = self.hue_rest;
        self.brightness = self.brightness_rest;
    }
}

/// `pan_x` / `pan_y` — the view offset every scene but `warp_mesh` accepts.
///
/// Its own type rather than two more fields on [`PaletteParams`] because it is
/// framing, not colour: the two blocks are accepted by different sets of scenes,
/// and a scene that pans without a palette (or the reverse) would otherwise have
/// to carry the half it does not use.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PanParams {
    /// `pan_x` — horizontal view offset.
    pub x: f32,
    /// `pan_y` — vertical view offset.
    pub y: f32,
}

impl Default for PanParams {
    fn default() -> Self {
        Self {
            x: DEFAULT_PAN,
            y: DEFAULT_PAN,
        }
    }
}

impl PanParams {
    /// Store `value` if `name` is `pan_x` or `pan_y`, and say whether it was.
    pub(crate) fn set(&mut self, name: &str, value: f32) -> bool {
        match name {
            "pan_x" => self.x = value,
            "pan_y" => self.y = value,
            _ => return false,
        }
        true
    }

    /// Back to centred.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    // Test asserts panic on failure; allowed here over the file's pragma.
    #![allow(clippy::panic)]

    use super::*;

    /// **Each block answers exactly its roster** — no more, no less.
    ///
    /// `core/tests/preset.rs`'s drift guard cannot see these eight names: it reads
    /// `set_param`'s match arms out of the source text, and every scene
    /// *delegates* them here rather than matching them. That is the right
    /// factoring — one implementation of what `saturation` means — and this is
    /// what replaces the coverage it costs, together with that guard's own check
    /// that a scene declaring one of these names carries the delegation.
    #[test]
    fn each_block_answers_exactly_its_roster() {
        let mut palette = PaletteParams::new(0.0, 1.0);
        for name in PALETTE_PARAMS {
            assert!(
                palette.set(name, 0.25),
                "`{name}` is in the roster but `PaletteParams::set` drops it"
            );
        }
        for name in PAN_PARAMS
            .iter()
            .copied()
            .chain(["zoom", "color_span", "", "Saturation"])
        {
            assert!(!palette.set(name, 0.25), "`{name}` is not this block's");
        }

        let mut pan = PanParams::default();
        for name in PAN_PARAMS {
            assert!(
                pan.set(name, 0.25),
                "`{name}` is in the roster but `PanParams::set` drops it"
            );
        }
        for name in PALETTE_PARAMS {
            assert!(!pan.set(name, 0.25), "`{name}` is not this block's");
        }
        assert!(pan.set("pan_y", -0.25));
        assert_eq!((pan.x, pan.y), (0.25, -0.25));
    }

    /// **No system declares a shared name that neither block answers.**
    ///
    /// The scenes' `PARAMS` lists are the roster a preset binds against, and each
    /// scene now delegates the colour and framing names wholesale. A name that
    /// *looks* shared but is spelled slightly differently in one scene's `PARAMS`
    /// would be declared, filtered out of the source-text guard, and answered by
    /// nothing.
    #[test]
    fn every_system_that_declares_a_shared_name_uses_the_shared_spelling() {
        use crate::preset::SystemKind;

        let mut palette = PaletteParams::new(0.0, 1.0);
        let mut pan = PanParams::default();
        let mut seen = 0usize;
        for system in SystemKind::ALL {
            for name in system.param_names() {
                if PALETTE_PARAMS.contains(name) || PAN_PARAMS.contains(name) {
                    seen += 1;
                    assert!(
                        palette.set(name, 0.5) || pan.set(name, 0.5),
                        "`{}` declares `{name}`, which neither shared block answers",
                        system.as_str()
                    );
                }
            }
        }
        assert!(
            seen > 40,
            "only {seen} shared-name declarations found across twelve systems — \
             the scan has stopped seeing them, so this guard would pass vacuously"
        );
    }

    /// **`reset` returns `hue` and `brightness` to the scene's resting values,
    /// not to a shared one** — the swarm rests dimmer and each line family opens
    /// on its own hue, and a preset switch must land back on that.
    #[test]
    fn reset_restores_the_scenes_own_hue_and_brightness() {
        let mut params = PaletteParams::new(0.55, 0.8);
        params.set("hue", 0.1);
        params.set("brightness", 2.0);
        params.set("saturation", 0.0);
        params.set("palette_mix", 1.0);
        params.reset();
        assert_eq!(params.hue, 0.55);
        assert_eq!(params.brightness, 0.8);
        assert_eq!(params.saturation, DEFAULT_SATURATION);
        assert_eq!(params.mix, DEFAULT_PALETTE_MIX);
        assert_eq!(params.steps, palette::DEFAULT_PALETTE_STEPS);
        assert_eq!(params.contour, palette::DEFAULT_PALETTE_CONTOUR);

        let mut pan = PanParams::default();
        pan.set("pan_x", 1.0);
        pan.reset();
        assert_eq!((pan.x, pan.y), (DEFAULT_PAN, DEFAULT_PAN));
    }

    /// **Nothing here clamps or rounds** — the reading side owns range, so a
    /// `[smoothing]` sweep passes through untouched (module docs).
    #[test]
    fn the_blocks_store_raw_values() {
        let mut params = PaletteParams::new(0.0, 1.0);
        params.set("saturation", -3.0);
        params.set("palette_steps", 1e9);
        params.set("brightness", f32::INFINITY);
        assert_eq!(params.saturation, -3.0);
        assert_eq!(params.steps, 1e9);
        assert!(params.brightness.is_infinite());
    }
}

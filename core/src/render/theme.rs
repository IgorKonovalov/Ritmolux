//! The interface's look, declared once (ADR-0252): colour roles, a type scale,
//! spacing, a corner radius, and motion timings with one easing curve.
//!
//! Every engine-drawn surface — the standalone's overlays, the core's
//! now-playing banner and diagnostics panel, and so the foobar component's
//! panel too — reads its values from [`THEME`]. The studio's `tokens.css` is
//! generated from the same table by [`css`], and `core/tests/suite/ui_tokens.rs`
//! fails when the committed file differs from what this module renders.
//!
//! The table names **roles**, never uses: `accent`, not `browser_row_selected`.
//! A surface restyle is a change of which role a surface reads.
//!
//! ## Colour encoding
//!
//! A [`Color`] holds **sRGB-encoded** components in `0.0..=1.0`, the numbers a
//! hex literal spells. That is what the text layer takes (glyphon treats its
//! colours as sRGB and linearises them for an sRGB target) and what CSS takes.
//! A pass that writes a colour straight out of a shader onto the `*Srgb`
//! surface — the diagnostics panel's quads — must hand the shader
//! [`Color::linear`] instead, or every value lands lighter than declared.
//!
//! Pure data, no platform or windowing type: this module is part of the
//! source-agnostic core and compiles in every build, with or without `text`.

// Hot-path panic-denial pragma (`render/` scan set): the easing curve is
// evaluated every frame an overlay moves.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::fmt::Write as _;

/// One colour role: sRGB-encoded RGBA, each component in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red, sRGB-encoded.
    pub r: f32,
    /// Green, sRGB-encoded.
    pub g: f32,
    /// Blue, sRGB-encoded.
    pub b: f32,
    /// Opacity, which no encoding touches.
    pub a: f32,
}

impl Color {
    /// An opaque colour from a `0xRRGGBB` literal.
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xff) as f32 / 255.0,
            g: ((rgb >> 8) & 0xff) as f32 / 255.0,
            b: (rgb & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// The same colour at opacity `a`.
    pub const fn alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// `[r, g, b, a]`, sRGB-encoded — what a text run and a `Line` take.
    pub const fn rgba(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// `[r, g, b]`, sRGB-encoded, for a caller that applies its own alpha.
    pub const fn rgb(self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    /// `[r, g, b, a]` with the colour channels linearised and alpha untouched —
    /// what a shader writing to an `*Srgb` target must output to show this
    /// colour as declared.
    pub fn linear(self) -> [f32; 4] {
        [
            srgb_to_linear(self.r),
            srgb_to_linear(self.g),
            srgb_to_linear(self.b),
            self.a,
        ]
    }

    /// The CSS spelling: `#rrggbb` when opaque, `#rrggbbaa` otherwise.
    pub fn css(self) -> String {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let mut out = format!(
            "#{:02x}{:02x}{:02x}",
            byte(self.r),
            byte(self.g),
            byte(self.b)
        );
        if self.a < 1.0 {
            let _ = write!(out, "{:02x}", byte(self.a));
        }
        out
    }
}

/// The IEC 61966-2-1 sRGB decode of one channel.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The one easing curve every surface moves on, as CSS's `cubic-bezier(x1, y1,
/// x2, y2)` with the endpoints pinned at `(0, 0)` and `(1, 1)`.
///
/// **Both `y` control points must stay inside `0.0..=1.0`.** That is what keeps
/// the curve monotone — an envelope that overshoots and comes back would make a
/// panel's alpha rise past its target and fall again. Both `x` control points
/// must stay inside `0.0..=1.0` too, which is what makes `x(t)` invertible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ease {
    /// First control point, time axis.
    pub x1: f32,
    /// First control point, progress axis.
    pub y1: f32,
    /// Second control point, time axis.
    pub x2: f32,
    /// Second control point, progress axis.
    pub y2: f32,
}

impl Ease {
    /// Progress at normalised time `t`: `0.0` at `t <= 0`, `1.0` at `t >= 1`, and
    /// the curve's height in between.
    ///
    /// Solves `x(s) = t` by bisection on the curve parameter `s` — a fixed 40
    /// halvings in `f64`, which puts `s` well inside an `f32` step of the root
    /// and allocates nothing.
    ///
    /// **Monotone in `t` to the last bit, by construction.** The height is read
    /// at the bisection's lower bound, which never decreases as `t` grows (every
    /// comparison `t` passes, a larger `t` passes too), and it is computed in
    /// `f64` and rounded once — an `f32` evaluation of the height wobbles by an
    /// ulp near the top of a flat ease-out, which is a panel's alpha stepping
    /// back down on its last frames.
    pub fn at(self, t: f32) -> f32 {
        if !t.is_finite() || t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let t = f64::from(t);
        let (x1, x2) = (f64::from(self.x1), f64::from(self.x2));
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if bezier(x1, x2, mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let y = bezier(f64::from(self.y1), f64::from(self.y2), lo);
        (y as f32).clamp(0.0, 1.0)
    }

    /// The CSS spelling, `cubic-bezier(x1, y1, x2, y2)`.
    pub fn css(self) -> String {
        format!(
            "cubic-bezier({}, {}, {}, {})",
            self.x1, self.y1, self.x2, self.y2
        )
    }
}

/// One coordinate of a cubic Bézier from `0` to `1` with control values `p1`
/// and `p2`, at parameter `s`.
fn bezier(p1: f64, p2: f64, s: f64) -> f64 {
    let u = 1.0 - s;
    3.0 * u * u * s * p1 + 3.0 * u * s * s * p2 + s * s * s
}

/// The whole look. One instance, [`THEME`]; a field is a role, a step, or a
/// timing, and never names the surface that reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The studio's window background. No engine surface paints it: the scene is
    /// the standalone's background.
    pub bg: Color,
    /// A panel's fill, translucent so the scene reads through it.
    pub panel: Color,
    /// A panel's 1 px lit edge.
    pub panel_edge: Color,
    /// The scanline modulation a panel carries on every [`Self::scanline_pitch`]th line.
    pub scanline: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text: headers, captions, status lines.
    pub text_dim: Color,
    /// Tertiary text: a placeholder, a control that is off.
    pub text_faint: Color,
    /// The warm accent: what the keys act on.
    pub accent: Color,
    /// The fill behind a highlighted row.
    pub highlight: Color,
    /// A marked-as-favourite item, warm but quieter than the accent.
    pub favourite: Color,
    /// A healthy reading.
    pub good: Color,
    /// A cool informational fill: a meter's level.
    pub info: Color,
    /// A reading near its limit.
    pub warn: Color,
    /// A reading past its limit, or a fault.
    pub error: Color,
    /// A meter's empty track.
    pub trough: Color,
    /// Font sizes in px at a 1080-line target, smallest first. Declared for the
    /// studio's generated tokens; no engine surface reads them yet, and the
    /// overlays still carry their own fixed sizes. A surface that adopts them
    /// scales by `target height / 1080`, never by an internal grid (ADR-0037).
    pub type_scale: [f32; 5],
    /// Spacing steps in px at the same 1080-line reference, smallest first.
    pub space: [f32; 5],
    /// Corner radius of a panel, px at the reference.
    pub radius: f32,
    /// Rows between scanlines on a panel.
    pub scanline_pitch: u32,
    /// Every panel's open and close, and every short state change.
    pub motion_short_ms: u32,
    /// A change that moves a whole view.
    pub motion_long_ms: u32,
    /// The one easing curve.
    pub ease: Ease,
}

/// The reference target height [`Theme::type_scale`] and [`Theme::space`] are
/// declared at.
pub const REFERENCE_HEIGHT: f32 = 1080.0;

const ACCENT: Color = Color::hex(0xffb454);

/// The look: direction A, "Amber phosphor" (Plan 0231 Phase 3) — a warm amber
/// accent over cool dark neutrals, panels with a lit edge and a faint scanline,
/// and one ease-out curve with a long settle.
pub const THEME: Theme = Theme {
    bg: Color::hex(0x0b0d11),
    panel: Color::hex(0x11141a).alpha(0.92),
    panel_edge: ACCENT.alpha(0.43),
    scanline: Color::hex(0x000000).alpha(0.06),
    text: Color::hex(0xece8e1),
    text_dim: Color::hex(0xa7acb5),
    text_faint: Color::hex(0x6b717c),
    accent: ACCENT,
    highlight: ACCENT.alpha(0.13),
    favourite: Color::hex(0xe8c9a0),
    good: Color::hex(0x5fd38d),
    info: Color::hex(0x6cb6ff),
    warn: Color::hex(0xf0b429),
    error: Color::hex(0xf2686c),
    trough: Color::hex(0x262a33),
    type_scale: [14.0, 18.0, 22.0, 28.0, 32.0],
    space: [4.0, 8.0, 16.0, 24.0, 32.0],
    radius: 6.0,
    scanline_pitch: 4,
    motion_short_ms: 180,
    motion_long_ms: 320,
    ease: Ease {
        x1: 0.16,
        y1: 1.0,
        x2: 0.3,
        y2: 1.0,
    },
};

/// The studio's `tokens.css`, rendered from [`THEME`].
///
/// Deterministic and newline-terminated. The custom property names are the
/// studio's own (`--bg`, `--panel`, `--text-dim`, ...), so its stylesheets read a
/// role by the same name the table gives it.
pub fn css() -> String {
    let t = THEME;
    let mut out = String::new();
    out.push_str(
        "/* GENERATED from core/src/render/theme.rs by core/tests/suite/ui_tokens.rs.\n   \
         Do not edit: regenerate with RLX_UPDATE_UI_TOKENS=1 (docs/developing.md). */\n",
    );
    out.push_str(":root {\n");
    let colors: [(&str, Color); 15] = [
        ("bg", t.bg),
        ("panel", t.panel),
        ("panel-edge", t.panel_edge),
        ("scanline", t.scanline),
        ("text", t.text),
        ("text-dim", t.text_dim),
        ("text-faint", t.text_faint),
        ("accent", t.accent),
        ("highlight", t.highlight),
        ("favourite", t.favourite),
        ("good", t.good),
        ("info", t.info),
        ("warn", t.warn),
        ("error", t.error),
        ("trough", t.trough),
    ];
    for (name, color) in colors {
        let _ = writeln!(out, "  --{name}: {};", color.css());
    }
    for (i, px) in t.type_scale.iter().enumerate() {
        let _ = writeln!(out, "  --type-{i}: {px}px;");
    }
    for (i, px) in t.space.iter().enumerate() {
        let _ = writeln!(out, "  --space-{i}: {px}px;");
    }
    let _ = writeln!(out, "  --radius: {}px;", t.radius);
    let _ = writeln!(out, "  --scanline-pitch: {}px;", t.scanline_pitch);
    let _ = writeln!(out, "  --motion-short: {}ms;", t.motion_short_ms);
    let _ = writeln!(out, "  --motion-long: {}ms;", t.motion_long_ms);
    let _ = writeln!(out, "  --ease: {};", t.ease.css());
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_hex_literal_round_trips_through_css() {
        assert_eq!(Color::hex(0xffb454).css(), "#ffb454");
        assert_eq!(Color::hex(0x11141a).alpha(0.92).css(), "#11141aeb");
    }

    #[test]
    fn the_ease_is_pinned_at_both_ends_and_monotone_between() {
        let ease = THEME.ease;
        assert_eq!(ease.at(0.0), 0.0);
        assert_eq!(ease.at(1.0), 1.0);
        let mut prev = 0.0;
        for i in 1..=200 {
            let v = ease.at(i as f32 / 200.0);
            assert!(v >= prev, "the ease fell at step {i}: {prev} -> {v}");
            assert!(v <= 1.0, "the ease overshot at step {i}: {v}");
            prev = v;
        }
    }

    #[test]
    fn the_ease_is_an_ease_out() {
        // Past halfway in value by a quarter of the time: fast out, long settle.
        assert!(THEME.ease.at(0.25) > 0.5);
    }

    #[test]
    fn linear_decodes_srgb_and_leaves_alpha_alone() {
        let [r, g, b, a] = Color::hex(0xffffff).alpha(0.5).linear();
        assert!((r - 1.0).abs() < 1e-6 && (g - 1.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6);
        assert_eq!(a, 0.5);
        // Mid-grey encodes well above its linear value.
        assert!(Color::hex(0x808080).linear()[0] < 0.25);
    }
}

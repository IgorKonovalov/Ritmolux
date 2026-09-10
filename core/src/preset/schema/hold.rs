//! The musical edge a `[hold]` entry re-samples its binding on.
//!
//! A compiled shape rather than an on-disk one, in [`Easing`](super::Easing)'s
//! position and for its reason -- a preset's `[hold]` table is what produces it,
//! and `raw::RawHold` is what the table deserializes into before validation.

/// When a held binding takes a new value (ADR-0180 rule 2). Between edges the
/// scene keeps reading the value taken at the last one.
///
/// The two named edges are counters the analysis frame already publishes, not
/// conditions this type evaluates: `beat` is the frame's own one-frame beat
/// gate and `bar` is a change in its bar counter. A [`Period`](Self::Period)
/// re-samples on the render clock instead, which is the only edge in the
/// vocabulary that exists on a silent stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoldEdge {
    /// Every frame the analysis frame's beat gate fires.
    Beat,
    /// Every change of the analysis frame's bar counter. That counter is the
    /// confidence-gated downbeat estimate where the tracker has locked and its
    /// own tempo-driven count everywhere else, so this steps on something
    /// musical without promising it is the downbeat.
    Bar,
    /// Every `n` seconds of render time, `n > 0`. Re-sampling restarts the
    /// interval from the frame it happened on, so a long frame delays the next
    /// edge rather than banking a debt of them.
    Period(f32),
}

impl HoldEdge {
    /// The two named edges, for the load error's "expected one of" listing and
    /// for the reference, which renders this rather than restating it.
    pub const NAMED: [HoldEdge; 2] = [HoldEdge::Beat, HoldEdge::Bar];

    /// Parse a named edge, or `None` if the word is not one. A period is not
    /// spelled here -- it arrives as a number, or as a numeric string.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "beat" => HoldEdge::Beat,
            "bar" => HoldEdge::Bar,
            _ => return None,
        })
    }

    /// The canonical name of a named edge -- [`from_name`](Self::from_name)'s
    /// inverse. A period has no name and renders as its own number.
    pub fn as_str(self) -> &'static str {
        match self {
            HoldEdge::Beat => "beat",
            HoldEdge::Bar => "bar",
            HoldEdge::Period(_) => "seconds",
        }
    }
}

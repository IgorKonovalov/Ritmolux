//! **An authored silhouette, parsed from inline SVG path data** (ADR-0107).
//!
//! Every other silhouette this engine draws is one of five names in the `marks`
//! roster. This module is the escape hatch: a `[path] d = "M ... Z"` string is
//! parsed **once at load** into a closed contour of `samples` points, normalized
//! into the same `[-1, 1]` box a mark lives in, and handed to the scene as
//! ordinary structural config. Nothing here runs per frame.
//!
//! # The subset, and why it has edges
//!
//! Accepted: `M m L l H h V v C c S s Q q T t Z z` — moveto, the three line
//! forms, cubic and quadratic Beziers with their smooth-continuation forms, and
//! closepath. Two things are **refused by name** rather than approximated:
//!
//! - **`A`/`a`, the elliptical arc.** Its centre parameterisation is a
//!   different geometry from the Bezier forms above, and every design tool can
//!   export the same curve as cubics.
//! - **A second subpath.** A morph aligns two contours by arc length, and two
//!   paths with different subpath counts have no natural correspondence at all
//!   — so the pair that could not be aligned is refused at the one path rather
//!   than guessed at the morph (ADR-0107).
//!
//! Both refusals name what they found, because an author meeting one is holding
//! a file a browser renders correctly.
//!
//! # A malformed path is an error with a character offset
//!
//! Not a fallback shape. A silently mis-parsed path renders as a *plausible
//! wrong figure*, which reads as a design decision rather than as a mistake —
//! so every failure carries the byte offset into `d` where it was found, and
//! [`PathError`]'s `Display` leads with it.
//!
//! # The normalization is recorded, not inferred
//!
//! The contour's own tight bounding box is centred on the origin and its longer
//! axis scaled to exactly `[-1, 1]`; the centre and factor applied are kept on
//! the [`PathShape`]. Two consequences, and the second is the point: a path
//! authored at any scale or offset lands in the same place, so **swapping one
//! path for another does not also move the figure** — the preset's `scale` and
//! `pan` stay the only things that do.

use std::fmt;

/// The fewest points a resampled contour may carry: a triangle.
pub const MIN_SAMPLES: usize = 3;

/// The most points a resampled contour may carry.
///
/// The field is fullscreen and evaluates a `min` over every segment at **every
/// pixel of every frame**, so this is a per-pixel `O(N)` budget rather than a
/// memory one — which is why exceeding it is a load error rather than a silent
/// decimation.
///
/// **The number is measured, and the measurement disagreed with ADR-0107's
/// construction by an order of magnitude.** `core/tests/path_cost.rs` prices the
/// contour walk at ~0.105 ms per segment at 1920x1080 on the integrated adapter
/// `docs/nfr.md` §1's floor is calibrated against; the ADR predicted ~2 % of
/// such a GPU at 32 segments and measured 26 %. At **64** the field alone is
/// 46 % of the floor's 16.67 ms frame budget, which is the most that can be
/// spent while leaving the composite chain room — so this is where the ceiling
/// sits, and it is the same value as [`DEFAULT_SAMPLES`] because that is where
/// the two independent answers landed.
pub const MAX_SAMPLES: usize = 64;

/// The most arc pieces a fitted contour may carry before the fit is discarded
/// and the figure stays a polyline.
///
/// A bound on the uniform the chain rides in, and a bound on the point of doing
/// it at all: an arc piece costs more per pixel than a line segment, so a fit
/// that did not collapse the count is not worth evaluating. The measured counts
/// at the tightest budget below sit at 25 and under.
pub const MAX_ARC_PIECES: usize = 32;

/// The lateral error the arc fit is held to, in the contour's own normalized
/// units — one pixel at 1080p for a figure drawn at `scale = 2`.
///
/// The fit happens at parse time, where the `scale` the preset will bind is not
/// known and can move per frame, so the budget is fixed at the **tightest**
/// figure size an author would reach for. A figure drawn smaller than that is
/// fitted more finely than it needs, which costs pieces and never fidelity.
const ARC_FIT_BUDGET: f32 = 1.0 / 1080.0;

/// The arity a `[path]` resamples to when it names none.
///
/// The arity at which a *smooth* contour stops reading as faceted: the chord
/// sagitta of a 64-gon inscribed in the normalized figure is under a pixel at
/// 1080p, and at 32 it is around two and a half. A polygonal silhouette wants
/// far fewer and should say so — `samples` is a lever downward, because
/// [`MAX_SAMPLES`] leaves it none upward.
pub const DEFAULT_SAMPLES: usize = 64;

/// How finely a Bezier is flattened before the contour is resampled, as a
/// divisor of the figure's own extent.
///
/// Flattening happens *before* normalization, so the step has to be relative to
/// the source drawing's size, or the same shape authored in a 1000-unit viewBox
/// and in a 1-unit one would flatten to different fidelity. A subdivision this
/// fine has a chord error far below one resampled segment at [`MAX_SAMPLES`],
/// which is what keeps the resample — not the flatten — the thing that sets
/// fidelity.
const FLATTEN_PER_EXTENT: f32 = 256.0;

/// The most pieces one Bezier is flattened into, whatever its control polygon
/// measures. A bound on load-time work for a pathological single curve.
const MAX_FLATTEN_PER_SEGMENT: usize = 256;

/// Two points closer than this fraction of the figure's extent are the same
/// point. Consecutive duplicates are dropped before the bounding box is taken,
/// so a `Z` landing exactly on the start point leaves no zero-length closing
/// edge for the arc-length walk to divide by.
const DEDUPE_FRACTION: f32 = 1e-6;

/// Why a `[path] d` string could not be parsed.
///
/// Always carries the byte offset into `d` at which the problem was found — see
/// this module's header for why that is not optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// Byte offset into the `d` string at which the problem was found.
    pub offset: usize,
    /// What was wrong there.
    pub kind: PathErrorKind,
}

/// What was wrong with a `[path] d` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathErrorKind {
    /// The elliptical-arc command, which the subset excludes.
    EllipticalArc(char),
    /// A second `M`/`m` — the path has more than one subpath.
    MultipleSubpaths,
    /// A letter that is not a path command at all.
    UnknownCommand(char),
    /// The string did not begin with a moveto.
    MissingMoveTo,
    /// A command needed another coordinate and the string ran out, or held
    /// something that is not a number.
    ExpectedNumber,
    /// Operands appeared where no command could repeat them — a number after
    /// `Z`, or before any command.
    UnexpectedOperand,
    /// The path parsed but encloses no area: fewer than three distinct points,
    /// or every point on one spot.
    Degenerate,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at character {}: ", self.offset)?;
        match &self.kind {
            PathErrorKind::EllipticalArc(c) => write!(
                f,
                "elliptical arc '{c}' is not in the [path] subset. Its centre parameterisation is \
                 a different geometry from the Bezier commands, and every design tool can export \
                 the same curve as cubics — re-export with arcs converted to paths"
            ),
            PathErrorKind::MultipleSubpaths => write!(
                f,
                "a second subpath begins here, and [path] takes one closed contour. Two contours \
                 have no natural point correspondence, which is what a morph needs — draw the \
                 shape as a single outline, or drop the counter"
            ),
            PathErrorKind::UnknownCommand(c) => write!(
                f,
                "'{c}' is not a path command. The subset is M m L l H h V v C c S s Q q T t Z z"
            ),
            PathErrorKind::MissingMoveTo => {
                write!(f, "a path must begin with a moveto ('M' or 'm')")
            }
            PathErrorKind::ExpectedNumber => write!(f, "expected a number"),
            PathErrorKind::UnexpectedOperand => {
                write!(f, "a number appears where no command can consume it")
            }
            PathErrorKind::Degenerate => write!(
                f,
                "the path encloses no area — a silhouette needs at least three distinct points"
            ),
        }
    }
}

impl std::error::Error for PathError {}

/// A parsed, normalized, resampled closed contour.
///
/// The points are the contour itself: `samples` of them, evenly spaced **by arc
/// length** around the outline and **not** repeating the first at the end — the
/// closing edge from the last point back to the first is implicit, and both the
/// distance field and the arc-length walk here assume it.
#[derive(Debug, Clone, PartialEq)]
pub struct PathShape {
    points: Vec<[f32; 2]>,
    /// The same outline as a **G1-continuous chain of circular arcs**, fitted
    /// through the line renderer's own fitter (ADR-0098) — or empty where the
    /// fit was not worth keeping.
    ///
    /// Fitted from the **dense flattened** contour rather than from `points`, so
    /// the chain is not limited by the resample's arity: `samples` governs the
    /// polyline's fidelity and the fit's own budget governs the chain's.
    pieces: Vec<crate::render::scenes::lines::biarc::Piece>,
    source_center: [f32; 2],
    source_scale: f32,
}

impl PathShape {
    /// Parse inline SVG path data into a normalized contour of `samples` points.
    ///
    /// `samples` is trusted to be in [`MIN_SAMPLES`]`..=`[`MAX_SAMPLES`]; the
    /// `[path]` table checks it at the load boundary, which is where a range
    /// belongs.
    pub fn parse(d: &str, samples: usize) -> Result<Self, PathError> {
        let segments = parse_segments(d)?;
        let dense = flatten(&segments);
        Self::from_dense(dense, samples)
    }

    /// The contour's points, normalized into `[-1, 1]` on its longer axis.
    pub fn points(&self) -> &[[f32; 2]] {
        &self.points
    }

    /// The centre of the source drawing's bounding box, in the source's own
    /// units — the translation the normalization applied, recorded.
    pub fn source_center(&self) -> [f32; 2] {
        self.source_center
    }

    /// The factor the source drawing was scaled by — the reciprocal of half its
    /// longer bounding-box axis, recorded.
    pub fn source_scale(&self) -> f32 {
        self.source_scale
    }

    /// Twice the shoelace sum: positive when the contour winds
    /// counter-clockwise in a y-up frame, negative when it winds clockwise.
    ///
    /// The sign is what a morph pair has to agree on (ADR-0107) — a clockwise
    /// contour interpolating into a counter-clockwise one turns inside out
    /// through the middle, passing through zero area on the way.
    pub fn signed_area(&self) -> f32 {
        signed_area(&self.points)
    }

    /// **This contour re-expressed so that interpolating toward it from `from`
    /// is a morph rather than a scramble** (ADR-0107).
    ///
    /// The two alignment problems ADR-0107 says have answers, solved in the
    /// order they have to be:
    ///
    /// 1. **Winding, by signed area.** A clockwise contour interpolating into a
    ///    counter-clockwise one turns inside out through the middle — every
    ///    intermediate frame is a valid shape and the motion is wrong — and the
    ///    contour passes through zero enclosed area on the way. When the two
    ///    signs disagree, the target is walked backwards.
    /// 2. **Start point, by minimising total displacement over cyclic
    ///    offsets.** Without it a star morphing into a star can unwind through a
    ///    spiral: each point travels to a *correspondent* rather than to its
    ///    neighbour, and every intermediate frame is again valid. `O(N^2)` at
    ///    load, which at this arity is thousands of operations, so the
    ///    brute-force search is affordable and no cleverness is owed.
    ///
    /// The third — two paths with different **subpath counts** — has no answer,
    /// and is refused at the parser rather than guessed at here.
    ///
    /// Both contours must already carry the same number of points; `None` if
    /// they do not, which the load boundary prevents by parsing the pair at one
    /// arity.
    pub fn aligned_to(&self, from: &Self) -> Option<Self> {
        let n = self.points.len();
        if n != from.points.len() || n < 3 {
            return None;
        }

        // 1 — winding. `rev` walks the target backwards, which flips its signed
        // area and leaves the same figure.
        let flip = signed_area(&self.points) * signed_area(&from.points) < 0.0;
        let oriented: Vec<[f32; 2]> = if flip {
            self.points.iter().rev().copied().collect()
        } else {
            self.points.clone()
        };

        // 2 — start point. The cost is the sum of SQUARED displacements, which
        // has the same minimiser as the sum of distances and no square roots in
        // the inner loop.
        let mut best_offset = 0usize;
        let mut best_cost = f32::INFINITY;
        for offset in 0..n {
            let mut cost = 0.0f32;
            for i in 0..n {
                let (Some(&a), Some(&b)) = (from.points.get(i), oriented.get((i + offset) % n))
                else {
                    return None;
                };
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                cost += dx * dx + dy * dy;
            }
            if cost < best_cost {
                best_cost = cost;
                best_offset = offset;
            }
        }

        let mut points = Vec::with_capacity(n);
        for i in 0..n {
            points.push(*oriented.get((i + best_offset) % n)?);
        }
        Some(Self {
            points,
            pieces: Vec::new(),
            source_center: self.source_center,
            source_scale: self.source_scale,
        })
    }

    /// The fitted arc chain, or empty where the figure stays a polyline.
    pub(crate) fn pieces(&self) -> &[crate::render::scenes::lines::biarc::Piece] {
        &self.pieces
    }

    /// How many arc pieces the fit kept — `0` where the figure stays a polyline.
    ///
    /// The count rather than the chain, so a caller outside the crate can report
    /// what a curve cost without [`biarc::Piece`](crate::render::scenes::lines::biarc)
    /// being public API.
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// Re-fit this contour's **points** to arcs at an arbitrary budget, for
    /// measuring what a curve costs in pieces at a given fidelity.
    ///
    /// Not the chain the scene draws — that one is fitted from the dense
    /// contour, at [`ARC_FIT_BUDGET`], and is [`pieces`](Self::pieces). This
    /// exists so the relationship between fidelity and piece count can be
    /// reported as a table rather than argued.
    #[cfg(test)]
    pub(crate) fn refit(&self, lateral: f32) -> Vec<crate::render::scenes::lines::biarc::Piece> {
        let mut out = Vec::new();
        let mut at = Vec::new();
        crate::render::scenes::lines::biarc::fit(&self.points, true, lateral, &mut out, &mut at);
        out
    }

    /// This contour resampled to `samples` points, evenly spaced by arc length
    /// from its own first point. `None` when the contour or the request is
    /// degenerate.
    pub fn resampled(&self, samples: usize) -> Option<Self> {
        Some(Self {
            points: resample(&self.points, samples)?,
            pieces: Vec::new(),
            source_center: self.source_center,
            source_scale: self.source_scale,
        })
    }

    /// Build from an already-flattened dense polyline: dedupe, take the tight
    /// bounding box, normalize, resample.
    fn from_dense(mut dense: Vec<[f32; 2]>, samples: usize) -> Result<Self, PathError> {
        // A dense flatten repeats the joint between pieces, and a `Z` re-states
        // the start point. Both would be zero-length edges in the walk below.
        let extent = rough_extent(&dense);
        dedupe(&mut dense, extent * DEDUPE_FRACTION);
        if dense.len() < 3 {
            return Err(PathError {
                offset: 0,
                kind: PathErrorKind::Degenerate,
            });
        }

        let (min, max) = bounds(&dense);
        let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
        let half = ((max[0] - min[0]) * 0.5).max((max[1] - min[1]) * 0.5);
        if !half.is_finite() || half <= 0.0 {
            return Err(PathError {
                offset: 0,
                kind: PathErrorKind::Degenerate,
            });
        }
        let scale = 1.0 / half;
        for p in &mut dense {
            p[0] = (p[0] - center[0]) * scale;
            p[1] = (p[1] - center[1]) * scale;
        }

        let points = resample(&dense, samples).ok_or(PathError {
            offset: 0,
            kind: PathErrorKind::Degenerate,
        })?;

        // **The fit reads the dense contour, not the resample.** A chain fitted
        // from `points` could be no more faithful than the polyline it came
        // from; fitted from the flatten it is limited only by its own budget, so
        // an arc figure's fidelity stops depending on `samples` at all.
        //
        // The chain is kept only where it is worth evaluating: it has to fit the
        // uniform, and it has to have collapsed the count — an arc piece costs
        // more per pixel than a line segment, so a chain the same length as the
        // polyline is strictly worse. A figure that is all corners (a polygon)
        // comes back from the fitter as the lines it went in as, and lands here.
        let mut pieces = Vec::new();
        let mut at = Vec::new();
        crate::render::scenes::lines::biarc::fit(
            &dense,
            true,
            ARC_FIT_BUDGET,
            &mut pieces,
            &mut at,
        );
        if pieces.len() > MAX_ARC_PIECES || pieces.len() * 2 > points.len() {
            pieces.clear();
        }

        Ok(Self {
            points,
            pieces,
            source_center: center,
            source_scale: scale,
        })
    }
}

/// One parsed segment, in absolute source coordinates. Its start point is the
/// previous segment's end, so it is not repeated here.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Seg {
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
}

/// Which kind of curve produced the last control point, for `S`/`T`'s
/// reflection. Anything else clears it, which is what makes an `S` after an `L`
/// reflect about the current point rather than about a stale handle.
#[derive(Clone, Copy, PartialEq)]
enum LastCtrl {
    None,
    Cubic([f32; 2]),
    Quad([f32; 2]),
}

/// The scanner over `d`: a byte cursor plus the number and separator rules SVG
/// path data uses — commas and whitespace are interchangeable, and both are
/// optional wherever a sign or a `.` already separates two numbers.
struct Scan<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scan<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s: s.as_bytes(),
            i: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// Whitespace and commas separate operands and may be omitted entirely.
    fn skip_sep(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() || b == b',' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    /// Whether a number could start here — the lookahead the implicit-repeat
    /// rule needs to tell "another operand group" from "the next command".
    fn at_number(&self) -> bool {
        matches!(self.peek(), Some(b) if b.is_ascii_digit() || b == b'+' || b == b'-' || b == b'.')
    }

    /// One number.
    ///
    /// Hand-rolled rather than delegated to `f32::from_str` over a slice found
    /// by scanning to the next separator: SVG allows `1.5.3` to mean two
    /// numbers, so where a number *ends* is part of the grammar, and stopping at
    /// the second `.` is what makes that path parse the way a browser parses it.
    fn number(&mut self) -> Result<f32, PathError> {
        self.skip_sep();
        let start = self.i;
        if matches!(self.peek(), Some(b'+') | Some(b'-')) {
            self.i += 1;
        }
        let mut digits = false;
        while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            self.i += 1;
            digits = true;
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
                self.i += 1;
                digits = true;
            }
        }
        if !digits {
            return Err(PathError {
                offset: start,
                kind: PathErrorKind::ExpectedNumber,
            });
        }
        // An exponent counts only when a digit actually follows it, so a stray
        // `e` does not swallow the cursor.
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            let save = self.i;
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            let mut exp_digits = false;
            while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
                self.i += 1;
                exp_digits = true;
            }
            if !exp_digits {
                self.i = save;
            }
        }
        let text = self
            .s
            .get(start..self.i)
            .and_then(|b| std::str::from_utf8(b).ok());
        let value = text
            .and_then(|t| t.parse::<f32>().ok())
            .filter(|v| v.is_finite());
        value.ok_or(PathError {
            offset: start,
            kind: PathErrorKind::ExpectedNumber,
        })
    }

    fn pair(&mut self) -> Result<[f32; 2], PathError> {
        let x = self.number()?;
        let y = self.number()?;
        Ok([x, y])
    }
}

/// Parse `d` into absolute segments, refusing what the subset excludes.
fn parse_segments(d: &str) -> Result<Vec<Seg>, PathError> {
    let mut scan = Scan::new(d);
    let mut segs: Vec<Seg> = Vec::new();
    let mut cur = [0.0f32, 0.0];
    let mut start = [0.0f32, 0.0];
    let mut last_ctrl = LastCtrl::None;
    let mut started = false;
    // The command an operand group repeats under when no letter is present. `0`
    // means "no command yet", which is what makes a leading number an error
    // rather than a silent lineto.
    let mut repeat: u8 = 0;

    loop {
        scan.skip_sep();
        let Some(b) = scan.peek() else { break };
        let at = scan.i;

        let cmd = if b.is_ascii_alphabetic() {
            scan.i += 1;
            b
        } else if scan.at_number() {
            // The implicit-repeat rule: a moveto's extra coordinate pairs are
            // linetos (`M x y x y` draws a line), every other command repeats
            // itself, and `Z` has no operands to repeat.
            match repeat {
                b'M' => b'L',
                b'm' => b'l',
                0 => {
                    return Err(PathError {
                        offset: at,
                        kind: PathErrorKind::MissingMoveTo,
                    });
                }
                b'Z' | b'z' => {
                    return Err(PathError {
                        offset: at,
                        kind: PathErrorKind::UnexpectedOperand,
                    });
                }
                other => other,
            }
        } else {
            return Err(PathError {
                offset: at,
                kind: PathErrorKind::UnexpectedOperand,
            });
        };

        if !started && !matches!(cmd, b'M' | b'm') {
            return Err(PathError {
                offset: at,
                kind: PathErrorKind::MissingMoveTo,
            });
        }

        // Relative commands are the lowercase half, and the point every one of
        // them is relative to is the current point.
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { [0.0, 0.0] };

        match cmd.to_ascii_uppercase() {
            b'M' => {
                if started {
                    return Err(PathError {
                        offset: at,
                        kind: PathErrorKind::MultipleSubpaths,
                    });
                }
                let p = scan.pair()?;
                cur = [base[0] + p[0], base[1] + p[1]];
                start = cur;
                started = true;
                last_ctrl = LastCtrl::None;
            }
            b'L' => {
                let p = scan.pair()?;
                cur = [base[0] + p[0], base[1] + p[1]];
                segs.push(Seg::Line(cur));
                last_ctrl = LastCtrl::None;
            }
            b'H' => {
                let x = scan.number()?;
                cur = [base[0] + x, cur[1]];
                segs.push(Seg::Line(cur));
                last_ctrl = LastCtrl::None;
            }
            b'V' => {
                let y = scan.number()?;
                cur = [cur[0], base[1] + y];
                segs.push(Seg::Line(cur));
                last_ctrl = LastCtrl::None;
            }
            b'C' => {
                let c1 = scan.pair()?;
                let c2 = scan.pair()?;
                let p = scan.pair()?;
                let c1 = [base[0] + c1[0], base[1] + c1[1]];
                let c2 = [base[0] + c2[0], base[1] + c2[1]];
                cur = [base[0] + p[0], base[1] + p[1]];
                segs.push(Seg::Cubic(c1, c2, cur));
                last_ctrl = LastCtrl::Cubic(c2);
            }
            b'S' => {
                let c2 = scan.pair()?;
                let p = scan.pair()?;
                // The reflected handle, and the classic place a hand-written
                // parser is wrong: it reflects the previous CUBIC's second
                // control point about the current point, and is the current
                // point itself when the previous command was not a cubic.
                let c1 = match last_ctrl {
                    LastCtrl::Cubic(prev) => [2.0 * cur[0] - prev[0], 2.0 * cur[1] - prev[1]],
                    _ => cur,
                };
                let c2 = [base[0] + c2[0], base[1] + c2[1]];
                cur = [base[0] + p[0], base[1] + p[1]];
                segs.push(Seg::Cubic(c1, c2, cur));
                last_ctrl = LastCtrl::Cubic(c2);
            }
            b'Q' => {
                let c = scan.pair()?;
                let p = scan.pair()?;
                let c = [base[0] + c[0], base[1] + c[1]];
                cur = [base[0] + p[0], base[1] + p[1]];
                segs.push(Seg::Quad(c, cur));
                last_ctrl = LastCtrl::Quad(c);
            }
            b'T' => {
                let p = scan.pair()?;
                // `T` reflects the previous QUADRATIC's control point — a `T`
                // after a cubic reflects nothing and draws a straight line.
                let c = match last_ctrl {
                    LastCtrl::Quad(prev) => [2.0 * cur[0] - prev[0], 2.0 * cur[1] - prev[1]],
                    _ => cur,
                };
                cur = [base[0] + p[0], base[1] + p[1]];
                segs.push(Seg::Quad(c, cur));
                last_ctrl = LastCtrl::Quad(c);
            }
            b'Z' => {
                cur = start;
                last_ctrl = LastCtrl::None;
            }
            b'A' => {
                return Err(PathError {
                    offset: at,
                    kind: PathErrorKind::EllipticalArc(b as char),
                });
            }
            _ => {
                return Err(PathError {
                    offset: at,
                    kind: PathErrorKind::UnknownCommand(b as char),
                });
            }
        }
        repeat = cmd;
    }

    if !started {
        return Err(PathError {
            offset: 0,
            kind: PathErrorKind::MissingMoveTo,
        });
    }
    if segs.is_empty() {
        return Err(PathError {
            offset: 0,
            kind: PathErrorKind::Degenerate,
        });
    }
    // The contour is closed whether or not the author wrote `Z`: a filled
    // silhouette has no open form, and an implicit close is what a browser
    // draws for `fill`. The closing edge lives in the point list's wrap rather
    // than in a segment, so nothing is appended — what IS prepended is the
    // moveto's own point, which the segments above carry only as an origin.
    segs.insert(0, Seg::Line(start));
    Ok(segs)
}

/// Flatten absolute segments into a dense polyline, starting at the first
/// segment's end — the moveto's point.
fn flatten(segs: &[Seg]) -> Vec<[f32; 2]> {
    let extent = control_extent(segs);
    let step = (extent / FLATTEN_PER_EXTENT).max(f32::MIN_POSITIVE);
    let mut out: Vec<[f32; 2]> = Vec::new();
    let mut cur = [0.0f32, 0.0];
    for seg in segs {
        match *seg {
            Seg::Line(p) => {
                out.push(p);
                cur = p;
            }
            Seg::Quad(c, p) => {
                let n = pieces(chord(cur, c) + chord(c, p), step);
                for k in 1..=n {
                    let t = k as f32 / n as f32;
                    out.push(quad_at(cur, c, p, t));
                }
                cur = p;
            }
            Seg::Cubic(c1, c2, p) => {
                let n = pieces(chord(cur, c1) + chord(c1, c2) + chord(c2, p), step);
                for k in 1..=n {
                    let t = k as f32 / n as f32;
                    out.push(cubic_at(cur, c1, c2, p, t));
                }
                cur = p;
            }
        }
    }
    out
}

/// How many pieces a curve whose control polygon measures `poly` is flattened
/// into at `step`.
fn pieces(poly: f32, step: f32) -> usize {
    let n = (poly / step).ceil();
    if !n.is_finite() || n < 1.0 {
        return 1;
    }
    (n as usize).min(MAX_FLATTEN_PER_SEGMENT)
}

fn chord(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

fn quad_at(p0: [f32; 2], c: [f32; 2], p1: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    [
        u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p1[0],
        u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p1[1],
    ]
}

fn cubic_at(p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p1: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        a * p0[0] + b * c1[0] + c * c2[0] + d * p1[0],
        a * p0[1] + b * c1[1] + c * c2[1] + d * p1[1],
    ]
}

/// The extent of every point a segment names, control points included — a
/// superset of the drawn figure's, and the scale the flatten step is relative
/// to. It is taken before flattening, which is the whole reason it reads control
/// points rather than the tight bounding box the normalization uses.
fn control_extent(segs: &[Seg]) -> f32 {
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    let mut see = |p: [f32; 2]| {
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[1]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[1]);
    };
    for seg in segs {
        match *seg {
            Seg::Line(p) => see(p),
            Seg::Quad(c, p) => {
                see(c);
                see(p);
            }
            Seg::Cubic(c1, c2, p) => {
                see(c1);
                see(c2);
                see(p);
            }
        }
    }
    let e = (max[0] - min[0]).max(max[1] - min[1]);
    if e.is_finite() && e > 0.0 { e } else { 1.0 }
}

fn rough_extent(points: &[[f32; 2]]) -> f32 {
    let (min, max) = bounds(points);
    let e = (max[0] - min[0]).max(max[1] - min[1]);
    if e.is_finite() && e > 0.0 { e } else { 1.0 }
}

fn bounds(points: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for p in points {
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[1]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[1]);
    }
    (min, max)
}

/// Drop consecutive points within `eps`, and the last point when it lands on
/// the first — the closing edge is implicit, so a repeated start point would be
/// a zero-length edge in the arc-length walk.
fn dedupe(points: &mut Vec<[f32; 2]>, eps: f32) {
    points.dedup_by(|a, b| chord(*a, *b) <= eps);
    while points.len() > 1 {
        let Some(&first) = points.first() else { break };
        let Some(&last) = points.last() else { break };
        if chord(first, last) <= eps {
            points.pop();
        } else {
            break;
        }
    }
}

/// Twice the shoelace sum over a closed polygon.
fn signed_area(points: &[[f32; 2]]) -> f32 {
    let n = points.len();
    let mut sum = 0.0;
    for i in 0..n {
        let (Some(&a), Some(&b)) = (points.get(i), points.get((i + 1) % n)) else {
            continue;
        };
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum
}

/// Walk the closed polygon and emit `samples` points evenly spaced by arc
/// length, starting exactly on `points[0]`.
///
/// Even spacing **by arc length** rather than per command: a shape whose
/// commands are unevenly sized would otherwise bunch its points where the author
/// happened to click, and a morph correspondence built on that bunching is wrong
/// everywhere the two shapes were drawn differently (ADR-0107).
fn resample(points: &[[f32; 2]], samples: usize) -> Option<Vec<[f32; 2]>> {
    let n = points.len();
    if n < 3 || samples < 3 {
        return None;
    }
    // Cumulative length at each vertex, wrapping: `cum[i]` is the distance from
    // `points[0]` to `points[i]` along the outline, and `cum[n]` the perimeter.
    let mut cum = Vec::with_capacity(n + 1);
    cum.push(0.0f32);
    let mut total = 0.0f32;
    for i in 0..n {
        let (Some(&a), Some(&b)) = (points.get(i), points.get((i + 1) % n)) else {
            return None;
        };
        total += chord(a, b);
        cum.push(total);
    }
    if !total.is_finite() || total <= 0.0 {
        return None;
    }

    let mut out = Vec::with_capacity(samples);
    let mut seg = 0usize;
    for k in 0..samples {
        let target = total * (k as f32) / (samples as f32);
        while seg + 1 < n && cum.get(seg + 1).is_some_and(|&c| c <= target) {
            seg += 1;
        }
        let (Some(&a), Some(&b)) = (points.get(seg), points.get((seg + 1) % n)) else {
            return None;
        };
        let (Some(&lo), Some(&hi)) = (cum.get(seg), cum.get(seg + 1)) else {
            return None;
        };
        let run = hi - lo;
        let t = if run > 0.0 {
            ((target - lo) / run).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
    Some(out)
}

#[cfg(test)]
mod tests;

//! The MilkDrop runtime: compiled EEL2 programs, the machine that executes them,
//! and the driver that turns their output into warp-mesh parameters
//! ([ADR-0113](../../../docs/adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)).
//!
//! **No `.milk` text, no HLSL and no translator is anywhere in this module.**
//! Conversion happens ahead of time in `milkconv`, which never ships; what
//! reaches a binary is bytecode, a stack VM, and the driver below.
//!
//! # The execution model, which is MilkDrop's
//!
//! A bundle carries three programs over **one shared register file**:
//!
//! 1. `per_frame_init` runs **once**, when the preset loads. It is where a preset
//!    seeds the `q` variables and its `megabuf`.
//! 2. `per_frame` runs **once per frame**, after the host has written this frame's
//!    audio and clock into the input registers. What it leaves in the output
//!    registers is the whole-mesh transform, and what it leaves in `q1`–`q32` is
//!    the bridge to the program below.
//! 3. `per_vertex` runs **once per mesh vertex**, starting each time from the
//!    register state `per_frame` left — so `q1` reads the same value at every
//!    vertex, and a write inside the program does not leak from one vertex to the
//!    next.
//!
//! That third property is why [`EelProgram::written_registers`] exists: the
//! restore is over the registers the program can actually write, not over the
//! whole file, which at thousands of vertices per frame is the difference between
//! a memcpy that matters and one that does not.
//!
//! # Rates: MilkDrop is per frame, this engine is per second
//!
//! **The single most consequential translation in the whole plan, and it is
//! here rather than in the converter.** MilkDrop's `zoom`, `rot`, `dx`, `dy`,
//! `warp` and `decay` are all *per rendered frame*: a preset written on a machine
//! running 30 fps drifts at half the speed on one running 60. This engine's
//! vocabulary is per second throughout (ADR-0019), which is what makes a look
//! identical on any display.
//!
//! So the driver converts, per frame, using the frame's own measured `dt`:
//! a factor becomes `v^fps` and a rate becomes `v * fps`. A preset authored
//! against MilkDrop's nominal **30 fps** therefore moves at the speed its author
//! saw, on any refresh — and a converted preset does not have to carry the
//! assumption in its bytecode. [`NOMINAL_FPS`] is the frame rate the `fps`
//! *variable* reports to the program, for the same reason: a preset that reads
//! `fps` and divides by it is compensating for a cadence, and telling it the
//! truth about a 144 Hz display would double-compensate.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to core/src/milk by
// Plan 0100 Phase 2). The driver runs per vertex per frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub mod bytecode;
pub mod vm;

use bytecode::{EelProgram, ProgramError};
use vm::{Budget, VmState};

/// The frame rate the `fps` variable reports, and the cadence a per-frame rate is
/// interpreted against.
///
/// MilkDrop's own nominal rate. A `.milk` preset's `zoom = 1.01` means "1 % per
/// frame at about this rate", and its author tuned it by eye there — so this is
/// the number that reproduces what they saw, not the display's actual refresh.
/// See the module docs.
pub const NOMINAL_FPS: f32 = 30.0;

/// The nine per-vertex outputs, in the order
/// [`warp_mesh::PER_VERTEX_PARAMS`](crate::render::scenes::warp_mesh::PER_VERTEX_PARAMS)
/// declares them — the same roster, because they *are* the same roster. A
/// converted preset and a hand-authored `[per_vertex]` table drive one scene.
const OUTPUT_NAMES: [&str; 9] = ["zoom", "rot", "cx", "cy", "dx", "dy", "sx", "sy", "warp"];

/// Whether output `i` is a **factor** (composed multiplicatively over time, so it
/// converts as `v^fps`) or a **rate** (`v * fps`). Positional with
/// [`OUTPUT_NAMES`].
///
/// `cx`/`cy` are neither: they are a *position*, not a motion, so they pass
/// through untouched. `false` here with a `false` in [`OUTPUT_RATE`] means that.
const OUTPUT_FACTOR: [bool; 9] = [true, false, false, false, false, false, true, true, false];
/// Whether output `i` is a rate — see [`OUTPUT_FACTOR`].
const OUTPUT_RATE: [bool; 9] = [false, true, false, false, true, true, false, false, true];

/// The composite outputs a per-frame program may set beyond the nine, in the
/// order [`MilkRuntime::run_frame`] returns them.
///
/// `decay` first because it is the one that is rate-converted; the six after it
/// are flags and a multiplier that pass through untouched, and they are exactly
/// [`warp_mesh::COMPOSITE_PARAMS`](crate::render::scenes::warp_mesh::COMPOSITE_PARAMS)
/// so the scene can apply them by name without a second table.
pub const EXTRA_OUTPUT_NAMES: [&str; 8] = [
    "decay",
    "gamma",
    "wrap",
    "darken_center",
    "brighten",
    "darken",
    "solarize",
    "invert",
];

/// Each extra output's value before a per-frame program runs, positionally with
/// [`EXTRA_OUTPUT_NAMES`]. MilkDrop's own defaults: 96 % of the previous frame
/// survives, unity gamma, and every flag off.
const EXTRA_DEFAULTS: [f32; 8] = [0.96, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

/// A converted preset's compiled programs: what a bundle carries beyond an
/// ordinary LMV preset.
///
/// Cloned into the warp mesh's structural config at load and never touched again
/// — the runtime state lives in [`MilkRuntime`], not here, so the same bundle can
/// drive two scenes (the roster's and a `[layer]`'s) without them sharing a
/// register file.
#[derive(Debug, Clone, PartialEq)]
pub struct MilkBundle {
    /// Run once at preset load.
    pub per_frame_init: EelProgram,
    /// Run once per frame.
    pub per_frame: EelProgram,
    /// Run once per mesh vertex.
    pub per_vertex: EelProgram,
}

/// What is wrong with a bundle, as a surfaced load error.
#[derive(Debug, Clone, PartialEq)]
pub enum BundleError {
    /// One of the three programs did not decode.
    Program {
        /// Which section — `per_frame_init`, `per_frame` or `per_vertex`.
        section: &'static str,
        /// Why.
        err: ProgramError,
    },
    /// The three programs declare different register rosters, so a `q1` written
    /// by one would not be the `q1` the next reads.
    RosterMismatch {
        /// The section whose roster differs from `per_frame`'s.
        section: &'static str,
    },
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BundleError::Program { section, err } => write!(f, "[milk] {section}: {err}"),
            BundleError::RosterMismatch { section } => write!(
                f,
                "[milk] {section} declares a different .regs roster from per_frame. \
                 The three programs share one register file — that sharing IS the \
                 q1..q32 bridge — so they must declare the same registers in the \
                 same order. `milkconv` emits them that way; a hand-written bundle \
                 has to as well."
            ),
        }
    }
}

impl std::error::Error for BundleError {}

impl MilkBundle {
    /// Decode a bundle from the three assembly sections. An absent section is the
    /// empty program, which runs nothing.
    pub fn from_assembly(
        per_frame_init: Option<&str>,
        per_frame: Option<&str>,
        per_vertex: Option<&str>,
    ) -> Result<Self, BundleError> {
        let decode = |section: &'static str, text: Option<&str>| match text {
            None => Ok(EelProgram::empty()),
            Some(text) => {
                EelProgram::from_assembly(text).map_err(|err| BundleError::Program { section, err })
            }
        };
        let bundle = Self {
            per_frame_init: decode("per_frame_init", per_frame_init)?,
            per_frame: decode("per_frame", per_frame)?,
            per_vertex: decode("per_vertex", per_vertex)?,
        };
        // The shared register file is the bridge, so the rosters have to agree.
        // An empty program declares nothing and is exempt.
        for (section, program) in [
            ("per_frame_init", &bundle.per_frame_init),
            ("per_vertex", &bundle.per_vertex),
        ] {
            if program.register_count() > 0
                && bundle.per_frame.register_count() > 0
                && program.names() != bundle.per_frame.names()
            {
                return Err(BundleError::RosterMismatch { section });
            }
        }
        Ok(bundle)
    }

    /// Whether any of the three programs draws from the RNG.
    pub fn uses_random(&self) -> bool {
        self.per_frame_init.uses_random()
            || self.per_frame.uses_random()
            || self.per_vertex.uses_random()
    }

    /// The roster the three programs share, for resolving indices once at load.
    fn roster(&self) -> &[String] {
        if self.per_frame.register_count() > 0 {
            self.per_frame.names()
        } else if self.per_vertex.register_count() > 0 {
            self.per_vertex.names()
        } else {
            self.per_frame_init.names()
        }
    }
}

/// The register indices the host writes before a per-frame run.
///
/// Every field is an `Option`: a program that never names `treb` has no register
/// for it, and writing one that does not exist is a no-op rather than an error.
/// Resolved **once at load** — nothing per frame looks a name up.
#[derive(Debug, Default, Clone, Copy)]
struct FrameInputs {
    bass: Option<u16>,
    mid: Option<u16>,
    treb: Option<u16>,
    bass_att: Option<u16>,
    mid_att: Option<u16>,
    treb_att: Option<u16>,
    time: Option<u16>,
    frame: Option<u16>,
    fps: Option<u16>,
    progress: Option<u16>,
    meshx: Option<u16>,
    meshy: Option<u16>,
    aspectx: Option<u16>,
    aspecty: Option<u16>,
}

/// The register indices the host writes before each per-vertex run.
#[derive(Debug, Default, Clone, Copy)]
struct VertexInputs {
    x: Option<u16>,
    y: Option<u16>,
    rad: Option<u16>,
    ang: Option<u16>,
}

/// One loaded bundle's live state: the VM's arena and the resolved indices.
///
/// Built at preset load, borrowed mutably per frame, and **never resized while a
/// preset renders** — the whole real-time claim.
pub struct MilkRuntime {
    bundle: MilkBundle,
    state: VmState,
    inputs: FrameInputs,
    vertex_inputs: VertexInputs,
    /// The nine per-vertex output registers, positionally with [`OUTPUT_NAMES`].
    outputs: [Option<u16>; 9],
    /// The eight composite outputs, positionally with [`EXTRA_OUTPUT_NAMES`].
    /// Per-frame only — a vertex has no decay and no gamma.
    extra: [Option<u16>; 8],
    /// The render target's aspect as of the last [`run_frame`](Self::run_frame),
    /// so [`run_vertex`](Self::run_vertex) can compute MilkDrop's `rad`/`ang`
    /// without the caller having to hand it over per vertex.
    aspect: f32,
    /// The register values `per_frame` left, for the registers `per_vertex` can
    /// write. Restored before each vertex (see the module docs).
    snapshot: Vec<f32>,
    /// Monotone frame counter, for the `frame` variable. Reset with the preset.
    frame_index: u32,
    /// Slow envelopes behind `bass_att`/`mid_att`/`treb_att`, which MilkDrop
    /// supplies and this engine's analysis frame does not carry. One-pole on the
    /// injected real `dt`, so they are frame-rate independent like everything
    /// else here.
    att: [f32; 3],
}

/// The time constant of the `*_att` envelopes, in seconds.
///
/// MilkDrop describes them as "an attenuated (smoothed) version" without naming a
/// constant. Half a second is the value that behaves the way presets use them —
/// as a slow floor under a percussive band, so `bass / bass_att` reads as "louder
/// than it has been lately". Short enough to follow a build, long enough not to
/// follow a kick.
const ATT_TAU: f32 = 0.5;

/// What MilkDrop's `bass` reads at an average level.
///
/// Its bands are normalized so ~1.0 is typical and a loud passage reaches 2–3.
/// This engine's are `0..1` against their own recent peak (ADR-0049), where ~0.5
/// is typical. Doubling puts a typical passage at MilkDrop's typical, which is
/// what a preset's thresholds were tuned against.
const BAND_SCALE: f32 = 2.0;

impl MilkRuntime {
    /// Build a runtime for `bundle`, resolving every index and running
    /// `per_frame_init` once.
    ///
    /// `salt` is the preset's (ADR-0051): the pinned twin on every capture path
    /// and the live one in the app, so a bundle using `rand()` is reproducible in
    /// the harness and varied in the app.
    pub fn new(bundle: MilkBundle, salt: u32) -> Self {
        let roster = bundle.roster().to_vec();
        let index = |name: &str| -> Option<u16> {
            roster
                .iter()
                .position(|n| n == name)
                .and_then(|i| u16::try_from(i).ok())
        };
        let inputs = FrameInputs {
            bass: index("bass"),
            mid: index("mid"),
            treb: index("treb"),
            bass_att: index("bass_att"),
            mid_att: index("mid_att"),
            treb_att: index("treb_att"),
            time: index("time"),
            frame: index("frame"),
            fps: index("fps"),
            progress: index("progress"),
            meshx: index("meshx"),
            meshy: index("meshy"),
            aspectx: index("aspectx"),
            aspecty: index("aspecty"),
        };
        let vertex_inputs = VertexInputs {
            x: index("x"),
            y: index("y"),
            rad: index("rad"),
            ang: index("ang"),
        };
        let outputs = std::array::from_fn(|i| OUTPUT_NAMES.get(i).and_then(|n| index(n)));
        let extra = std::array::from_fn(|i| EXTRA_OUTPUT_NAMES.get(i).and_then(|n| index(n)));
        let stack = bundle
            .per_frame_init
            .stack_depth()
            .max(bundle.per_frame.stack_depth())
            .max(bundle.per_vertex.stack_depth());
        let mut state = VmState::new(roster.len(), stack, salt);
        state.accommodate(&bundle.per_frame_init);
        state.accommodate(&bundle.per_frame);
        state.accommodate(&bundle.per_vertex);
        let snapshot = vec![0.0; bundle.per_vertex.written_registers().len()];
        let mut runtime = Self {
            bundle,
            state,
            inputs,
            vertex_inputs,
            outputs,
            extra,
            aspect: 1.0,
            snapshot,
            frame_index: 0,
            att: [0.0; 3],
        };
        runtime.reset();
        runtime
    }

    /// Reset to the state a freshly-loaded preset is in: registers and arenas
    /// zeroed, RNG back at its seed, frame counter at zero, `per_frame_init` run
    /// once.
    ///
    /// **What makes a capture reproducible.** The harness rebuilds a preset from
    /// the top, and everything the previous run left — a `megabuf` a program
    /// filled, an RNG stream it advanced — has to go with it (NFR §6).
    pub fn reset(&mut self) {
        self.state.clear_registers();
        self.state.clear_memory();
        self.state.reset_rng();
        self.frame_index = 0;
        self.att = [0.0; 3];
        vm::run(&self.bundle.per_frame_init, &mut self.state, Budget::INIT);
    }

    /// Whether this bundle has a per-vertex program at all. A bundle without one
    /// drives the mesh from its per-frame outputs alone, which is a perfectly
    /// good MilkDrop preset.
    pub fn has_per_vertex(&self) -> bool {
        !self.bundle.per_vertex.code().is_empty()
    }

    /// Run `per_frame` for this frame and return the whole-mesh outputs, already
    /// converted from MilkDrop's per-frame rates to this engine's per-second ones
    /// (module docs).
    ///
    /// Returns `(outputs, decay)`, positionally with [`OUTPUT_NAMES`]. `decay` is
    /// `None` when the program never names it, so the scene keeps its own default
    /// rather than being handed a zero.
    pub fn run_frame(
        &mut self,
        frame: &crate::dsp::AnalysisFrame,
        time: f32,
        dt: f32,
        mesh: (u32, u32),
        aspect: f32,
    ) -> ([f32; 9], [Option<f32>; 8]) {
        self.aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        // The `*_att` envelopes, on the injected real `dt`.
        let alpha = if dt > 0.0 && dt.is_finite() {
            1.0 - (-dt / ATT_TAU).exp()
        } else {
            0.0
        };
        for (slot, level) in self.att.iter_mut().zip([frame.bass, frame.mid, frame.treb]) {
            *slot += alpha * (level * BAND_SCALE - *slot);
        }

        let set = |state: &mut VmState, slot: Option<u16>, value: f32| {
            if let Some(index) = slot {
                state.set(index, value);
            }
        };
        set(&mut self.state, self.inputs.bass, frame.bass * BAND_SCALE);
        set(&mut self.state, self.inputs.mid, frame.mid * BAND_SCALE);
        set(&mut self.state, self.inputs.treb, frame.treb * BAND_SCALE);
        set(&mut self.state, self.inputs.bass_att, self.att[0]);
        set(&mut self.state, self.inputs.mid_att, self.att[1]);
        set(&mut self.state, self.inputs.treb_att, self.att[2]);
        set(&mut self.state, self.inputs.time, time);
        set(&mut self.state, self.inputs.frame, self.frame_index as f32);
        set(&mut self.state, self.inputs.fps, NOMINAL_FPS);
        // `progress` is "how far through this preset's time slice", which this
        // engine has no equivalent of — presets rotate on a transition rather
        // than on a timer. Zero rather than absent: a preset reading it gets a
        // defined value, and Phase 3's roster note says which of these are
        // supplied rather than guessed.
        set(&mut self.state, self.inputs.progress, 0.0);
        set(&mut self.state, self.inputs.meshx, mesh.0 as f32);
        set(&mut self.state, self.inputs.meshy, mesh.1 as f32);
        // MilkDrop's aspect pair, in its own convention: the LONGER axis reads 1
        // and the shorter one reads the ratio, which is what makes `x * aspectx`
        // an isotropic coordinate.
        let (ax, ay) = if aspect >= 1.0 {
            (1.0, aspect)
        } else {
            (1.0 / aspect.max(1e-4), 1.0)
        };
        set(&mut self.state, self.inputs.aspectx, ax);
        set(&mut self.state, self.inputs.aspecty, ay);

        // The outputs start at the identity, so a program that writes only some
        // of them leaves the rest still rather than at zero.
        for (i, slot) in self.outputs.iter().enumerate() {
            if let Some(index) = *slot {
                self.state.set(index, identity_output(i));
            }
        }
        for (i, slot) in self.extra.iter().enumerate() {
            if let Some(index) = *slot {
                self.state
                    .set(index, EXTRA_DEFAULTS.get(i).copied().unwrap_or(0.0));
            }
        }

        vm::run(&self.bundle.per_frame, &mut self.state, Budget::FRAME);
        self.frame_index = self.frame_index.wrapping_add(1);

        // Snapshot what `per_vertex` can write, so each vertex starts from here.
        for (slot, index) in self
            .snapshot
            .iter_mut()
            .zip(self.bundle.per_vertex.written_registers())
        {
            *slot = self.state.get(*index);
        }

        let raw: [f32; 9] = std::array::from_fn(|i| {
            self.outputs
                .get(i)
                .and_then(|slot| *slot)
                .map_or_else(|| identity_output(i), |index| self.state.get(index))
        });
        let extra: [Option<f32>; 8] = std::array::from_fn(|i| {
            let index = self.extra.get(i).and_then(|slot| *slot)?;
            let value = self.state.get(index);
            // `decay` is a per-FRAME survival factor and becomes a per-second
            // one; the seven after it are a multiplier and six flags, which are
            // not rates and pass through.
            Some(if i == 0 {
                per_second_factor(value)
            } else if value.is_finite() {
                value
            } else {
                EXTRA_DEFAULTS.get(i).copied().unwrap_or(0.0)
            })
        });
        (convert_outputs(raw), extra)
    }

    /// Run `per_vertex` for the vertex at uv `(x, y)` — `y = 0` at the **top**,
    /// which is the reference's own convention — and return its nine outputs,
    /// converted like the per-frame ones.
    ///
    /// `rad` and `ang` are computed here rather than taken, and deliberately:
    /// **MilkDrop normalizes them differently from this engine's native
    /// `[per_vertex]` vocabulary**, and a converted preset has to get MilkDrop's.
    /// The reference takes `rad = |(x_ndc * aspectx, y_ndc * aspecty)|` with the
    /// *longer* axis scaled to 1, so `rad` reaches `1.0` at the middle of the
    /// left and right edges of a wide frame; the native `rad`
    /// ([`warp_mesh::vertex_position`](crate::render::scenes::warp_mesh::vertex_position))
    /// reaches `1.0` at the top and bottom instead. The two differ by a factor of
    /// the aspect, which on a 16:9 display is 1.78 — enough that a preset written
    /// as `zoom = 1 + rad * 0.1` would be most of a stop out. `ang` is the
    /// reference's `atan2` in `-pi..pi`, not the native `0..tau`.
    ///
    /// Restores the per-frame register state first, so a write inside the program
    /// does not leak into the next vertex — MilkDrop's semantics, and the reason
    /// two adjacent vertices of an identical program give identical answers.
    pub fn run_vertex(&mut self, x: f32, y: f32) -> [f32; 9] {
        // Clip-space position, +y up, which is what the reference's `rad`/`ang`
        // are taken from.
        let nx = x * 2.0 - 1.0;
        let ny = 1.0 - y * 2.0;
        let (ax, ay) = if self.aspect >= 1.0 {
            (1.0, 1.0 / self.aspect)
        } else {
            (self.aspect, 1.0)
        };
        let (px, py) = (nx * ax, ny * ay);
        let rad = (px * px + py * py).sqrt();
        let ang = py.atan2(px);
        for (value, index) in self
            .snapshot
            .iter()
            .zip(self.bundle.per_vertex.written_registers())
        {
            self.state.set(*index, *value);
        }
        if let Some(index) = self.vertex_inputs.x {
            self.state.set(index, x);
        }
        if let Some(index) = self.vertex_inputs.y {
            self.state.set(index, y);
        }
        if let Some(index) = self.vertex_inputs.rad {
            self.state.set(index, rad);
        }
        if let Some(index) = self.vertex_inputs.ang {
            self.state.set(index, ang);
        }
        vm::run(&self.bundle.per_vertex, &mut self.state, Budget::VERTEX);
        let raw: [f32; 9] = std::array::from_fn(|i| {
            self.outputs
                .get(i)
                .and_then(|slot| *slot)
                .map_or_else(|| identity_output(i), |index| self.state.get(index))
        });
        convert_outputs(raw)
    }
}

/// Output `i`'s identity value — what the register holds before a program runs,
/// so a program that never writes it leaves the past still.
fn identity_output(i: usize) -> f32 {
    match OUTPUT_NAMES.get(i) {
        Some(&"zoom") | Some(&"sx") | Some(&"sy") => 1.0,
        Some(&"cx") | Some(&"cy") => 0.5,
        _ => 0.0,
    }
}

/// The widest a converted factor may get, and its reciprocal the narrowest.
///
/// Raising to [`NOMINAL_FPS`] is a thirtieth power, so it **overflows `f32` at a
/// per-frame factor of about 13** — and an overflow that fell back to `1.0` would
/// turn the most extreme zoom a preset can ask for into no zoom at all, which is
/// the opposite of what it says. Saturating instead keeps the direction: a
/// runaway zoom collapses the source window to a point, which is what a runaway
/// zoom looks like. Wide enough that no plausible preset reaches it (`1.05` per
/// frame, a brisk drift, is `4.3` per second).
const MAX_FACTOR: f32 = 1.0e30;

/// A per-frame survival/scale factor as a per-second one, at [`NOMINAL_FPS`].
///
/// `v^fps`: thirty frames of `0.96` is `0.96^30` per second at the nominal rate.
/// Total on a non-finite or non-positive input, which a program can produce — a
/// factor at or below zero is not a factor, so it reads as the identity rather
/// than as a mirror.
fn per_second_factor(v: f32) -> f32 {
    if !v.is_finite() || v <= 0.0 {
        return 1.0;
    }
    let out = v.powf(NOMINAL_FPS);
    if out.is_finite() {
        out.clamp(1.0 / MAX_FACTOR, MAX_FACTOR)
    } else if v > 1.0 {
        MAX_FACTOR
    } else {
        1.0 / MAX_FACTOR
    }
}

/// The nine raw MilkDrop outputs as this engine's per-second vocabulary.
fn convert_outputs(raw: [f32; 9]) -> [f32; 9] {
    std::array::from_fn(|i| {
        let v = raw.get(i).copied().unwrap_or(0.0);
        if OUTPUT_FACTOR.get(i).copied().unwrap_or(false) {
            per_second_factor(v)
        } else if OUTPUT_RATE.get(i).copied().unwrap_or(false) {
            let out = v * NOMINAL_FPS;
            if out.is_finite() { out } else { 0.0 }
        } else if v.is_finite() {
            v
        } else {
            identity_output(i)
        }
    })
}

#[cfg(test)]
mod tests;

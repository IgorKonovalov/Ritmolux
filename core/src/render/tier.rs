//! Quality tiers: the engine's capacity constants, resolved once (ADR-0045).
//!
//! NFR §1 specifies two quality levels — a reduced tier holding 60 fps
//! at 1080p on the ~2015-iGPU baseline, and a richer presentation on
//! capable hardware. This module is where both live.
//!
//! # What a tier is
//!
//! A [`TierConfig`] is a plain struct of **capacity** values: how many particles,
//! how many segments, how large an internal grid may get. Nothing here changes
//! *what* the engine draws, only how much of it — which is what makes
//! [`Tier::Floor`] byte-identical to the pre-tier engine and what lets captures
//! pin it (see below). A value that changes the *content* of a frame — the
//! reaction-diffusion simulation grid, whose pattern scale moves with its
//! resolution (ADR-0034) — deliberately does **not** live here.
//!
//! **That separation is a property of the consuming scene, not of this
//! struct.** The attractor draws its particles with an *additive* blend
//! into a linear accumulation, so
//! [`attractor_particles`](TierConfig::attractor_particles) sets the
//! total light in the frame as directly as it sets the sample count —
//! `Rich` rendered every attractor preset three stops hot behind a
//! green suite, because no capture pins `Rich`. What holds the claim up
//! is [`deposit_scale`](super::scenes::particles::deposit_scale)
//! dividing the deposit by the count (ADR-0065). The lesson
//! generalizes: **a count feeding an accumulating pass is a look value
//! until something normalizes it.** If a future field lands here for
//! such a pass, that normalization is part of adding it.
//!
//! # Where the numbers come from
//!
//! [`TierConfig::FLOOR`] is the pre-tier engine, constant for constant: each
//! value's former definition site now reads this struct, so no number exists
//! twice. Its justifications came with it and are on the fields.
//! [`TierConfig::RICH`] is calibrated against a midrange discrete GPU
//! (RTX 3060 / RX 6600 class) on device — Plan 0044 Phase 4 — rather than
//! asserted from a multiplier.
//!
//! # Resolution and the governor
//!
//! The tier resolves **once, at renderer construction**, from an optional pin
//! ([`RendererOptions`](super::RendererOptions)); unpinned resolves [`Tier::Rich`].
//! An unpinned renderer may then be demoted to [`Tier::Floor`] by the frame-time
//! governor — one way, once per session, never silently. A pinned tier never
//! moves.
//!
//! Headless capture is [`Tier::Floor`] **by construction**:
//! [`Renderer::new_headless`](super::Renderer::new_headless) cannot produce any
//! other tier, so every golden baseline stays byte-reproducible on the WARP
//! software adapter and the suite's cost does not scale with the rich tier.
//! [`Renderer::new_headless_tiered`](super::Renderer::new_headless_tiered) is the
//! deliberate opt-in the `shot` CLI's `--tier` reaches.
//!
//! Pure and GPU-free throughout — a tier is a set of numbers, so it is decided
//! without a device and tested without one.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The governor runs once per displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// Which quality tier a renderer is running (ADR-0045).
///
/// Two named levels rather than a continuum: the output of a preset has to be
/// predictable enough to baseline, document, and reproduce in a bug report, which
/// a load-history-dependent feature-shedding scheme cannot deliver (ADR-0045
/// Alternative B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tier {
    /// The NFR §1/§2 iGPU floor — the pre-tier engine's exact constants. The
    /// default here because it is the safe answer: a `Tier` value that appeared
    /// from nowhere should not raise anyone's budgets.
    #[default]
    Floor,
    /// Calibrated for a midrange discrete GPU: higher particle, segment and
    /// resolution budgets, same visual grammar.
    Rich,
}

impl Tier {
    /// The lowercase name the CLI, the env var and the config file all use.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Floor => "floor",
            Tier::Rich => "rich",
        }
    }

    /// The uppercase name the 5x7 diagnostics overlay paints. Separate from
    /// [`as_str`](Self::as_str) only because that font has no lowercase glyphs;
    /// both come off the same match so there is no second spelling to drift.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Floor => "FLOOR",
            Tier::Rich => "RICH",
        }
    }

    /// Parse a tier name, case-insensitively. `None` for anything else — callers
    /// surface that as a usage error rather than guessing a tier.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "floor" => Some(Tier::Floor),
            "rich" => Some(Tier::Rich),
            _ => None,
        }
    }
}

/// The capacity values a tier sets. Resolved once at renderer construction and
/// read at construction/reconfigure time only — never branched on per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierConfig {
    /// Which tier these values are, so a demotion and the overlay have one thing
    /// to read rather than a parallel field to keep in step.
    pub tier: Tier,

    /// Cap on a post stage's internal grid (ADR-0034), width then height.
    ///
    /// The floor value is NFR §12 memory arithmetic, **redone for the linear-light
    /// composite** (Plan 0045 Phase 3 / ADR-0046). Every intermediate upstream of
    /// the tonemap is now `COMPOSITE_FORMAT` — 8
    /// bytes/texel, not 4 — so a stage offscreen costs twice what the surface
    /// format would charge, while the trails accumulation
    /// (`PingPongField`, two textures) was
    /// already float and did not move.
    ///
    /// Per chain, both stages live, at this cap (1920x1080, 8 bytes/texel =
    /// 16.6 MB a texture):
    ///
    /// | buffer                     | before | after |
    /// |----------------------------|--------|-------|
    /// | trails composited          | 8.3    | 16.6  |
    /// | trails accumulation (x2)   | 33.2   | 33.2  |
    /// | kaleidoscope source        | 8.3    | 16.6  |
    /// | **per chain**              | **50** | **66** |
    ///
    /// Plan 0023's dual-live dissolve holds two whole `PostChain`s, so the peak is
    /// ~133 MB rather than ~100. Outside the chains the frame carries one more
    /// surface-sized float buffer beyond the chains — the tonemap's input, 16.6
    /// MB, the one allocation ADR-0046 genuinely adds — plus the blend's
    /// snapshot/live pair at 16.6 MB each while a dissolve runs (8.3 at 8-bit),
    /// and ink's 8.3 MB input, which stays 8-bit because the tonemap hands it
    /// display-referred pixels. Worst case — dual-live, both stages, ink on — is
    /// ~191 MB against NFR §12's ~350 MB soft ceiling, which is mostly driver
    /// floor already.
    ///
    /// At the rich cap (2560x1440) the same arithmetic is ~118 MB per chain and
    /// ~236 MB dual-live, up from ~88 and ~177 — the trade ADR-0034 priced and
    /// declined at floor budgets and the rich tier takes.
    ///
    /// **This cap is the relief lever** if the float chain misses NFR §1 on a
    /// floor-tier iGPU: lower it rather than re-fixing the grids (ADR-0046), since
    /// bandwidth roughly doubled with the format and the grid policy is shared.
    ///
    /// **Bloom adds to this only when a preset switches it on** (Plan 0045
    /// Phase 4), and it is **two** allocations, not one. Its pyramid is two
    /// textures per level, each level a quarter of the last, so the pyramid
    /// converges to `2 * (1/4 + 1/16 + …) ≈ 2/3` of one grid-sized texture — ~11 MB
    /// at this cap. On top of that the stage owns its own **grid-sized `bloom-src`
    /// offscreen**, a full 16.6 MB at this cap, because a `PostStage` reads its
    /// input from a texture it owns. So the stage costs **16.6 + ~11 ≈ 28 MB** on
    /// top of the ~66 MB per chain above, and ~55 MB in the dual-live worst case —
    /// which is what NFR §12's table charges and what the ~246 MB worst case there
    /// is computed from. It is charged only against presets that bind
    /// `bloom_amount`, since an inactive stage builds nothing.
    pub post_cap: (u32, u32),

    /// How many levels deep the bloom pyramid goes
    /// (`Bloom`, ADR-0046).
    ///
    /// This is a **capacity**, not a look: each level doubles the halo's reach and
    /// costs three passes at a quarter of the previous level's area, so the tail
    /// is cheap in pixels and not free in passes. The floor runs four (a halo
    /// reaching ~16 of the grid's texels at the default radius); rich runs six,
    /// which is where the widest levels start to matter on a 1440p-class grid.
    ///
    /// `level_sizes` clamps this down on a small render target, so
    /// a value here is an upper bound rather than a promise.
    pub bloom_levels: u32,

    /// The attractor's sample budget **at [`REFERENCE_PX`]** — the anchor of the
    /// density law, not the count drawn (ADR-0140).
    ///
    /// [`attractor_budget`] scales this by `target_px / REFERENCE_PX` and clamps
    /// it between this value and one of the two ceilings below, so a target at or
    /// under the reference draws exactly this many and a larger one draws more.
    /// What a *preset* then draws out of that budget is
    /// `round(budget * density)` (ADR-0069), which is a different and smaller
    /// number again.
    ///
    /// State is 48 bytes each ([`Particle`](super::scenes::particles)), and the
    /// real ceiling is **additive-blend fill rate**, which is why the floor value
    /// was described as the number to validate against the 60 fps @ 1080p floor
    /// (ADR-0015 Risks).
    ///
    /// This is a sample count and **not** a brightness: the additive draw divides
    /// its deposit by the *active* count
    /// ([`deposit_scale`](super::scenes::particles::deposit_scale), ADR-0065), so
    /// raising it buys a smoother figure rather than a brighter one. Changing it
    /// changes shot noise and cost; it does not change exposure.
    pub attractor_particles: u32,

    /// The largest budget [`attractor_budget`] may resolve for a scene drawing
    /// into a **live surface** — a window, or the plugin's host surface.
    ///
    /// Frame-time bound, and it is the number that keeps the law from spending a
    /// display's whole budget on sample count. It is also the **allocation**: the
    /// particle buffer is sized here at construction and never resized, so a
    /// resize changes the active count and rebuilds no GPU resource. That costs
    /// `ceiling * 48 B` of GPU storage plus the same again for the CPU seed
    /// scatter the scene holds for re-upload, in **every** window, whether or not
    /// the target is large and whether or not an attractor preset is loaded
    /// (`create_all` builds every scene up front).
    ///
    /// # Where these numbers come from
    ///
    /// Measured, not chosen — Plan 0128 Phase 1, at 1920x1080 on
    /// `attractor_leviathan`, counts interleaved in one process so a throttling
    /// laptop GPU could not read as signal.
    ///
    /// **`Rich`: 600 000**, four times the anchor. On the midrange-discrete
    /// reference NFR §1 calibrates `Rich` against, the rule was *the largest swept
    /// count whose marginal p99 over today's anchor stays inside 10 % of the
    /// 16.67 ms budget*: 600 000 reads +1.454 ms (8.7 %) and the next step up,
    /// 1 200 000, reads +4.703 ms (28.2 %).
    ///
    /// **`Floor`: its own anchor**, so the law is a no-op there. On integrated
    /// hardware — the baseline NFR §1's floor commitment is about — 1080p at
    /// `Floor` already sits *on* the 16.67 ms budget at today's 50 000 (p99
    /// 16.854 ms), and the law's own 1080p `Floor` value of 450 000 takes it to
    /// 31.942 ms. NFR §1 promises `Floor` "values exactly the pre-tier engine's";
    /// this is what keeps that true at every target size.
    pub attractor_particles_live_ceiling: u32,

    /// The same ceiling for a **headless render** — `shot --render`, where there
    /// is no present deadline and no governor, and the only bound is memory.
    ///
    /// # Where these numbers come from
    ///
    /// The bound is the **device's storage-buffer binding limit**, not process
    /// memory, because it is reached first: at 48 B a particle, wgpu's default
    /// `max_storage_buffer_binding_size` of 134 217 728 B holds 2 796 202
    /// particles, and 5 400 000 — the law's own unclamped 4K value — fails
    /// outright with `Buffer binding 0 range 259200000 exceeds
    /// max_*_buffer_binding_size limit 134217728` (Plan 0128 Phase 1).
    ///
    /// `Rich` takes **2 700 000**, the largest whole multiple of its anchor under
    /// that wall — 18x, 129.6 MB, 3.4 % of headroom. `Floor` takes the **same
    /// multiple** rather than the same number, so a tier still means something
    /// offline: at one shared ceiling `--tier floor --render` and
    /// `--tier rich --render` would draw an identical count at 4K.
    pub attractor_particles_offline_ceiling: u32,

    /// Upper bound on each axis of the attractor's trail accumulation grid.
    ///
    /// The floor is the ceiling Plan 0027/0029 chose for a high-DPI display while
    /// keeping the worst case bounded on the iGPU: every frame pays a decay pass
    /// plus the full additive instance draw over this grid, so cost scales with
    /// its area. Rich lifts it to 4K so a 4K or ultrawide display sizes near 1:1
    /// instead of degrading to a uniform upscale.
    pub attractor_trail_cap: (u32, u32),

    /// How many particles the swarm simulates.
    ///
    /// Plan 0043 left the floor value at an **unmeasured** iGPU cost (+0.5 ms per
    /// frame of depth math on the dev box, and not fill rate), which is why the
    /// plans index calls this a live tier candidate rather than a settled
    /// constant. If the on-device floor check misses, this is the lever — on the
    /// floor value, and that routes back through `architect`.
    pub swarm_particles: usize,

    /// How many objects the emitter's pool holds (ADR-0057).
    ///
    /// Unlike every other count here this is a **ceiling on a varying
    /// population**, not the population: the emitter spawns and retires, so a
    /// preset's `spawn_rate * lifetime` decides how many objects are actually
    /// alive and this decides how many *can* be. Spawns past it are dropped
    /// rather than queued or allocated for — that is the phase's whole real-time
    /// hazard — so raising it does not brighten a preset that never reaches it,
    /// and lowering it below one that does thins the shower rather than changing
    /// its motion.
    ///
    /// Not an accumulating count in the sense the module docs warn about: each
    /// object is one sprite drawn once per frame, so the light in the frame is
    /// `population * brightness` and the population is a preset's own arithmetic.
    /// The tier only says where that arithmetic is cut off.
    ///
    /// The floor holds a shipped preset's shower with room to spare (the emitter
    /// family runs a few hundred objects live); rich triples it for the denser
    /// looks a discrete GPU can carry. Cheap either way — see NFR §12: the pool
    /// and its instance buffer are well under a megabyte at both tiers.
    pub emitter_objects: usize,

    /// Ceiling on the warp mesh's grid, in **cells**, width then height
    /// (Plan 0100 Phase 1).
    ///
    /// A capacity in the strictest sense: the grid is a *resolution* for the
    /// per-vertex program, not a shape (ADR-0037), so raising it refines the
    /// warp's spatial detail and changes nothing about what the scene draws. The
    /// vertex count is `(x + 1) * (y + 1)`, and every one of those vertices costs
    /// one evaluation of each `[per_vertex]` binding **on the render thread**,
    /// which is what this bounds.
    ///
    /// The upper bound either tier may name is the `.milk` format's own —
    /// `meshx <= 128`, `meshy <= 96` — so a converted preset's requested grid is
    /// representable at the top of the range and clamped below it.
    /// [`warp_mesh::clamp_grid`](super::scenes::warp_mesh::clamp_grid) is the one
    /// place the clamp happens, shared by the loader and the scene.
    ///
    /// # Where these numbers come from
    ///
    /// **Measured, not chosen** — Plan 0100 Phase 1's done-when. The rule it set
    /// was: raise the grid until one frame of per-vertex evaluation costs more
    /// than **1 ms** — 6 % of the 16.67 ms NFR §1 commits to at 1080p — and cap
    /// the floor one step below.
    ///
    /// `mesh_cost_by_grid` in `scenes/warp_mesh/tests.rs` is the measurement and
    /// prints the ladder on every run. Taken **2026-08-16 on the development box
    /// (Windows 10, desktop CPU, `--release`)**, evaluating a four-binding
    /// `[per_vertex]` program of the shape a real preset writes — two runs,
    /// agreeing to about 1 %:
    ///
    /// ```text
    ///  grid      vertices    per frame     share of 16.67 ms
    ///  16x12        221       0.036 ms      0.2 %
    ///  32x24        825       0.129 ms      0.8 %
    ///  48x36      1 813       0.280 ms      1.7 %
    ///  64x48      3 185       0.488 ms      2.9 %   <- Floor
    ///  72x54      4 015       0.616 ms      3.7 %
    ///  80x60      4 941       0.755 ms      4.5 %
    ///  88x66      5 963       0.909 ms      5.5 %   <- Rich
    ///  96x72      7 081       1.081 ms      6.5 %   <- the bar is crossed here
    /// 112x84      9 605       1.483 ms      8.9 %
    /// 128x96     12 513       1.918 ms     11.5 %
    /// ```
    ///
    /// **The bar is crossed between `88x66` and `96x72`**, so `88x66` is the
    /// largest grid the rule admits and it is what `Rich` takes. The format's own
    /// maximum is therefore **refused**: at 1.92 ms it is 11.5 % of the frame on
    /// a desktop CPU, which is not a number any tier should spend on one
    /// parameter surface. The grid is lowered because it did not measure clean,
    /// which is the whole of the rule.
    ///
    /// **`Floor` sits a step further down than the rule alone would put it, and
    /// deliberately.** The rig above is a desktop CPU; NFR §1's floor tier
    /// targets a ~2015 iGPU-class machine whose single-thread performance this
    /// box does not model, and this is CPU work on the render thread, so a
    /// slower machine pays proportionally more of a budget it is already
    /// struggling to hold. `64x48` is 2.9 % here and leaves room for that
    /// machine to be several times slower before the surface is a problem.
    ///
    /// **When the floor tier is next exercised on real target hardware this is
    /// the constant to re-measure**, and the ladder prints exactly what that
    /// needs.
    pub mesh_grid: (u32, u32),

    /// The one capacity value here that a preset can *see*: past it geometry is
    /// truncated, and ADR-0007 requires that be surfaced rather than silently cut.
    /// So a preset whose mirror pushes over the floor cap reports an overflow at
    /// the floor and not at rich — the message is the tier's most visible edge for
    /// the content lane, which is why shipped presets are authored against the
    /// floor.
    pub max_segments: usize,

    /// Cap on how many flat elements a `shape_collage` canvas may hold
    /// (ADR-0123).
    ///
    /// **The one capacity here that bounds a per-pixel loop**, which is what
    /// makes it load-bearing rather than a memory number. Every other count in
    /// this struct bounds work paid once per particle, per vertex or per
    /// segment; this one bounds work every *fragment* pays, so a frame costs
    /// `elements x pixels` and ADR-0123 prices the bounding-box reject alone —
    /// before anything is drawn — at roughly `6N` operations per pixel. The
    /// buffer is irrelevant at any value either tier would take: 64 bytes an
    /// element, so 128 elements is 8 KB.
    ///
    /// # Where the floor value comes from
    ///
    /// **Measured, then decided by a human** — Plan 0113 Phases 2 and 3.
    /// `core/tests/collage_cost.rs` sweeps the count on hardware, prints
    /// the ladder on every run, and its module docs own the readings and
    /// the trap in quoting them. **Two tables are not interchangeable**:
    /// the pre-roster ladder is what the Phase 3 gate read, and the
    /// post-roster table is what a canvas costs today — the eight-kind
    /// roster made the loop cheaper, because rings, sectors and checker
    /// patches shade far less of their own bounding box than a quad
    /// does.
    ///
    /// **40 is the reference set's own top, not a budget line.** The
    /// gate was a look judgement and the cost was not the binding
    /// constraint: the user's working density is **8 to 14 elements**,
    /// which on the system as shipped costs **8.2 % of a 60 Hz frame at
    /// eight and 10.7 % at sixteen**, and denser canvases were rejected
    /// on sight long before they were rejected on cost. The ceiling is
    /// the densest canvas in ADR-0123's roster, Kandinsky's *On White
    /// II*, counted at just above 40 once its lines and arcs are
    /// included — so this value sits **exactly on** that canvas, and a
    /// `collage_onwhite` needing a forty-first element moves this
    /// number rather than being quietly truncated.
    ///
    /// `Rich` is provisional in the sense every [`RICH`](TierConfig::RICH) value
    /// is — see that constant's own note.
    ///
    /// # It clamps, and it does not yet say so
    ///
    /// `shape_collage::applied_count` holds a bound `count` to this value
    /// **silently**, unlike [`max_segments`](Self::max_segments), which
    /// ADR-0007 requires surface an overflow. That was harmless while the cap
    /// sat far above any authored canvas and is not harmless now that it sits on
    /// one. Recorded as a followup on Plan 0113 rather than fixed there: the
    /// surfaced channel is [`CapOverflow`](super::scenes::lines::CapOverflow),
    /// whose context enum is shared with the line scenes, so widening it is an
    /// architect call.
    pub collage_elements: usize,
}

impl TierConfig {
    /// The iGPU floor: the pre-tier engine's constants, unchanged.
    pub const FLOOR: Self = Self {
        tier: Tier::Floor,
        post_cap: (1920, 1080),
        bloom_levels: 4,
        attractor_particles: 50_000,
        attractor_particles_live_ceiling: 50_000,
        attractor_particles_offline_ceiling: 900_000,
        attractor_trail_cap: (2560, 1440),
        swarm_particles: 10_000,
        emitter_objects: 2_000,
        mesh_grid: (64, 48),
        max_segments: 20_000,
        collage_elements: 40,
    };

    /// The midrange-discrete tier.
    ///
    /// **These are provisional multipliers, not measurements.** Plan 0044 Phase 4
    /// runs the standalone pinned here on the target GPU at native fullscreen
    /// across the heaviest preset of each family and records the frame times; the
    /// values that ship are the ones that hold the display rate. A number that
    /// misses gets lowered, and no number here is invented upward to look good.
    /// Until that phase closes, treat every field below as a starting point.
    pub const RICH: Self = Self {
        tier: Tier::Rich,
        post_cap: (2560, 1440),
        bloom_levels: 6,
        attractor_particles: 150_000,
        attractor_particles_live_ceiling: 600_000,
        attractor_particles_offline_ceiling: 2_700_000,
        attractor_trail_cap: (3840, 2160),
        swarm_particles: 30_000,
        emitter_objects: 6_000,
        mesh_grid: (88, 66),
        max_segments: 60_000,
        collage_elements: 96,
    };

    /// The config for `tier`.
    pub const fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Floor => Self::FLOOR,
            Tier::Rich => Self::RICH,
        }
    }
}

impl Default for TierConfig {
    fn default() -> Self {
        Self::FLOOR
    }
}

// ---------------------------------------------------------------------------
// The attractor's sample budget (ADR-0140)
// ---------------------------------------------------------------------------

/// The render target the attractor's sample density is anchored to: 640x360,
/// 230 400 pixels.
///
/// The attractor's trail grid is surface-sized, so its deposit spreads over
/// whatever the target holds and a flat count therefore *falls* in density as the
/// target grows — 0.651 particles per pixel per frame here, 0.072 at 1080p, which
/// is the whole of the "it just looks like Leviathan upscaled" verdict. This is
/// the size whose density is already accepted, so it is where
/// [`attractor_budget`] resolves to exactly the tier's own anchor.
///
/// **The denominator is target pixels, not grid texels**, and the two differ by
/// the grid's 256-px quantization — 1.71x here, 1.07x at 720p, 1.26x at 1080p,
/// 1.00x at 4K. Bounded, and named because the deposit lands per texel.
pub const REFERENCE_PX: u32 = 230_400;

/// The attractor's drawn sample budget for a target of `target_px` pixels, before
/// a preset's `[particles] density` narrows it further (ADR-0140).
///
/// `clamp(round(anchor * target_px / REFERENCE_PX), anchor, ceiling)`.
///
/// **The lower clamp is load-bearing.** The law can only ever *add* samples above
/// [`REFERENCE_PX`], never remove them below it, so every existing capture — the
/// 128x128 golden suite, the 96x96 sanity suite, every small `shot` still —
/// resolves to exactly the count it resolved before this function existed and
/// stays byte-identical. That is assertable on the value rather than inferred
/// from pixels, which is the same shape of argument ADR-0065 used for
/// `deposit_scale` being exactly `1.0` at `Floor`.
///
/// `f64` throughout: `anchor * target_px` reaches 41 bits at a 4K target, past
/// `f32`'s 24-bit mantissa, so the product would be rounded before the divide.
///
/// A `ceiling` below `anchor` is raised to it rather than inverting the clamp —
/// `u32::clamp` panics when `min > max`, and this runs on the resize path.
pub fn attractor_budget(anchor: u32, target_px: u32, ceiling: u32) -> u32 {
    let scaled = (f64::from(anchor) * f64::from(target_px) / f64::from(REFERENCE_PX)).round();
    let scaled = if scaled >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        scaled as u32
    };
    scaled.clamp(anchor, ceiling.max(anchor))
}

impl TierConfig {
    /// [`attractor_budget`] against this tier's **live** ceiling — what a window
    /// and the plugin's host surface resolve.
    pub fn attractor_budget_live(&self, target_px: u32) -> u32 {
        attractor_budget(
            self.attractor_particles,
            target_px,
            self.attractor_particles_live_ceiling,
        )
    }

    /// [`attractor_budget`] against this tier's **offline** ceiling — what a
    /// headless render resolves, where the bound is memory rather than frame time.
    pub fn attractor_budget_offline(&self, target_px: u32) -> u32 {
        attractor_budget(
            self.attractor_particles,
            target_px,
            self.attractor_particles_offline_ceiling,
        )
    }
}

// ---------------------------------------------------------------------------
// The frame-time governor (Plan 0044 Phase 2)
// ---------------------------------------------------------------------------

/// The display budget assumed when the frontend has not named a refresh rate —
/// 60 Hz, the rate NFR §1's floor is quoted at.
pub const DEFAULT_DISPLAY_HZ: f32 = 60.0;

/// How far past the display budget a single frame must run to count as a miss.
///
/// Not 1.0. A frame landing a hair over the budget is the ordinary condition of a
/// vsynced renderer — the measured interval *is* the refresh interval, plus
/// scheduling noise — so a bare comparison would read a perfectly healthy 60 fps
/// run as missing on half its frames. 1.25 is "missing the budget by a quarter",
/// which at 60 Hz is 20.8 ms: past it the run is visibly not holding the rate.
pub const MISS_FACTOR: f32 = 1.25;

/// What fraction of the observed frames must be misses before the governor
/// demotes. Three quarters: high enough that an intermittently-heavy passage
/// rides through, low enough that a genuine overload does not have to be
/// unanimous (a demotion is triggered by *sustained* pressure, and a real
/// overload still has fast frames in it — a cheap preset in the rotation, a
/// dissolve that ended).
pub const MISS_FRACTION: f32 = 0.75;

/// Frames of history required before the governor will demote at all.
///
/// This is the hysteresis, and it is a *count* rather than a smoothing constant
/// on purpose: it makes "a single spike must not demote" true by arithmetic
/// instead of by tuning. With 180 frames required and 75 % of them needing to
/// miss, no run of fewer than 135 consecutive bad frames can demote — so a window
/// drag, a driver hiccup, or a shader compile cannot, whatever their magnitude.
/// At 60 Hz it is also a 3 s warm-up, which keeps the pathological first frames
/// of a session (pipeline creation, first-use resource builds) out of the verdict.
///
/// **This number is only satisfiable because of a constant in another module.**
/// The only series the renderer ever hands [`sustained_miss`] is
/// [`FrameStats::samples`](crate::diag::FrameStats::samples), which yields at
/// most `crate::diag::RING` items — so a `MIN_SAMPLES` above the ring's
/// capacity makes the governor a permanent no-op. See the assertion below.
pub const MIN_SAMPLES: usize = 180;

/// The governor must be able to *reach* its own threshold from its real input.
///
/// A build failure rather than a test, because the failure it guards is silent:
/// nothing observable happens when the governor stops demoting — a machine that
/// cannot hold the rich budget simply stutters for the rest of the session, which
/// is the exact NFR §1 outcome ADR-0045 built the governor to prevent. A runtime
/// check could not fire (there is nothing to fire *on*), and a unit test over an
/// injected series cannot see this at all: the series in the tests below are
/// `Vec`s of any length we like, so they would keep passing while the real
/// producer had gone too short to ever trigger a demotion.
const _: () = assert!(
    MIN_SAMPLES <= crate::diag::RING,
    "the frame-time ring is shorter than the governor's minimum sample count, \
     so the governor can never demote"
);

/// Whether a frame-time series shows a **sustained** miss of the display budget,
/// which is the one condition that demotes [`Tier::Rich`] to [`Tier::Floor`]
/// (ADR-0045).
///
/// Pure and total: a function of the series and the budget with no clock, no
/// state and no allocation, so the policy is unit-testable against injected
/// series (which is how the spike-versus-overload distinction above is checked
/// rather than asserted). `frame_secs` is the rolling history in **seconds**, the
/// unit [`FrameStats::samples`](crate::diag::FrameStats::samples) yields; order
/// does not matter, only the counts.
///
/// Says `false` for a non-positive or non-finite budget, and for a series shorter
/// than [`MIN_SAMPLES`] — the safe direction, since a wrong `true` costs the user
/// the rich tier for the rest of the session and a wrong `false` costs nothing but
/// another second of measurement.
pub fn sustained_miss(frame_secs: impl Iterator<Item = f32>, budget_secs: f32) -> bool {
    if !budget_secs.is_finite() || budget_secs <= 0.0 {
        return false;
    }
    let threshold = budget_secs * MISS_FACTOR;
    let mut total = 0usize;
    let mut missed = 0usize;
    for dt in frame_secs {
        total += 1;
        if dt.is_finite() && dt > threshold {
            missed += 1;
        }
    }
    total >= MIN_SAMPLES && missed as f32 >= total as f32 * MISS_FRACTION
}

/// **The governor's whole decision**: whether to demote `tier` right now.
///
/// Pure — every input is a value, so the three properties ADR-0045 asks of the
/// governor are unit-testable together rather than one being a fact about
/// `Renderer`'s field layout: a pin never demotes, an already-demoted session
/// never demotes again (the one-way latch), and only a sustained miss demotes.
///
/// The caller owns the latch: it sets its "demoted" flag and rebuilds when this
/// says `true`, and passes that flag back in on every later frame. Keeping the
/// flag out here is what makes "exactly once" a property of the decision instead
/// of a property of the call site.
pub fn should_demote(
    tier: Tier,
    pinned: bool,
    already_demoted: bool,
    frame_secs: impl Iterator<Item = f32>,
    budget_secs: f32,
) -> bool {
    // Ordered cheapest-first: three flag reads settle the steady state, and the
    // series is only walked on a governed rich session that has not yet demoted.
    if pinned || already_demoted || tier == Tier::Floor {
        return false;
    }
    sustained_miss(frame_secs, budget_secs)
}

/// **Whether a runtime tier change is allowed at all** (ADR-0054): only on a
/// context that has a surface.
///
/// A surface-less context is exactly the headless capture path, and ADR-0045's
/// guarantee is that a capture is `Tier::Floor` **by construction** —
/// `Renderer::new_headless` takes no tier argument, so no baseline can be blessed
/// at another tier by forgetting a field. `Renderer::set_tier` is a public
/// mutator on the very type the golden suite renders through, so it is the one
/// hole that guarantee was shaped to exclude, and this predicate is what keeps it
/// closed.
///
/// Pure, and separate from `set_tier`, deliberately. A `Renderer` **with** a
/// surface cannot be constructed in CI — there is no window — so a test that only
/// observed the headless no-op would pass equally well against a `set_tier` that
/// did nothing at all. Expressed as a value-in/value-out function, both
/// directions are assertable.
pub fn tier_change_permitted(has_surface: bool) -> bool {
    has_surface
}

/// The frame budget for a display running at `hz`, in seconds. Falls back to
/// [`DEFAULT_DISPLAY_HZ`] for a value that is not a usable rate, so a frontend
/// that cannot read its monitor still gets a governed session rather than an
/// ungoverned one.
pub fn budget_secs(hz: f32) -> f32 {
    let hz = if hz.is_finite() && hz > 0.0 {
        hz
    } else {
        DEFAULT_DISPLAY_HZ
    };
    1.0 / hz
}

#[cfg(test)]
mod tests;

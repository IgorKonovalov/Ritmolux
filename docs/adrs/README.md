# Architecture Decision Records

Numbered, append-only records of decisions that have a rejected alternative worth
remembering. Accepted ADRs are never edited in place — to change a decision, write a new
ADR that supersedes the old one and update the status here.

Rule of thumb: if you can't name an option you're *not* taking, you don't need an ADR —
you need a code comment.

**Next free number: 0118**

An index row is a pointer, not an abstract: the link, the title as the ADR body's `H1` writes it,
and the status. `scripts/check-index-rows.mjs` holds every row below to 320 bytes
([ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)).

<!-- roster:begin cap=320 -->

| ADR  | Title                                                      | Status   |
|------|------------------------------------------------------------|----------|
| [0001](0001-rust-core-wgpu-cabi-foobar-shim.md) | Rust core, wgpu rendering, C ABI with a C++ foobar shim | accepted |
| [0002](0002-layered-preset-architecture.md) | Layered preset architecture: data + expressions + optional script | accepted (supplemented by 0020) |
| [0003](0003-c-abi-v1-surface.md) | C ABI v1 surface | accepted (extended by 0006, 0008, 0013 — the shipped surface is **v4**) |
| [0004](0004-living-behavioral-spec-layer.md) | Living behavioral-spec layer: seed two contracts, no gate, no ritual yet | accepted |
| [0005](0005-versioning-and-release-cadence.md) | App versioning: SemVer 0.x, one workspace-inherited version, cargo-release as the single bump authority run at plan close | accepted |
| [0006](0006-c-abi-v2-preset-loading.md) | C ABI v2: add a preset-loading entry point | accepted |
| [0007](0007-line-geometry-generators.md) | Line-geometry generators: a cached-build built-in category + instanced-quad line rendering | accepted |
| [0008](0008-c-abi-v3-diagnostics.md) | C ABI v3: diagnostics query + debug-overlay toggle | accepted |
| [0009](0009-glyphon-text-rendering.md) | Adopt glyphon for standalone on-canvas text (feature-gated) | accepted |
| [0010](0010-accept-gpu-driver-memory-floor.md) | Accept the DX12/wgpu driver-stack memory floor; retarget the runtime-memory NFR | accepted |
| [0011](0011-image-crate-for-capture-tooling.md) | Use the `image` crate (dev-dependency only) for headless-capture PNG I/O and golden compare | accepted |
| [0012](0012-stateful-feedback-render-system.md) | Stateful feedback render system: ping-pong offscreen simulation with a fixed-timestep accumulator (Gray-Scott first) | accepted |
| [0013](0013-c-abi-v4-render-dt.md) | C ABI v4: add lmv_render_dt (injected real dt); bump to v4 | accepted |
| [0014](0014-preset-dir-override-for-dev-iteration.md) | Preset-directory override (`LMV_PRESET_DIR`) with a shared resolver, polling over a watcher | accepted |
| [0015](0015-gpu-compute-particle-idiom.md) | GPU compute pipelines for particle scenes; the four render-idiom catalogue | accepted |
| [0016](0016-gpu-tests-opt-in-ci-scope.md) | Headless GPU-capture tests skip when no adapter is present (keep GPU out of the CI contract) | accepted |
| [0017](0017-preset-author-skill-lane.md) | A third skill lane: `preset-author` (preset content, not engine code) | accepted |
| [0018](0018-engine-wide-scene-compositing.md) | Engine-wide scene compositing: shared view transform, background pre-pass, feedback trails, and screen-space post-effects (fixed order, not a render graph) | accepted |
| [0019](0019-eased-parameters.md) | Eased (smoothed) parameters: render-layer one-pole filtering on injected `dt`, expression layer stays pure | accepted |
| [0020](0020-preset-grammar-v2-branching-functions-tempo.md) | Preset expression grammar v2: branching, math functions, a tempo variable, and soft typo warnings | accepted |
| [0021](0021-shared-palette-system.md) | Shared preset-controllable palette system: baked gradient LUT, named + custom stops, bindable modulation | accepted |
| [0022](0022-build-time-preset-embedding.md) | Build-time embedding of the preset library (generated, not hand-maintained) | accepted |
| [0023](0023-golden-drift-guard-uses-frozen-fixtures.md) | The golden drift guard renders frozen per-system fixtures, not shipped presets | accepted |
| [0024](0024-cross-preset-transitions.md) | Cross-preset transitions: a two-input blend stage over the engine composite, adaptive dual-live/freeze, engine-default policy | accepted |
| [0025](0025-foobar-component-version-single-sourced.md) | Single-source the foobar component version from the workspace version (generated header) | accepted |
| [0026](0026-full-composite-coverage-fullscreen-scenes.md) | Full composite coverage: background + view transform for the fullscreen/accumulating scenes (reaction-diffusion, attractor) | accepted |
| [0027](0027-scene-rotation-constant-default-calmer-cadence.md) | Scene rotation: hold one scene by default, calmer cadence, softened drop bias | accepted |
| [0028](0028-final-stage-ink-tone-remap.md) | Final-stage duotone "ink" tone-remap (paper/ink), generalizing invert | accepted |
| [0029](0029-parametric-curve-shape-params.md) | Enrich the Maurer curve family via named shape params (radial offset + phase), not new families or a superformula | accepted |
| [0030](0030-scene-target-size-hot-path-hook.md) | Third `Scene` widening: a per-frame target-size hook; hot-path notifications are now in scope | accepted |
| [0031](0031-post-stage-trait-instantiable-composite-chain.md) | Post-composite stages behind a `PostStage` trait; the composite becomes an instantiable ordered chain | accepted (membership revised by 0032) |
| [0032](0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) | Ink leaves the `PostChain`: a terminal engine post-pass, with the transition blend between chain and ink | accepted |
| [0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) | Testing strategy: named tiers, a core-only coverage ratchet, and a local pre-push gate | accepted |
| [0034](0034-internal-resolution-follows-the-target.md) | Internal render resolutions follow the target (quantized and capped); the reaction-diffusion **simulation** grid does not | accepted (Plan 0033; Outcome) |
| [0035](0035-asymmetric-attack-release-easing.md) | Asymmetric easing: `[smoothing]` accepts an `{ attack, release }` pair (supplements ADR-0019) | accepted |
| [0037](0037-internal-grid-is-a-resolution-not-a-shape.md) | An internal grid is a **resolution**, not a **shape**: aspect comes from the render target | accepted (Plan 0035) |
| [0036](0036-preset-reachable-spectrum.md) | Preset-reachable spectrum: a scalar `bin(x)` function, an N-element spectrum scene, and per-element evaluation as a bounded third step | accepted 2026-07-27 (Outcome) |
| [0038](0038-tag-driven-release-unsigned-universal-mac-app.md) | Distribution: a tag-driven GitHub Release carrying an unsigned, ad-hoc-signed universal macOS `.app`; standalone binaries only | accepted (Plan 0036; Outcome; extended by 0115) |
| [0039](0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) | 0039 — Verify easing with a deterministic transient probe, not a committed audio clip | accepted |
| [0040](0040-spectrum-level-curve-applies-before-the-easing.md) | The spectrum level curve applies *before* the per-element easing, and is a bindable exponent rather than a named mode | accepted (Plan 0038; Outcome) |
| [0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md) | Line joins are a per-endpoint flag on the segment instance, not a global cap rule | accepted (Plan 0039; Outcome) |
| [0042](0042-reachability-measured-on-the-expression-tree.md) | Preset reachability is measured on the expression tree, not inferred from frames; `--report` reads at two levels | accepted (Plan 0041; Outcome) |
| [0043](0043-reachability-reports-comparison-nodes.md) | Reachability reports comparison nodes, suppressed only where a `select()` already names them | accepted |
| [0044](0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) | The swarm's world is a 2.5D torus sized from the render target, and additive blending is why the depth axis is nearly free | accepted |
| [0045](0045-quality-tiers-floor-and-rich.md) | Quality tiers: a `Rich` tier beside the iGPU `Floor`, auto-selected with a manual pin | accepted |
| [0046](0046-linear-light-hdr-composite-bloom-tonemap.md) | The composite accumulates in linear-light `Rgba16Float`, with a bloom stage and one engine-fixed tonemap at present | accepted 2026-07-31 (Plan 0045; Outcome) |
| [0047](0047-kaleidoscope-fold-domain-disc-with-falloff.md) | The kaleidoscope folds a disc: radius clamped to the inscribed extent with a radial falloff, and a bindable fold centre | accepted 2026-07-31 (Plan 0045; Outcome) |
| [0048](0048-transformed-feedback.md) | Transformed feedback: the accumulation buffers resample the past through a bindable affine + curated-warp transform, with a selectable deposit blend | accepted (Plan 0046; Outcome) |
| [0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md) | Analysis v2: a dual-resolution low end makes the band axis truly logarithmic, and `bass`/`mid`/`treb` become normalized (with `*_raw` escapes) | accepted (Plan 0048; Outcome) |
| [0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) | Downbeat and phrase tracking: bar-aware time variables, gated by a measured confidence with a deterministic counter fallback | accepted (Plan 0048; Outcome) |
| [0051](0051-seeded-grammar-randomness-with-per-run-opt-in.md) | `hash(x)` and `noise(x)` in the grammar, seeded per preset; an opt-in per-run seed that capture paths always pin | accepted |
| [0052](0052-analysis-diagnostics-are-native-only.md) | The analysis diagnostics surface is native-only and does not cross the C ABI | accepted (Plan 0049; Outcome) |
| [0053](0053-plan-lanes-run-in-git-worktrees.md) | Plan lanes run in git worktrees, and a close merges main *into* the branch before fast-forwarding main | accepted |
| [0054](0054-runtime-tier-switching-rebuilds-on-the-live-context.md) | A runtime quality-tier change rebuilds the engine's GPU resources on the live context | accepted 2026-08-04 (Plan 0050; Outcome) |
| [0055](0055-backdrop-leaves-the-post-chain.md) | The backdrop leaves the post chain: the composite carries premultiplied alpha and the backdrop is composited underneath | accepted 2026-07-31 (Plan 0045) |
| [0056](0056-additive-scenes-emit-premultiplied-alpha.md) | A scene that draws into the chain emits premultiplied alpha equal to its own coverage, and the alpha blend saturates rather than sums | accepted 2026-08-01 (Plan 0051; Outcome) |
| [0057](0057-emitter-scene-analytic-ballistics-seeded-individuation.md) | Objects that spawn, fall and die live in a new emitter scene, on analytic ballistics with seeded per-object individuation | accepted 2026-08-04 |
| [0058](0058-bind-group-layout-collisions-carry-evidence.md) | Two bind-group layouts that can be live in one frame may not share a shape unless an allowlist carries per-pair evidence | accepted 2026-08-09 (Plan 0053; Outcome) |
| [0059](0059-line-scenes-colour-along-their-generator-axis.md) | Every line scene honours the palette and colours along its own generator's axis | accepted (Plan 0054; Outcome) |
| [0060](0060-star-pattern-variants-interpolate.md) | `star_pattern` builds its rosette at a continuous contact angle, so `variant` interpolates instead of cutting | accepted (Plan 0054; Outcome) |
| [0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) | What the fold does outside its disc is a per-preset choice, selected by one stepped param inside a single pipeline | accepted 2026-08-04 (Outcome) |
| [0062](0062-clamp-occupancy-is-the-saturation-instrument.md) | Clamp occupancy is the saturation instrument, and it is a gate | accepted (Plan 0056; Outcome) |
| [0063](0063-address-the-spectrum-by-frequency.md) | A preset addresses the spectrum by frequency, not by array position | accepted — half built (Plan 0056; Outcome) |
| [0064](0064-a-capture-may-pin-the-rich-tier.md) | A capture may pin the `Rich` tier: `shot --tier` | accepted (Outcome) |
| [0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md) | The attractor's additive deposit is normalized by particle count: a tier buys smoothness, not brightness | accepted (Outcome) |
| [0066](0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) | A reseed disturbs the cloud rather than replacing it | accepted (Outcome) |
| [0067](0067-coverage-measures-the-scene-not-the-backdrop.md) | Coverage measures the scene, not the backdrop | accepted (Outcome) |
| [0068](0068-the-projection-basis-is-a-per-family-property.md) | The 3-D projection basis is a per-family property: Lorenz renders x–z | accepted (Plan 0059) |
| [0069](0069-the-attractor-trades-sample-count-for-trace-length.md) | The attractor trades sample count for trace length: `[particles] density` and the continuous-flow streak | accepted (Plan 0059; Outcome) |
| [0070](0070-a-feedback-pass-addresses-its-own-target-in-framebuffer-space.md) | A feedback pass addresses its own target in framebuffer space | accepted (Plan 0059) |
| [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) | A numeric test contract states a property, or names the machine it was measured on | accepted |
| [0072](0072-the-c-abi-ships-from-its-own-crate.md) | The C ABI ships from its own crate | accepted (Outcome) |
| [0073](0073-the-windows-ci-critical-path.md) | The Windows CI critical path: the sweep gets one owner, and a shape claim stops sweeping | accepted (Outcome) |
| [0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md) | A ratio against an in-run control is not automatically portable | accepted (Outcome) |
| [0075](0075-ifs-family-morphs-in-singular-value-space.md) | The IFS family is parameterized by its singular values, and morphs there | accepted (Plan 0062; Outcome) |
| [0076](0076-the-attractor-keeps-the-depth-it-already-computes.md) | The attractor keeps the depth it already computes | accepted (Outcome) |
| [0077](0077-the-symmetry-stage-owns-one-coordinate-map.md) | The symmetry stage owns one coordinate map, applied at one sample | accepted 2026-08-09 (Plan 0064; Outcome) |
| [0078](0078-banding-is-a-palette-coordinate-operation.md) | Banding is an operation on the palette coordinate, not on the baked LUT | accepted 2026-08-09 (Plan 0064) |
| [0079](0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md) | The mandala interior is rings of motifs, and it lives inside `star_pattern` | accepted (Outcome) |
| [0080](0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) | The attractor owns its level, and the bright-pass thresholds exposed light | accepted 2026-08-05 (Plan 0066; Outcome) |
| [0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) | The content lane lands presets; `architect` curates the shipped set | accepted 2026-08-09 (Plan 0067; Outcome) |
| [0082](0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) | The downbeat gate holds, and the estimator is diagnosed before it is tuned | accepted (Plan 0068; Outcome) |
| [0083](0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) | In-frame geometry is measured at the line renderer's draw seam | accepted (Plan 0069; Outcome) |
| [0084](0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) | A particle mark's silhouette is a signed-distance function | accepted 2026-08-05 (Plan 0070; Outcome) |
| [0085](0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) | How much a scene occludes the backdrop is one number, at one seam | accepted 2026-08-09 (Plan 0071; Outcome) |
| [0086](0086-the-backdrop-colours-through-the-preset-palette.md) | The backdrop colours through the preset's palette | accepted (Outcome) |
| [0087](0087-the-ifs-particle-carries-its-age-and-its-last-map.md) | The IFS particle carries its age and its last map, and respawns onto the attractor | accepted 2026-08-06 (Plan 0073; Outcome) |
| [0088](0088-the-ifs-colours-by-distance-from-its-own-skeleton.md) | The IFS colours by distance from its own skeleton, and the age channel is retired | accepted 2026-08-08 (Plan 0074; Outcome) |
| [0089](0089-the-library-renews-by-replacement-cohorts.md) | 0089 — The library renews by replacement cohorts, never by a delete-all reset | accepted 2026-08-09 |
| [0090](0090-a-preset-composes-two-scene-layers.md) | 0090 — A preset composes two scene layers: a per-preset join point, linear-light blend at the `over` join, per-layer scene instances | accepted 2026-08-09 |
| [0091](0091-the-animation-gate-scores-motion-against-the-figures-footprint.md) | The animation gate scores motion against the figure's own footprint | accepted 2026-08-11 (Plan 0077; Outcome) |
| [0092](0092-the-ink-remap-gains-a-contrast-exponent.md) | The ink remap gains a contrast exponent | accepted 2026-08-11 (Plan 0078; Outcome) |
| [0093](0093-attractor-tuples-are-content-with-per-tuple-framing.md) | Attractor tuples are content: a curated roster with per-tuple framing, and morph paths only where measured | accepted 2026-08-11 (Plan 0079; Outcome) |
| [0094](0094-the-backdrop-paints-a-directional-ramp.md) | The backdrop paints a directional ramp through the preset's palette | accepted 2026-08-12 (Plan 0080; Outcome) |
| [0095](0095-the-backdrop-paints-a-curved-band.md) | The backdrop paints a curved band under the scene | accepted 2026-08-12 (Plan 0081; Outcome) |
| [0096](0096-the-display-write-dithers.md) | The display write dithers, in the encoded domain, from an integer hash | accepted 2026-08-12 (Plan 0082; Outcome) |
| [0097](0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md) | the downbeat cue is chosen against per-beat evidence, not against the ladder argument | accepted 2026-08-13 (Outcome) |
| [0098](0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) | the line renderer draws arcs as per-pixel distance fields | accepted 2026-08-13 |
| [0099](0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) | the show-length horizon is a spot-check, and it splits in two | accepted 2026-08-13 (Outcome) |
| [0100](0100-documentation-images-are-committed-headless-renders.md) | Documentation images are committed headless renders | accepted 2026-08-13 (Outcome) |
| [0101](0101-the-preset-docs-gain-a-tutorial-layer-rather-than-a-merge.md) | The preset docs gain a tutorial layer rather than a merge | accepted 2026-08-13 |
| [0102](0102-a-palette-coordinates-edge-is-a-per-preset-choice.md) | a palette coordinate's edge is a per-preset choice | accepted — decided, unbuilt |
| [0103](0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) | the IFS fit frames a figure that does not turn, and says so | accepted (Outcome) |
| [0104](0104-the-emitters-source-is-authorable-geometry.md) | the emitter's source is authorable geometry, and the pool can start warm | accepted (Outcome) |
| [0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) | The mark roster becomes a fullscreen distance field | accepted 2026-08-13 (Outcome) |
| [0106](0106-two-tone-graphics-come-from-a-multiply-layer.md) | Two-tone graphics come from a multiply layer, not a composite redesign | accepted 2026-08-13 (Outcome) |
| [0107](0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md) | An authored path is inline SVG data, and it morphs by resampling | accepted 2026-08-13 |
| [0108](0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) | a backlog claim about the repo carries an executable probe | accepted 2026-08-15 |
| [0109](0109-the-beat-clock-counts-onsets-not-beats.md) | the beat clock counts onsets, not beats, and Layer 2 gets its own grid | proposed 2026-08-15 |
| [0110](0110-now-playing-is-a-shell-supplied-string-and-the-core-owns-the-banner.md) | Now-playing metadata is a shell-supplied string, and the core owns the banner | accepted 2026-08-16 (Outcome) |
| [0111](0111-the-shape-field-gains-a-scaled-copy-coordinate.md) | The shape field gains a scaled-copy coordinate beside its distance one | proposed |
| [0112](0112-a-blender-model-enters-as-inline-mesh-data-and-the-gpu-scatters-its-points.md) | A Blender model enters as inline mesh data, and the GPU scatters its points | proposed |
| [0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md) | MilkDrop presets are translated ahead of time onto a warp-mesh idiom | accepted 2026-08-16 (Outcome) |
| [0114](0114-the-engine-renders-video-offline-and-delegates-encoding.md) | The engine renders video offline and delegates encoding to a pipe | accepted 2026-08-16 (Outcome) |
| [0115](0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md) | The foobar2000 component is a released artifact, and the SDK is a build parameter | accepted 2026-08-16 (Plan 0102; Outcome) |
| [0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md) | An index row is a pointer, and a gate holds it to one | accepted 2026-08-16 (Plan 0105; Outcome) |
| [0117](0117-c-abi-v6-the-host-reads-the-roster-and-selects-a-preset.md) | C ABI v6: the host reads the roster and selects a preset | proposed |

<!-- roster:end -->

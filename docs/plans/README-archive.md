# Plans index — archive

The full `Recently closed` write-ups, moved out of
[README.md](README.md) verbatim by Plan 0061 Phase 7b.

**This is a write-only record.** It is not loaded each session; the index keeps a
one-line summary per plan and points here. Nothing in it was rewritten — each
entry is the paragraph its close ceremony wrote, unchanged, so the reasoning a
close recorded is still quotable.

Also archived here: the **prior sequencing notes**, kept at the bottom, which
were superseded orderings of the active roster.

## Recently closed (full entries)

- [0110 — The shader surface stops being invisible](done/0110-the-shader-surface-stops-being-invisible.md)
  — closed 2026-08-19, five `dev` phases in `4595e14`, `c2b36cc`, `916df90`, `e46232f`, `2b639fe`.
  Review: **no blockers, one major, three minors.** **Phase 6 (`human`) is deliberately still
  open** — nothing is pushed, so the CI reading this plan exists to turn green has not happened.

  **What shipped.** The fixture the engine never had: `core/tests/fixtures/warp_mesh_shader.toml`,
  the only preset anywhere in the crate carrying WGSL, whose `[mesh]`, `[palette]` and `[params]`
  tables are **byte-identical** to `warp_mesh_milk.toml` and whose `[milk]` programs differ only by
  `blur_level = 3` — which is what makes the shaders-vs-defaults comparison a clean single variable.
  Around it: 7 unit tests over `shader.rs`'s pure half (no GPU), 4 GPU tests over the build, the
  partly-absent `Option` arms and the blur chain, 6 tests over the wave/shape element half of
  `milk/mod.rs`, a golden baseline appended **last** to `EXTRA_FIXTURES`, and the prelude drift
  guard — `milkconv/tests/fixture.rs`'s discipline applied to WGSL, asserting the fixture's two
  modules still begin with exactly `fragment_prelude(g)` so a hand-inlined ~2 KB of generated text
  cannot rot into a comment that used to be true.

  **The coverage arithmetic, and the log's one superseded claim.** `dev` recorded the local total
  (29,232 lines / 10,827 missed) as irreconcilable with CI's 19,935 / 2,540 and left it as an open
  question. The review resolved it: the local total reproduces exactly, and diffing it against CI's
  own per-file table from run `32008593831` sorts every diverging file into **`delta lines == delta
  missed`** (never-executed duplicate mappings merged out of `target/llvm-cov-target` — `bloom.rs`
  +507/+507, `particles/mod.rs` +1130/+1130, `expr.rs` +679/+679) or **`delta lines == 0` with
  misses down** (the real work: `shader.rs` -716, `milk/mod.rs` -216, `warp_mesh/mod.rs` -69). Same
  tree, stale artifacts; `cargo llvm-cov clean --workspace` first is what a future coverage plan
  should be told. And because `headless()` is software-preferred, **every GPU test here ran on WARP,
  the adapter CI uses** — so the per-file eliminations transfer, and the projection is **~1,530
  missed over ~19,943 lines, ~92.3 %**: 1,010 eliminated against the 746 required, ~265 lines of
  headroom over the floor, with `vm.rs` and `draw.rs` untouched as the plan wanted.

  **The major, and it is about the guard rather than the code.** Phase 1 declared its tests as
  `#[cfg(test)] / #[path = "shader_tests.rs"] / mod tests;`. `hygiene.rs`'s `is_cfg_test_module`
  skips out-of-line test modules by matching `#[cfg(test)]` **immediately followed by** `mod <file
  stem>;` — and this is neither adjacent nor stem-named. So `shader_tests.rs` is scanned as
  hot-path source and passes **only** because its `#![allow(...)]` block contains the literal
  `clippy::indexing_slicing` the guard greps for: the "spelling coincidence" that function's own
  header warns about, now live. Proven by probe rather than inferred — rename that one lint and
  `hot_path_modules_carry_the_panic_pragma` fails naming the file. Nothing is unsafe (the file is
  test-only); what is damaged is the guard, for a class of file this plan introduced. Carried as a
  followup.

  **What the phases found, and did not.** Phase 3 asked for a finding if `blur_level = 0` and `= 3`
  rendered identically — they do not (`frame_diff 0.2998`), so the followup that reserved that
  question is discharged rather than owed. Phase 5's ADR-0058 comparison, run before the bless at
  `golden.rs`'s own capture conditions, put hardware and WARP at `frame_diff 0.000267` with a
  max-channel outlier of 1: **the fifteen-binding layout with two 3D textures does not alias**,
  which is the specific hazard a new layout of that size owes a measurement for. The bless produced
  exactly one new file, confirmed by diff. The probe itself was a throwaway, so that evidence now
  lives only here — a permanent `#[ignore]`d sibling is the second followup.

  **Version: no bump, deliberately.** Tests, a fixture and a baseline; the built binary is
  byte-identical to `v0.74.0` and a tag push publishes release zips. `docs/releasing.md` blesses
  "no bump" for a chore-only plan as a choice, and this was made as one rather than missed.

- [0107 — The foobar menu picks a preset](done/0107-the-foobar-menu-picks-a-preset.md)
  — closed 2026-08-18, four `dev` phases in `1ea486b`, `bdadf47`, `2919b7b`, `4d1f450`. Review:
  **no blockers, two majors (both repaired at the close in `cc1b7ef`), four minors, two nits.**

  **What shipped.** [ADR-0117](../adrs/0117-c-abi-v6-the-host-reads-the-roster-and-selects-a-preset.md)'s
  two functions and nothing else — `lmv_get_presets` (caller-buffer roster snapshot, call-twice
  sizing, NUL-separated UTF-8, plus the current index) and `lmv_select_preset`, with
  `LMV_ABI_VERSION` 5 → 6. **Fifteen exports in `core-cabi/src/lib.rs`, fifteen in the header,
  signatures identical** — checked at the review rather than assumed, because nothing mechanically
  compares them (no cbindgen, per ADR-0003). One core addition, `Renderer::target_preset_index`, a
  thin read of the dissolve's target so a host checkmark follows the click and not the fade. Shim
  side: a flat **Preset** submenu with a radio mark, **Reload presets**, **Open presets folder**, and
  the component's first `cfg_var` persisting the choice **by name**.

  **The done-when that could not be satisfied as written, and what replaced it.** Phase 1 asked the
  FFI test to defend the roster claims, but the roster only exists after `lmv_attach_window` — so
  there was nothing to assert against headless. The user chose the fix before the "go": the test
  opens a hidden `STATIC` window through two raw `extern "system"` user32 declarations (no new
  dependency, per ADR-0001's every-crate-is-a-cost rule) and drives the **real attach path — its
  first coverage ever**. It **skips green** when no adapter can present, so a CI pass is not evidence
  for any of it; the review confirmed it genuinely attaches here by re-running under
  `--success-output final` and seeing no skip notice. Recorded as a known gap in
  [`docs/specs/0001-c-abi.md`](../specs/0001-c-abi.md), which is the honest place for it.

  **Path drift, found and repaired mid-plan.** The plan and the spec both said `core/tests/ffi.rs`;
  ADR-0072 had moved it to `core-cabi/tests/ffi.rs` at Plan 0061's close. Three further stale
  `core/src/ffi.rs` references were sitting in the header's own comments, including the
  `static_assert` message that exists to catch a layout mismatch — a guard pointing at a file that
  had not held the struct for fifteen plans.

  **Three judgment calls the plan did not make.** `Space` also writes the persisted name (the plan
  said "menu pick or Next scene"; `Space` *is* Next scene). Reload re-stores the name afterwards, so
  a deleted file cannot leave a stale value behind. And the restore runs on the mid-playback
  format-change handle re-creation too, which previously snapped the show back to the roster's first
  entry — the review's one caveat on that being that it restores with a **dissolve**, since the ABI
  has no cut form, so a fresh handle crossfades from the roster's first entry over ~1 s.

  **What the review repaired.** The Phase 4 doc sweep had written **`B`** for the standalone's browse
  overlay in two places in `docs/presets.md`; the binding is **`Tab`** (`standalone/src/main.rs` maps
  `KeyCode::Tab`, and there is no `KeyB` arm anywhere). That is the doc the `preset-author` lane reads
  as truth, which is what made it a major rather than a typo. Separately,
  `docs/on-device-validation.md` was absent from the plan's Phase 4 holder list and still described
  the component as `Space`-only.

  **Phase 5 (`human`) did not run, and the close does not claim it did.** It became a standing
  checklist item in `on-device-validation.md`, the way Plan 0102's Phase 5 was carried forward —
  that file's own escalation rule is that on-device checks do not gate closes here.

  **What outlived the plan.** Three filed rather than fixed. **Backlog 0117**: the menu dispatches a
  snapshot index across a modal wait on the argument that *"nothing on this thread can reload presets
  between the build and the click"* — but `TrackPopupMenu`'s modal loop dispatches `WM_TIMER`, whose
  handler reaches `ensure_handle` and can replace the handle outright, which the post-dismiss
  null-check does not notice. Bounded in practice, and the fix is to dispatch by name the way every
  other selection path in the file already does. **Backlog 0118**: `foo_lmv.dll` measured
  **9,279,488 B** against the spec's recorded 8,879,104 B, so NFR §4 headroom is **~0.72 MB**, not
  the "~1.07 MB" the spec and this plan's own ADR both advertise. The plan links nothing new and is
  not the cause; the ~400 KB arrived unattributed between Plans 0097 and 0107, and the spec's stated
  re-measure trigger ("when a dependency is added") would never have fired on it. Recorded as a dated
  `Outcome` on ADR-0117 rather than edited into its body. **Backlog 0103** was updated in place: it
  convicts a menu of two items that is now five items and a submenu, so the entry is stronger and
  stays live.

- [0108 — The MilkDrop import gets its tone back](done/0108-the-milkdrop-import-gets-its-tone-back.md)
  — closed 2026-08-17, four dev phases in `b02cd45`, `60674da`, `6e92eb3`, `a07b0c6`, with Phases 2
  and 6 (`human`) run as one live look-gate session at the close. Review: **no blockers, two majors
  (both repaired at the review), three minors, one nit.**

  **What shipped and works.** [ADR-0118](../adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)'s
  feedback quantizer, in both warp epilogues out of one WGSL text, driven by a runtime uniform and
  on by default for any `[milk]` bundle. Off is an exact identity — `warp_mesh.png` did not move, and
  a bless-to-bless control moved exactly 1 of 32 baselines. The field reaches **exact zero** and
  stays black at a hundred times the brightness that shows the unquantized control still positive.
  The transfer-function domain was *rendered* rather than assumed and recorded as a dated `Outcome`.
  Phases 4 and 5 fixed the `wave_usedots` beads (a mark carried no caps, so it was a sub-pixel dash —
  **2 of 512 marks lit at 320x180**), `wave_mode 5` emitting a stroke where the preset asked for
  dots, and a custom wave's per-point state being reset between points — the last reaching **3 368 of
  the corpus's 6 347** custom-wave presets.

  **What the plan got wrong, found by its own look gate.** The verdict on its central question was
  **still merely different** — one pair better, six wrong, and *not one of the six for the reason the
  plan was built on*. Backlog 0106 had claimed one mechanism with four presentations; on the five
  pairs with no video echo the background sits **three orders of magnitude above the quantizer's
  floor**, so nothing it does can reach them. Two conclusions were falsified outright: Phase 5's
  attribution of *chasers 19 Portal*'s fold to per-point code (the real cause is
  `per_pixel_3 = sx = -zm`, and `pow(max(v, 1e-4), dt)` clamps the sign away — **363 corpus files**),
  and backlog 0106's "tonal inversion", which is a mid-grey non-additive waveform drawn over an
  already-washed ground rather than any inversion at all.

  **What outlived the plan.** Four engine defects it was never scoped to fix — the clamped negative
  scale, the missing video-echo stage, `time * 0.05` in the mode 6/7 waveform angle (suspected by
  Phase 4, convicted by the gate), and **the wash itself, still unexplained and dominant**. All four
  carry to [Plan 0109](0109-the-milkdrop-import-gets-its-geometry-back.md) and backlog 0113-0116.
  Two wash hypotheses died during the gate and are recorded so nobody re-runs them: the deposit is
  already `dt`-scaled, and `bAdditiveWaves` does not separate the washed presets from the clean one.
  Also recorded: the review's own major — a wave's per-point carry crosses the **frame** boundary
  too, so an odd-length trace inverts every frame at the display's refresh rate, believed faithful
  but **unverified against the reference**.

- [0101 — The engine renders a music video](done/0101-the-engine-renders-a-music-video.md)
  — closed 2026-08-17, four dev phases in commits `39b36e6`–`0ab8400`, Phase 5 (`human`) run live at
  the close. Review: **no blockers, no majors, five minors and nits** — two of which were found by
  *running* the feature rather than reading it. **`shot --render` walks a WAV at a fixed injected
  `dt` and streams Y4M to a user-supplied `ffmpeg`; no encoder ships and `lmv.exe` did not change
  size.** Phase 1 chose Y4M over NUT on the plan's own tie-break (a plain-text header, one
  non-`ffmpeg` consumer already foreseen in Plan 0106), paying an RGB→YUV conversion for it. That
  choice reached Phase 3 exactly as the plan predicted, and the plan's escape clause was used as
  written: byte-identity is asserted on the **RGB frame** handed to the writer, never on the wire,
  with the 8-bit conversion swept as its own round-trip property — a wire-level version could only
  have been loosened to a tolerance until it passed. **Phase 3 needed no `core/src/render/` change
  at all**, which the plan had budgeted for: `capture_stream` goes through the one `draw_frame` the
  on-surface present path and every capture path share, so the tap sits after the tonemap
  (ADR-0046) and after the encoded-domain dither (ADR-0096) *structurally* rather than by
  placement. The review walked both clocks and confirmed the identity is real — at 30,720 Hz a hop
  is 1/60 s, so frame *N* and hop *N* coincide, and at index 45 both paths have pushed 46 hops and
  advanced the clock 46 × `FALLBACK_DT`. **Phase 4's argument is structural and was verified as
  such**: the mode encodes no passes of its own, so it inherits `capture::read_back`'s poll — the
  same retirement Plan 0099 had to give `step_offscreen` — and the resident set is *reported, never
  asserted* (ADR-0071), with warm-up split from growth because the measured 357→434→434 series
  would otherwise have printed "+76 MB" and read exactly like 0099's leak. Determinism was checked
  at the root: no wall-clock read exists anywhere in `core/src/render/`, and the frame-time governor
  lives in the surface present path where no capture reaches it, so `--tier rich` renders at full
  cost. The C ABI did not move. **Phase 5, run live on Floating Points' *LesAlpx* (4:41,
  `attractor_leviathan`, 1920x1080/60 `--tier rich`): 16,869 frames in 37m20s, a playable MP4 with
  audio, container tagged `color_range=pc`/`bt709`/`60/1`, duration 281.140998 s against the
  source's 281.141565 s — `-shortest` trimming the trailing partial frame exactly as designed — and
  resident growth of **-7.9 MB across the whole track**, i.e. Phase 4's property holding outside the
  harness at full size and 17 % past the done-when's length. **The verdict was yes, and its first
  question answered no:** an offline `Rich` render does *not* look better than the live app, it
  looks like the same image larger ("it just looks like Leviathan upscaled"). That impression was
  run down rather than filed as taste — `TierConfig::attractor_particles` is a fixed 50,000/150,000
  with no render-target term while the trail grid *is* surface-sized, so 1080p multiplies pixels 4x
  while `Rich` multiplies particles 3x and **density falls as resolution rises**. Filed as
  [backlog 0110](../design-backlog.md), which now gates whether a rendered file is publishable and
  which Plan 0103's demo material depends on. Two more findings came from the same session and were
  filed rather than fixed: the encoder is spawned *and* a GPU device built before `--preset` is
  validated, so a typo'd name leaves a 262-byte audio-only MP4 at `--out`
  ([0111](../design-backlog.md)); and the one canonical `ffmpeg` invocation is archival-grade with
  no size lever — 3.73 GB and 106 Mbit/s for 4:41, about 9x a typical 1080p60 upload rate
  ([0112](../design-backlog.md)). Version: **0.71.0 → 0.72.0**.
- [0100 — The engine speaks MilkDrop](done/0100-the-engine-speaks-milkdrop.md)
  — closed 2026-08-16, six dev phases in commits `2603309`–`0948cf2` across two sessions (one `wip:`
  checkpoint kept unsquashed by the no-rewrite rule), Phases 7 and 8 (`human`) run live at the close.
  Review: **no blockers, one major, two minors.** The major was the review's own operator-doc sweep
  firing: `warp_mesh` is a full palette participant — per-pixel fragment-stage LUT, so both
  `palette_steps` *and* `palette_contour` are live there — and `docs/preset-palettes.md` had no
  section and no scoping-table row for it; repaired at the close. The minors were both fat index
  rows, one already trimmed from `main` by Plan 0105's close. **The stop condition never fired**:
  Phase 6's census (430,854 shader lines, ~30 intrinsics) predicted a hand-written Rust HLSL
  frontend would cover the corpus, and it did — 80.1 % converts with shaders, 77.9 % renders
  non-blank, `emitter-invalid` at zero. The review verified the done-when tests as real: the Phase 6
  test renders the converted MD2 fixture against a shader-stripped control and asserts inequality;
  the tier ladder is dated, machine-named and asserts nothing (ADR-0071 at its best); ADR-0037
  discipline included a three-grids-one-aspect regression test. **Phase 7, judged over seven presets
  side by side against `foo_vis_milk2` 0.2.0.0: mostly there, with defects — and the HDR pipeline
  makes them *merely different*, not better**, the finding the plan itself said would outweigh the
  feature, recorded as a dated Outcome on ADR-0113. Structure, motion and reactivity survive in
  every pair; four defects filed as backlog 0106 (the float field never truncates where the
  reference's 8-bit target does — dominates the verdict, re-judge after it lands), 0107 (waveform
  placement/dots, a horizontal reflection seam in warp sampling, Portal's inert mirror), 0108 (the
  HLSL-array and converted-but-blank tail). **Phase 8: decide later** — nothing third-party ships,
  the import path stays converter-plus-user-directory, recorded in `docs/presets.md`. **Curation
  (step 3b): no preset content landed** — `presets/` gained only its README roster — so no
  near-duplicate sweep was owed; the workaround grep is unchanged by this plan. Size: whole plan
  +383 KB / +376 KB against the plan's "near zero" expectation, honestly recorded as optimistic by
  ~140 KB; cdylib at ~8.8 MiB under NFR §4's ~10 MB soft cap.
- [0102 — The component ships](done/0102-the-component-ships.md)
  — closed 2026-08-16, Phases 1-4 in three commits (`e5e03de`, `07f1573`, `56c3edf`), one session.
  Review: **no blockers, three majors, four minors.** **Phase 1's `human` question — the one this
  had been waiting on for sixty plans — took one reading:** the foobar2000 SDK licence is
  BSD-style, permits binary redistribution, and puts a notice obligation only on redistributed
  *source*, so Phase 3 took the CI-fetch branch and the component is a released artifact. The
  fetched archive is **byte-identical to the copy this project has built against since Plan 0001**
  (SHA-256 recomputed independently at the close), and foobar2000.org keeps every SDK at a stable
  `/downloads/SDK-<date>.7z` URL back to 2011, so the pin cannot rot from under it. **The recipe was
  verified as an artifact, not as a diff:** `target/dist` carries a deliberate `v9.9.9` run beside
  the `v0.69.0` one, so the version-substitution path was exercised rather than assumed; the
  produced `.fb2k-component` holds exactly `x64/foo_lmv.dll` and nothing else, which is the check
  that matters because foobar2000 extracts a component's whole archive into the user's components
  folder; and every reader-facing claim in `READ-ME-FIRST.md` was checked against `foo_lmv.cpp`
  rather than believed. **`build-component.ps1` earns its 350 lines in two places a
  `Compress-Archive` could not:** it names every zip entry by hand, because both
  `Compress-Archive` and .NET Framework's `ZipFile` emit **backslash** separators on Windows
  PowerShell 5.1 and forward slashes on pwsh 7 — and a check that read the archive back through the
  same broken API would have passed either way; and it reads the PE headers directly to assert
  machine type, the `foobar2000_get_interface` export, and the declared version, so `-SkipBuild` on
  a box with no MSVC is still held to the full bar. **Two scope deviations, both defensible and both
  reported by `dev`:** `docs/releasing.md` was edited though Phase 3 lists it as the *alternative*
  branch (it claimed "two zips" in three places against a workflow that now demands three — the
  operator-doc sweep, not scope creep), and `READ-ME-FIRST.md` landed in Phase 2 rather than Phase 4
  because the recipe's copy step is fatal without it. Phase 2's "factor out the packaging step" had
  no packaging step to factor; what was genuinely duplicated was the ADR-0025 version regex, now
  dot-sourced from `packaging/foobar/lmv-version.ps1` — **two of its three copies**, the third still
  inline in the workflow's `windows` job. **The three majors.** The shipped troubleshooting text
  mis-diagnosed the one first-run failure already on file — [backlog 0102](../design-backlog.md)
  says a docked panel can render without presenting and revive only at a track boundary with nothing
  in the Console, and the text sent the reader hunting for Console lines that will not be there;
  fixed at the close, with [backlog 0103](../design-backlog.md)'s undiscoverable layout-removal
  workaround added beside it. Neither the plan nor ADR-0115 names those two entries as distribution
  risks though both are `Medium` and both are what a stranger meets first — recorded in the plan's
  close rather than back-edited into an accepted ADR. And the pre-staged SDK route, which ADR-0115
  makes first-class, stamps `@SDK_VERSION@` into a shipped document while asserting nothing about
  what is staged; filed as [backlog 0105](../design-backlog.md), whose fix is one grep against the
  `sdk-readme.html` the SDK itself ships. **What outlived the plan:** the component is now on the
  release critical path — `needs: [macos, windows, foobar]` plus an exact three-zip count means a
  foobar-job failure produces **no release at all**, standalone zips included. That is the right
  trade, and it is what makes `build.ps1`'s hardcoded `/p:PlatformToolset=v143` (under a comment
  claiming the toolset is retargeted) a release-wide risk on the next runner-image bump rather than
  a plugin-only one. **Phase 5 is unrun by design** — it tests the *released* artifact, which the
  tag this close writes has not yet produced; it is carried to
  [`on-device-validation.md`](../on-device-validation.md) with its expected failures named, and the
  SDK-staleness watcher `sdk-pin.ps1`'s header admits nothing guards is a followup, not a gap.

- [0099 — The horizon reaches its own length](done/0099-the-horizon-reaches-its-own-length.md)
  — closed 2026-08-16, all three `dev` phases in two commits (`b0a5ba0`, `cb8a434`), one session.
  Review: **no blockers, no majors, four minors and a nit.** **The plan's discriminator was the
  whole value of it, and it returned a third answer rather than either of the two on offer.**
  `reaction_etching` — the RD world the original measurement missed — fails like its two siblings,
  so the family reading held; but the ceiling is **not the 3,601 frames** the backlog entry, this
  plan and [ADR-0099](../adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md)
  all recorded. All three worlds clear **5,401** and fail at **7,201**, one unfixed run died at
  4.4 GB and another squeaked to 36,001 at 4.9 GB: it is memory pressure, so 3,601 was one machine's
  headroom read as a limit. And it is not the RD family's *mechanism* either — **retention is per
  pass, not per pixel**. Over one unpolled stretch a 13-pass RD frame retained **950 KB** against a
  36 KB captured frame where single-pass worlds retained ~30 KB; every world grew, RD reached the
  allocator first, and that is what made the family look responsible when the poll cadence was. The
  prediction that follows was confirmed **before anything was changed** — the same world over the
  same 36,001 frames cleared at `--interval 5` and died at `--interval 30`, because
  `capture::read_back` held the only device poll on the path and the interval *is* the reclaim
  period. **Phase 2 is one line and the argument for it is measured, not assumed:** a non-blocking
  `PollType::Poll` in `step_offscreen` took the same stretch from 3,668 MB to 3,188 MB and no
  further, because a headless loop submits far faster than the GPU drains, so it is
  `wait_indefinitely` — the same `poll(Wait)` the sampled path always paid. Result on the run that
  died: 419 MB -> 4,402 MB then dead, against 398.8 MB at 4 s -> 399.6 MB at 54 s after, at ~1.5x
  wall clock on an RD world; `capture_api.rs` is off the hot path by construction and nothing behind
  the C ABI reaches it. **The regression test's honesty is the part worth copying.** It pins 6,000
  frames as **one** unpolled stretch (`at_frames = [0, LONG_RUN]`), past 3,601 *and* past the 5,401
  that still cleared — and it is hardware-gated because, written first on this suite's WARP
  renderer, it **passed against the unfixed path**: 6,000 unpolled frames do not accumulate on the
  software adapter, so it was a three-minute no-op rather than a regression test. Windows CI has
  only WARP ([ADR-0073](../adrs/0073-the-windows-ci-critical-path.md)), so it skips there and prints
  why, which means **this guard has no CI cover at all** and earns its keep on a developer box —
  stated rather than papered over. **Phase 3 found a second way to overstate a run, and it survives
  the memory fix entirely:** a `--horizon` the `--interval` does not divide is floored to the last
  whole interval (rows are exact multiples, which is what makes row *k* comparable across runs), and
  nothing said so — `--horizon 10 --interval 45` printed a header claiming 10.0 minutes over a table
  ending at 585 s. The header now states the length **reached**, flags the shortfall directly above
  the table, and the JSON carries `reached_secs`/`shortfall_secs`/`truncated`. A run that dies now
  prints a `TRUNCATED` block **on stdout where the table goes** — verified live against a
  deliberately un-fixed build — with the wall clock, the resident set it died at, and the levers,
  naming `--interval` as explicitly **not** one: it was the right advice for one afternoon and
  Phase 2 made it wrong. **Minors, all four documentation or assertion-shape rather than behaviour.**
  The long-run check asserts `frame_diff > 0.01`, which is exactly the shape
  [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) removed from
  `core/src/render/tests.rs`, where a comment records that same threshold as *"half of `NOISE_FLOOR`
  and so … inside the band this project already calls noise"* — the determinism contract makes the
  exact form free, and the failure it guards against (the first frame returned twice) is a byte
  identity. `docs/capturing.md` called the resident set *"flat: 324 -> 400 MB"* in the same sentence
  that put the travel at 0.8 MB; repaired at the close to separate the before/after report from the
  0.8 MB the render itself travels. The `preset-author` skill's two copies of the horizon bound
  still told the content lane that the RD worlds cannot reach ten minutes — swept at the close, and
  the wall-clock estimate they carried (*"roughly 10 s per simulated minute"*) replaced with the
  measured 16 s / 54 s. And the plan's Phase 3 file list named `standalone/examples/shot.rs` where
  the table and JSON actually live in `standalone/src/shot/horizon.rs`; `dev` said so in the commit
  rather than editing either. Nit: the shortfall tolerance is one frame, which silently absorbs a
  sub-frame shortfall — correct, and worth knowing before anyone reads `shortfall_secs` as exact.
  **Two things outlive the plan.** Backlog 0093's `absent: poll` probe went **red on delivery**,
  precisely as [ADR-0108](../adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)
  designed it to — a probe whose falsification is the finish signal, and the first shipped instance
  of that working end to end. And two ADRs took dated `Outcome` sections rather than edits: 0099's
  item 3 (the frame-ceiling claim, wrong twice over) and 0114's *"long renders inherit an unfixed
  defect"* Negative, which now carries the one conditional Plan 0101 Phase 4 needs — a render mode
  that submits its own passes outside `step_offscreen` inherits the defect and none of the fix.

- [0105 — The indexes go back to being indexes](done/0105-the-indexes-go-back-to-being-indexes.md)
  — closed 2026-08-16, all six `dev` phases (`5791d25`, `0171fdf`, `f17be77`, `665eb0e`, `34b72ea`,
  `7903351`) in one session. Review: **no blockers, two majors, two minors, one nit.** The three
  roster files went **477,594 -> 220,626 bytes** — `docs/adrs/README.md` alone 189,305 -> 21,085, an
  89 % cut against the ~24 KB [ADR-0116](../adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)
  predicted — and `scripts/check-index-rows.mjs` now holds every row inside a
  `<!-- roster:begin cap=320 -->` region to 320 bytes at the pre-push hook, the CI `links` job, and
  the architect close ceremony. **The cap did not move**: the widest row in the tree is 246 bytes.
  **Three deviations, every one surfaced in the commit that hit it rather than tuned to.** The
  over-cap count was **136, not 135** (26 plan bullets against the plan's 25; the ADR and backlog
  figures matched exactly). Phase 2's `awk`-and-`grep` forward-reference invariant **moved 7 -> 3
  rather than holding** — four of the seven matches were the regex catching ordinary words (*"an
  owed supplement"*, *"extended to"*, *"a superseded blob"*) inside the abstracts the phase was
  deleting, and a fifth was `awk` splitting a row whose title cell contains a pipe; the **three
  genuine inbound edges** (0002 supplemented by 0020, 0003 extended by 0006/0008/0013, 0031
  membership revised by 0032) are byte-unchanged, and all 24 outbound trailers were checked edge by
  edge against the ADR bodies and found redundant. And Phase 4 came back green a phase early,
  because the gate does not know it is not wired up yet. **The verification work is the part worth
  keeping.** Every number and backticked identifier in every ADR index cell was matched against the
  linked body and each survivor read by hand: 43 flagged, 39 resolved as notation, **four
  genuinely index-only** — those became dated `Outcome` sections on ADRs 0046, 0103, 0104 and 0105
  rather than being deleted, on the ADR-0054 / ADR-0074 precedent. The plans section turned out to
  be worse than assumed: **24 of the 26 fat bullets had never been archived at all**, so the
  section had been emptied once by Plan 0061 Phase 7b and then regrew entirely fresh — preservation
  was never the problem, which is precisely ADR-0116's argument. Moving that prose stranded **9
  reference-link uses** from their definitions, the third break class, caught by
  `check-doc-links.mjs`. In the backlog ledger exactly **one** datum survived both the archive and
  the pointed-at document — commit `3732fb4` in row 0056, which is in git and in no document at all
  — and it stayed in the row. **Two majors, both outside the trim.** First, **the gate has no
  assertion that it can convict**: its fixture is the only one in `scripts/fixtures/` that asserts
  exit 0, and a mutation that makes the row matcher match nothing still exits 0 there and on the
  repo, so a dead detector reads exactly like a clean tree — the same non-vacuity class Plans 0084
  and 0094 fixed for the other two gates, filed as [backlog 0104](../design-backlog.md). Second,
  **Phase 5 changed what a `git push` runs without sweeping the two operator docs that enumerate
  it** — `README.md`'s "What it runs" table and `docs/nfr.md`'s CI-gate and pre-push bullets both
  still named only `check-doc-links.mjs`, and had *already* missed `check-backlog-claims.mjs` since
  Plan 0093, so this was the second consecutive miss on the same two lists; repaired in the close
  commit, and `CLAUDE.md`'s `scripts/` description with them. Minors: ADR-0116's Negative bullet
  estimating *"roughly six rows"* of forward-reference graph measured **three**, now recorded as a
  dated `Outcome` there — which matters because that number is the whole argument against
  regenerating the index from the bodies; and the plan's own TL;DR and Phase 1/3 done-whens still
  read 135/25, left standing since a plan is the contract as written and the correction belongs
  here. Nit: two missing blank lines in the architect skill's new steps, where a `2.`/`3.` list
  marker cannot interrupt the paragraph above it and renders as run-on prose. **What outlives the
  plan is the loop, not the trim.** The ceremony that wrote the fat rows now states the pointer
  shape at each of the three places it refreshes a roster and re-runs the gate at step 1d, so the
  lane that causes the defect is the lane it fires on — and this close was its first live exercise.

- [0097 — The track announces itself](done/0097-the-track-announces-itself.md) — closed
  2026-08-16, all five `dev` phases (`c9f7a3e`, `3621030`, `cb41dee`, `1c96327`, `51d4489`) plus
  one **approved out-of-plan fix** (`1016777`), with the `human` Phase 6 run the same day on the
  user's machine. Review: **no blockers, no majors, three minors and a nit.** A track change now
  fades a two-line artist/title banner in over the visuals; the **core owns it** (string, `dt`
  envelope, layout, truncation) and each shell only supplies a string, so the two frontends
  cannot drift on what a track change looks like. **Phase 4's stop condition did not fire**, and
  Phase 5 corrected which artifact it measured: the shipped `foo_lmv.dll` went **6,774,784 ->
  8,879,104 B (+31.1 %)** against NFR 4's ~10 MB cap — the shim links the *staticlib*, so the
  cdylib Phase 4 first measured is a proxy, not the number the cap is about. ~1.07 MB of
  headroom, the tightest this component has had. **Both facts ADR-0110 flagged `unverified` are
  now settled** and carry a dated `Outcome` there: foobar2000 **v2.25.10** publishes SMTC with no
  extra component, and the WinRT apartment risk — which the plan called the most likely place
  Phase 2 would stall — resolved by *not reusing either apartment*, the source owning its own MTA
  thread because the capture thread is real-time and winit's loop is an STA. **The lesson worth
  keeping is from Phase 6, not the code.** The plugin half was correct and still unusable on
  first contact, because two **pre-existing** defects sit between a working core and a visible
  banner — a panel that attaches its surface at 1x1 ([backlog 0102](../design-backlog.md)) and a
  context menu that shadows foobar's layout menu ([backlog 0103](../design-backlog.md)) — plus a
  third, a render timer that once killed had nothing able to re-arm it, fixed under an explicitly
  approved scope expansion (`1016777`, with a watchdog that re-derives visibility from the window
  rather than trusting edges). Diagnosing them cost **four wrong hypotheses**, each killed by a
  measurement the next one should have started from: font-system cost (19 ms, 327 faces — not a
  stall), a new DLL dependency (`dumpbin`: import tables byte-identical to the pre-plan build), a
  crash reading of an abrupt log stop (it was `KillTimer`), and a 6.4 fps "collapse" that was
  `kIdleTimerMs = 150` doing exactly its job with nothing playing. **In this shim, "the core can
  draw it" and "the user can see it" are far apart.** Touched no presets, so the curation sweep
  did not fire; carries no `Closes:` header, so nothing was archived.

- [0096 — The HUD gets out of the way](done/0096-the-hud-gets-out-of-the-way.md) — closed
  2026-08-16, all three `dev` phases (`5e5ce0d`, `6c9694f`, `ad86ff8`), the same day it was
  written. Review: **no blockers, no majors, two minors and a nit.** Three shell-local UX nits
  from a user report, and it stayed shell-local: every file under `standalone/` plus `README.md`,
  no core change, no ABI change, no dependency. The preset name is now a **pure, unit-tested
  rule** (`preset_name_visible(modal, diagnostics, enabled)`) rather than an unconditional push —
  presence-based, so it returns the instant the modal or the F3 panel closes — and the F3 capture
  line is deliberately **not** gated on it, because that line exists *because* the panel is up
  (Plan 0083). `Escape` leaves fullscreen, intercepted after the modal branches and gated on
  `modal().is_none()` so it cannot steal a menu-close, and it **never quits**. The new `[hud]`
  section (`preset_name`, default `true`) is the one 0097 Phase 2 was told it might have to
  create — it does not. **The check worth keeping from this close is the one that came back
  clean**: `Escape`'s guard and `toggle_fullscreen` both read `window.fullscreen().is_some()`,
  not `config.output.fullscreen`; had they disagreed, `Escape` would *enter* fullscreen and no
  test at the development configuration could have told which source was used. The minors were a
  doc sweep (`on-device-validation.md` described the F3 overlay without saying the corner name
  now steps aside, while the checklist has the tester identify presets in exactly that state —
  fixed in the close commit) and a done-when that is not observable as written: toggling the
  `Preset name` row shows nothing on the canvas while the menu that edits it is open, because
  Phase 1's rule correctly wins. Touched no presets and closed no backlog entry, so neither the
  curation sweep nor the archive step fired.

- [0091 — The figure fills the frame](done/0091-the-figure-fills-the-frame.md) — closed
  2026-08-16, Phases 1-5 (`e2dd537`, `7f93b3e`, `78d1671`, `080a7ef`, `82c7471`). The `human` **Phase
  6 carries forward** (Standing / content brief item 6) and **Phase 7 was cut** — see [backlog
  0095](../design-backlog.md). Review: **no blockers, no majors, three minors.** `shape_field` is
  the **tenth system**: the shared `marks` roster drawn as a fullscreen distance, so the palette
  coordinate is a *distance* and `palette_steps` draws **concentric offset contours of a shape**
  (ADR-0105's whole argument, and it holds). **Phase 2 is the phase that earned the plan.** The
  exterior nobody had ever looked at was measured against a numerically sampled outline and both
  cheap arms were exactly as wrong as `marks.rs:33-37` implied — `polygon` **0.326**, `star` **1.057**
  out — and both are now exact outside, each keeping its original expression verbatim for `d <=
  1` so **the particle path moves zero pixels**; verified at the close against the *committed*
  baselines, not merely re-blessed. `star`'s **interior** stays approximate knowingly, its error
  stated as a function of point count (0.066 at 5, 0.248 at 12) and bounded from *both* sides —
  the lower bound fires if it ever becomes exact, because that would mean the sprite's arithmetic
  moved. **Phase 1 falsified one of ADR-0106's own Negatives** (a particle layer *can* darken,
  luma 0.9 against the field route's 18.9 — footprint, not capability) and settled its open path
  negatively (multiply does not reach the backdrop; a two-tone graphic's light ground comes from
  the chain). Both are a dated `Outcome` on that ADR. **Phase 4 is mostly verification and says
  so** rather than pretending to build: `color_center` scrolls the rings — checked across a full
  cycle *including the wrap step*, on a cyclic gradient — `scale` breathes monotonically (14 → 26
  → 41 px), `palette_steps` steps between whole figures. The one thing built is the `gamma`
  response exponent, whose **direction is the opposite of `ink_gamma`'s** (below 1 tightens
  toward the centre), documented as a table in three places. **The ADR-0037 test bites and was
  confirmed by reversal** — a literal `1.0` for the target's aspect renders the disc 29x14 at
  240x120 and fails on both orientations; the first time this family's aspect claim has been
  proved rather than asserted, on a bug that shipped here three times. Phase 5 promoted
  `STAR_INNER` and added edge curvature and seeded per-spike jitter, every default an exact
  identity, with the jitter drawn from an **integer** hash (a `sin`-based one differs between
  GPUs). Minors: five user-facing docs still said "nine systems" (repaired at the close with
  count-free phrasing; `docs/preset-guide.md`'s gallery entry is genuinely owed by the first
  shipped world); `core/tests/distinctness.rs`'s family array does not carry `ShapeField`
  (harmless at zero presets, and exactly what its own comment warns about); and one test asserts
  only the first half of its own name. **Curation (step 3b):** no preset content landed — engine
  only, by design (ADR-0081) — so no near-duplicate sweep owed, and the workaround grep over all
  27 headers finds nothing citing a defect this plan repaired.

- [0090 — The emitter's source moves](done/0090-the-emitters-source-moves.md) — closed
  2026-08-15, all four `dev` phases (`a274a48`, `10072ed`, `1c87eb7`, `669c6bd`), **and the
  `human` Phase 5 was judged hours later the same day** — so the plan is complete on all five. **Both
  of ADR-0104's argued negatives held under a look**: `spawn_fade = 0.35` hides the pop of a
  source on the screen midline (against a paired `spawn_fade = 0` control), and a prewarmed world
  does **not** switch in badly, which discharges the transition-stage crossfade followup unfired.
  `emitter_perseids` keeps its place — a fast shower and a slow sky are different looks, not two
  tunings of one. The verdicts are a dated `Outcome` on
  [ADR-0104](../adrs/0104-the-emitters-source-is-authorable-geometry.md); what remains under
  Standing is authoring the two worlds, which is content-lane work with the answers already in
  hand. Review: **no blockers, no majors, two minors, one nit.** The source line is now four
  preset-owned scalars — `source_y`, `source_width` (**fractional**, so `aspect * 1.0` is
  bit-for-bit the line the scene always drew), `spawn_fade` and `prewarm` — and the "no
  positionable source" paragraph is retired rather than reworded. **Zero pixels moved, and that
  was verified at the close rather than taken on report**: the golden suite passes against the *committed*
  baselines, not merely re-blessed. **Phase 3's measurement is the part that outlives the plan,
  and it corrected the plan's own guess.** The plan expected the animation gate to be the wall
  for a sparse slow look; it is not — ADR-0091's footprint statistic passes the draft **cold** at
  `0.0629` against a `0.01` floor. The wall is `sanity`, which convicts the draft **blank** at
  `prewarm = 0` (cover `0.0074`, 0 of 10 radial shells) and passes it at `prewarm = 1` (`0.1470`,
  10 of 10). Reactivity is the one still short — `0.0195` against `0.02` — on a draft nobody
  tuned for it. **No gate's capture length, floor or statistic moved**, which is what [backlog
  0068](../design-backlog-archive.md#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
  named as the wrong answer; the warm-up got attacked instead. Two deviations, both improvements
  and both declared in their commits: `spawn_fade` landed CPU-side at the draw site rather than
  in the shader (the emitter resolves brightness on the CPU, and the draw site is the seam where
  easing moves the whole population rather than only new marks), and `prewarm` gained a
  `MAX_PREWARM = 2.0` ceiling plus a window clipped at the longest possible life, so the first
  step is bounded at two pool-fulls however hostile the binding. **Phase 4's `systems.md` sweep
  was a done-when rather than a reviewer's catch** — the fix Plan 0081's close identified — and
  it worked: the file was current before the content phase that reads it. **One finding banked
  for the backlog machinery:** entry 0068's `present: SOURCE_Y: f32 = -1\.12` probe was written
  to go red on delivery and stayed green, because `DEFAULT_SOURCE_Y` still contains the
  substring. Anchor a probe on the line, not on a bare identifier. **Curation found one stale
  preset, on a second pass.** `emitter_perseids.toml:7` still declares the quiet sky *"ROUTED,
  NOT SHIPPED ... on two measured walls: per-mark variation beyond the spreads (backlog 0068),
  and the sanity gates themselves"* — **both walls are down** (Plan 0077 Phase 2, this plan, and
  Plan 0075's shell-occupancy rescue), so the header is a documented lie about the current
  engine. The rewrite is content work and rides Phase 5, along with whether the quiet world joins
  `emitter_perseids` or replaces it. **The ceremony's own grep did not catch it**: step 3b's
  pattern matches `design-backlog 00NN` and this header says the bare `backlog 0068` — widen it.

- [0094 — The two doc gates check what they claim
  to](done/0094-the-two-doc-gates-check-what-they-claim-to.md) — closed 2026-08-15, the day it
  was written and hours after the plan it repairs, all three `dev` phases (`38addde`, `a9f8c70`,
  `24c85dd`). Review: **no blockers, no majors, two minors, two nits** — and neither minor is
  against the implementation. **Both markdown gates now cover what they claimed to.** The link
  gate skipped any directory *named* `fixtures`, which silently swallowed `core/tests/fixtures/`
  and its three READMEs — twelve relative links, four of them into `docs/plans/done/`, which is
  precisely the set a close ceremony moves; the skip is now **by path**, enumerated once, so a
  second seeded tree has to name itself. The claim gate enforced the second half of
  [ADR-0108](../adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)'s
  Decision and not the first: its entry roster was built *out of the bullets it found*, so an
  entry with no bullet at all was invisible to it, and the 14 live entries complied only because
  Plan 0093 Phase 2 did it by hand. Live `## NNNN` headings are now collected separately and any
  without a dated bullet is a break at the heading's own line, with a message naming both ways
  out. **Every phase was re-verified at the close rather than taken on report**: the walk goes **249
  → 252** against 258 markdown files that exist, regaining exactly the three
  `core/tests/fixtures/` READMEs and still skipping exactly the six under `scripts/fixtures/`;
  `--self-test` 9/9 with the fixture pinned at 5 breaks / 2 probes / 7 live entries. **Phase 3's
  bug was reproduced, not argued** — on a `git clone --depth 1` the tip is grafted parentless, so
  `git log -1 --format=%cs` returns the tip date for every path: `core/src/dsp/mod.rs` reads
  2026-08-15 there against its real **2026-07-30**, which on the un-bypassable CI call site would
  have reported all 25 probed paths as moved on every run from the first one dated after the
  stamps, burying the `unprobeable:` roster that shares the block and is the whole reason it
  exists. The fix takes ADR-0016's shape — the reading is **withheld with a notice** rather than
  printed wrong — and a git that cannot answer falls back to today's behaviour rather than to the
  notice, because printing "cannot see the history" on a full clone would be a new lie in place
  of the old one. Verified both ways at the close: shallow clone gives the notice, no moved rows,
  the roster, exit 0; the full checkout still reports two real moved rows when entry 0032's stamp
  is backdated. `fetch-depth: 0` on the `links` job was rejected — a full-history fetch on every
  run for a block that never touches the exit code, leaving the gate honest only where a workflow
  was configured for it. **The one finding against the plan is the plan's own arithmetic**: Phase
  1's done-when said the walk "reads **257**, up from 248, with all nine previously dropped files
  accounted for", which counted the files that *exist* rather than the files the walk *reads* —
  six of the nine stay skipped by design. `dev` asserted the property instead, which the plan's
  own parenthetical authorises, and verified it by seeding a broken link under each of the two
  trees. That is the "do the arithmetic on every numeric done-when before the plan ships" rule
  failing at authoring time and being caught by the escape hatch written beside it. **Minors
  (both repaired in this close commit, neither in `scripts/`):** the architect skill's step 1c
  described a break as always naming a probe and the advisory as always naming moved paths, both
  now half-true; and `docs/design-backlog.md`'s preamble stated the carry-a-probe rule as prose
  without telling the entry author the gate now reds on them — which is the one reader Phase 2
  was built for. **Nits:** `check-backlog-claims.mjs:247`'s `if (bullet.entry)` guard can never
  be false (the field is coalesced to `"(outside any entry)"` one function up), and a `##`
  heading the `ENTRY` regex rejects does not reset the current entry, so a bullet inside a
  section preamble is attributed to the previous entry and would silently satisfy the new
  requirement for it — pre-existing, narrow today because preambles precede their entries. **Curation
  (step 3b):** `presets/` untouched, no engine defect fixed — neither sweep fires.

- [0093 — The backlog stops asserting things about a repo it has not
  read](done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md) — closed
  2026-08-15, the day it was written, all four `dev` phases (`ee471fb`, `7a975ad`, `0ab3331`,
  `9d8b1ff`). Review: **no blockers, two majors, one minor, two nits** — all three substantive
  findings are in the machinery rather than the plan, and all three are carried by
  [0094](done/0094-the-two-doc-gates-check-what-they-claim-to.md). **A backlog claim about the
  repo now carries a probe a script re-runs**
  ([ADR-0108](../adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)):
  `absent: <regex> in: <path>` / `present:` / a reasoned `unprobeable:`, resolved by
  `scripts/check-backlog-claims.mjs` with **its own file walker and no shell**, at the three call
  sites the link checker already occupies. 25 probes across the 14 live entries, 3 reasoned
  opt-outs, printed and countable at every push. **The non-vacuity assertion is the part worth
  keeping**: `--self-test` asserts permanently that the probe reconstructed from backlog 0082's
  own claim — `absent: sustained_miss in: core/src` — still **fails** against today's tree, which
  is the instrument proving without time travel that this gate would have caught the historical
  case on the day the governor landed (it bites at `core/src/diag/mod.rs:35`). Staleness is a
  printed advisory that never touches the exit code, and it named nobody on the tree as it stands
  — the honest consequence of Phase 2 having re-stamped all 14 entries hours earlier, stated in
  the commit rather than fixed by lowering something. **Phase 2 convicted backlog entry 0093 and
  `dev` reported it rather than repairing it**, which is the split the plan wrote down; the
  correction was made at this close. The shared fixture tree under `scripts/fixtures/` finally
  gave `check-doc-links.mjs`'s `root` argument its first committed caller — [0084]'s one recorded
  loose end — so both gates' bite checks are now repeatable commands rather than properties
  nobody has re-tested.

- [0085 — The show-length horizon gets an
  instrument](done/0085-the-show-length-horizon-gets-an-instrument.md) — closed 2026-08-15 (four
  `dev` phases: `3280136`, `a1e62e5`, `97b7227`, `9514e2b`; **the `human` Phase 5 was then run
  the same day — all five phases complete, see Standing**). Review: **no blockers, one major,
  three minors, one nit**. **The first instrument in this repo that measures past half a
  second.** `shot --horizon <minutes>` renders N *simulated* minutes at the fixed capture step
  and prints one row per interval — coverage, `peak/mean` concentration, footprint motion — plus
  a `delta`/`monotone` trend per statistic and no threshold anywhere
  ([ADR-0099](../adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md)).
  Both determinism properties are asserted on **rendered pixels through the CLI**, not on
  arithmetic: two runs of one request are byte-identical, and a 0.05-minute run row-for-row
  prefixes a 0.1-minute one — which is what makes a recorded header verdict worth anything. The
  non-vacuity half runs beside it, a `fade = 0.999` de Jong reading monotone 1.00 against a
  static star pattern reading `delta 0.0000` on all three statistics, **both fixtures written by
  the test** rather than pointed at shipped content. `--soak` gained **three** columns, not the
  two the plan's Data shapes named, and the correction is the plan's own fault: the done-when
  asked for "the two frame-time columns diverging" from a log that carried **no `p99` column at
  all**, so the raw statistic is appended beside `frame_ms_p99_steady` or there is nothing to
  diverge from. The exclusion is a **frame** count with a stated derivation and a test that
  measures the core's `pub(crate)` ring through the public `FrameStats` and fails if it ever
  outgrows the constant (measured 240, constant 300). **Two core additions the plan's file list
  did not allow, both approved at the Step 2 gate and both right**: `metrics::peak_to_mean`
  (ADR-0099 called it existing; it did not exist) and `Renderer::capture_preset_at`, the long-run
  primitive its two siblings cannot be — asserted **byte-equal to `capture_preset` at the same
  length on the software adapter**, which is the strict case for the readback-allocation hazard. **Phase
  4 found two stale premises and corrected them, and this is the load-bearing part.** R0 is **not**
  unbuilt — Plan 0044 / ADR-0045 shipped tiers and the governor on 2026-07-30 and this README
  said they "remain for a later plan" for six weeks — and **the shipped governor never reads
  `p99`**: `sustained_miss` needs 75 % of ≥180 samples past `budget × 1.25`, which a switch's
  handful of slow frames in a 240-sample ring cannot approach. So the hazard is real and lives in
  the **description**, not the code, and all three entry points a governor design starts from now
  say so. **The major is not against this implementation**: Phase 2 surfaced a **pre-existing**
  headless-capture frame ceiling — both reaction-diffusion worlds die at 3,601 frames with an
  invalid readback buffer after RSS reaches ~2.9 GB, with the shipped `capture_preset` run as the
  control before it was called a finding — filed as [backlog 0093](../design-backlog.md) with a
  candidate mechanism (`step_offscreen` submits per frame and never polls). Until it is fixed, "N
  simulated minutes" is bounded by world, and those two rows are 0.5 minutes rather than 10. **Minors:**
  `frame_ms_p99_steady` initializes to `0.0` and writes an impossible `0.000` if the first sample
  lands inside an exclusion window, against a doc comment that says it carries its last *trusted*
  value; `shot_cli.rs` asserts `monotone == 1.0` exactly on rendered output over three steps, and
  CI runs `shot` on WARP where it has never run (if it flakes, the fix is more steps, not a
  looser threshold); and this README carried a duplicated "The" at the roadmap item 3 seam. **Nit:**
  `REACTIVITY_FLOOR` is a mean-channel-difference floor reused as a coverage-fraction floor. **Curation
  (step 3b):** `presets/` was touched for one header and no values, and the plan fixed no engine
  defect, so neither sweep fires — but the header it added is the instrument's first conviction,
  `attractor_ink` drying out 0.199 → 0.002 over ten minutes with the silhouette intact, **recorded
  and deliberately not repaired**; judging it is content-lane work. The named subject
  `swarm_shatter` came back **clean** (monotone 0.50, wandering 0.197–0.384 with no trend), which
  the plan called the expected outcome.

- [0089 — The framing contract stops lying, and two doc gaps
  close](done/0089-the-framing-contract-stops-lying.md) — closed 2026-08-15 (three phases, one
  `dev` session: `e23bd04`, `d4570e7`, `52b1dc3`). Review: **no blockers, no majors, one minor,
  three nits**. **A stated invariant stopped being false without a pixel moving.** `FRAME_FILL = 0.88`
  documented that a fitted IFS figure sits inside the frame; the fit measures an *axis-aligned*
  box and `project` then rotates it at `spin`'s default of one revolution per 34.9 s, so only a
  figure at or under `sqrt(1/FRAME_FILL² − 1)` — about 1.85x taller than wide — stays inside at
  every angle. Measured over the roster from each figure's own `chaos_extent`: **only the fern
  complies** (`a = 0.4851` against the `0.5397` bound); sierpinski overruns by 34 %, tree 41,
  dragon 58, spiral 79 — which is why all three shipped 2-D IFS worlds independently carry a base
  `zoom` below 1. The new test asserts the closed form **against the shipped `fit_scale`** rather
  than a parallel arithmetic, derives every constant from `FRAME_FILL` (ADR-0071), derives its
  sweep tolerance from the sweep's own angular step, guards the knife edge against `f32`
  rounding, and is **non-vacuous in both directions**.
  [ADR-0103](../adrs/0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) accepted, with a
  dated `Outcome` correcting the plan's own arithmetic: horizontal binding is unsatisfiable at
  `aspect >= 1/FRAME_FILL = 1.136`, not at every `aspect >= 1`, and the whole derivation assumes
  a landscape target. **Two `dev` deviations, both correct** — Phase 3's shipped instance moved
  to the three `reaction_*` presets because `chthonic_coral_oracle.toml` had been retired three
  days before the plan was written (`d92dcb2`), and the fern's header says something different
  from the other two because the measurement says the fern is the one figure that *satisfies* the
  rotated bound. Phases 2 and 3 closed the two doc gaps that had each named a home and never got
  a carrier: `kaleido_tile`'s bindability and the clipped border cell, and the gain rule's
  exception class (**a param whose cap is a failure state rather than a maximum**, treated by
  pulling the range in at *both* ends, worked through Gray-Scott `feed`/`kill`). Curation (step
  3b): the plan touched `presets/` for three headers and no values, fixed no engine defect, so
  neither sweep fires — but the three headers move the other way, and the dragon's `zoom` stops
  reading as a workaround while the fern's and volute's stop reading as taste.

- [0088 — The docs get pictures](done/0088-the-docs-get-pictures.md) — closed 2026-08-13 (all
  seven phases, written and landed the same day; six `dev` commits plus the `human` Phase 7 look
  call **run at the close rather than carried forward**). Review: **no blockers, no majors, three
  minors, two nits**, all repaired in `5dda709`. **Eighty-eight plans of a real-time graphics
  project, and this is the first committed image of any kind.** Sixteen of them
  ([ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md)): nine
  gallery, one hero, six walkthrough, every one a 1280x720 `shot` render captured **under real
  audio** through the real analyzer and driven from an argument-free `scripts/docs-shots.mjs`
  whose manifest is the only record of what produced each file. The capability came first — `shot --frame-at
  <hop>` (`476a989`), because the filmstrip path scales every frame to a **363x208 bordered
  tile** and nothing in the tool could produce a full-resolution frame under real audio. Two new
  documents on top of the three references rather than merged into them
  ([ADR-0101](../adrs/0101-the-preset-docs-gain-a-tutorial-layer-rather-than-a-merge.md)):
  [`docs/preset-guide.md`](../preset-guide.md) and
  [`docs/preset-tuning-walkthrough.md`](../preset-tuning-walkthrough.md), and the
  one-fact-one-home rule **held under review** — the guide reproduces no parameter, function or
  palette table. **Two deviations, both argued and both right, and the first is this project's
  own arithmetic rule failing on the authoring side.** The plan specified capture hop **340**;
  `dev` moved it to **300** after re-deriving it against `core/src/signal.rs:144`, where `beat %
  8 → 6|7 => 0.04` puts the rest at hop 306.8 — so the plan's own number sat **34 hops inside
  `dynamic_groove`'s two-beat rest**, chosen from scene time and never checked against the
  phrase. The second was user-directed: Phase 5's subject moved from a `swarm` to a
  `fragment_field` mandala, because a still picture cannot teach `force`, `spin`, `field_freq`,
  `reseed` or `twinkle` and the five steps' method is family-agnostic. **The weight has two
  numbers and ADR-0100 conflated them**, which is the finding a later close must carry: the **tree**
  holds 16 images / **20,459,591 bytes**, but **history** holds 19 blobs / **25,489,457 bytes** —
  `hero.png` was written three times and `swarm.png` twice, and a superseded blob never leaves a
  repository that does not rewrite history. Both are inside the ≤ 22 images / ≤ 32 MB ceiling,
  but **the ceiling is about the history figure**, so a whole-set re-shoot costs its full weight
  again — recorded as a dated
  [Outcome](../adrs/0100-documentation-images-are-committed-headless-renders.md#outcome--2026-08-13-at-plan-0088s-close). **The
  close tested a done-when no phase could**: re-running the script after Phase 7's two manifest
  edits moved **exactly those two images** and left the other fourteen byte-identical — same
  machine and binary only, which is not evidence against the cross-adapter drift that keeps this
  out of CI. **Phase 7's verdict**: all ten committed pictures opened, eight of nine gallery
  picks stand, two swapped against alternatives shot for the comparison (`swarm_drift →
  swarm_shatter`, charcoal on black collapsing to a dark rectangle at README width; hero
  `fragment_supernova → fragment_tunnel`, a flat salmon field reading as wallpaper at the top of
  a front page). **`emitter_perseids` and `star_rosewindow` are accepted rather than good** and
  the hop is provably not the lever — both were re-shot at other hops with the same framing — so
  they went to [`docs/content-brief.md`](../content-brief.md) §5 as a **framing brief for the
  content lane**, each family shipping exactly one preset with nothing to swap to. **Curation
  (step 3b): no preset content landed** — `docs/examples/` is teaching material and never enters
  `presets/` — so no near-duplicate sweep was owed, and the workaround grep over `presets/*.toml`
  is unchanged by this plan. **Carried forward:** [0087] invalidates the curve family's gallery
  image and the hero, which is one script re-run and is named in the sequence section above.

- [0084 — Two gates stop lying about what they
  check](done/0084-two-gates-stop-lying-about-what-they-check.md) — closed 2026-08-13 (all four
  `dev` phases, written and landed the same day). Review: **no blockers, no majors, three minors,
  one nit**. **The doc-link checker sees markdown's second link form** — a use with no definition
  in its file, and a definition whose relative target does not resolve, both reported through the
  existing `file:line -> target` shape. **The narrowing that makes it usable was measured rather
  than assumed**, which the plan's risk section had asked for: a shortcut use is reported only
  when *some* file in the tree defines that label, without which the repo corpus yields 31
  findings of which 24 are ordinary prose brackets, and with which 7 and no noise. It found
  exactly the seven breaks the plan predicted and **proved itself again at this close** — the
  `git mv` into `done/` broke four inbound links and it named all four, one in the definition
  class it had just learned to see. **The capture path can advance without rasterizing**:
  `capture_audio_after_warmup` takes a count of leading hops to step the analyzer and the clock
  with no render pass, and `capture_audio` is that call with a warm-up of zero, signature and
  behaviour unchanged. Measured on this Windows box's DX12 software adapter (ADR-0071 — in a
  docstring, not an assertion): **136.3 s -> 100.2 s over 36 presets**, the superseded 86 s ->
  167 s pair kept and labelled pre-0084 rather than deleted, since it was taken on a 41-preset
  library. **The plan's real acceptance criterion failed and was accepted at the escalation, not
  absorbed** — its premise was half wrong, because the warm-up renders were also the *scene*
  warm-up, which `reactivity.rs` said in as many words. 35 of 36 per-band vectors moved, the
  exception being `spectrum/Halo`, the only preset in the set with no accumulating state; every
  maximum rose or held and the lowest across the library went 0.0287 -> 0.0504 against the 0.020
  floor, so the tightest headroom roughly doubled. **Read any reactivity figure recorded before
  2026-08-13 as a different measurement, not as drift.** **Minors:** Phase 1's fixture done-when
  left no committed artifact, so the script's optional `root` argument has no caller in the repo
  and the bite check is unrepeatable — which is the property the phase itself argued matters
  most, since a link checker that silently passes is worse than none; `docs/capturing.md` still
  named the gate's old capture call and its old ~1.8x conversion price in the exact paragraph
  that tells a future author how to copy this pattern, repaired in the close series; and the
  byte-identity test guards the property that was never at risk — the render pass structurally
  cannot reach the analyzer, which publishes before it skips at `capture_api.rs:321` — while the
  property that actually moved, GPU-integrated scene state meeting the measured window
  `WARMUP_HOPS` steps colder, is documented in three places and asserted in none. That last one
  is carried forward rather than fixed: **any gate copying this pattern onto an accumulating
  scene inherits the cold start silently**, which is why `docs/capturing.md`'s gate section now
  says so where the next author reads it. **Nit:** the use-class narrowing is deliberately blind
  to the mirror failure, where a definition block is deleted outright and every use of the label
  goes quiet — documented in the script header, and the right call against the 31-vs-7
  measurement. **Curation (step 3b): `presets/` untouched and no engine defect fixed — nothing
  owed.** No `aspect` in the diff, no platform or audio-source type entered `core/`, and the only
  new public item is `AudioCapture` on the capture path.

- [0083 — The build says why it hears nothing](done/0083-the-build-says-why-it-hears-nothing.md)
  — closed 2026-08-13 (all four `dev` phases, written and landed the same day; **Phase 5 is
  `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, one
  minor, two nits**. **The capture verdict becomes a value** —
  `CaptureVerdict::{Live,Failed,Unsupported}` in a new `standalone/src/capture_verdict.rs`,
  produced by all three `cfg` arms of `start_capture` through a `CaptureStart` struct that
  replaced a three-tuple, so **an arm that forgets to set one does not compile** rather than
  rendering as a success. It reaches the two artifacts a remote tester can actually send: a
  trailing `capture` column on every `diagnostics.log` row, and an `audio` line under the F3
  panel. **Both read one `String` built once at startup and borrowed thereafter** (`maybe_log`
  takes `&str`), which is what makes the two surfaces unable to disagree about a run *and* keeps
  the per-frame row builder allocation-free. **Why a column and not a startup line** is the
  plan's load-bearing decision and it held: the file rotates at 1 MiB keeping one backup and the
  tester's log spanned 6.5 days, so a line written once is exactly what rotation deletes — and a
  column also catches a capture that dies mid-run. **The Windows arm was in scope for a reason
  that is not symmetry** — nobody on this project can execute the macOS path, so building both is
  what got the mechanism tested and reviewed on the development box; the Mac arm differs only in
  which error type it formats. **The sanitizer is tested against a deliberately hostile message**
  (`" start failed:\tcode -3801\r\n\tat SCStream\n\n"` → `failed SCK start failed: code -3801 at
  SCStream`) rather than a real platform error, on the plan's own argument that a real one which
  happens to be clean proves nothing; an all-whitespace message renders `(no message)` so a row
  never trails off looking truncated. **The failed verdict deliberately carries no format** —
  both arms fall back to a hardcoded 48 kHz stereo so the analyzer has something valid, and
  reporting that would have the log state a format nothing is delivering; the constant is now
  named `FALLBACK_FORMAT` at one site instead of three inline literals. The frozen-prefix
  assertion in `diaglog.rs` was **widened rather than rewritten** (the fourteen pre-0083 names
  are the prefix, `capture` the appended tail), and the tests locate the field **by header
  position** rather than by index, so the next appended column moves nothing. Docs swept: both
  `packaging/*/READ-ME-FIRST.md` demote the Terminal relaunch from step 3 to a fallback, and
  `docs/on-device-validation.md` gains the column and an instruction to read the `audio` line *before*
  judging reactivity — flat band meters mean "capture failed" or "nothing playing", and every
  reactivity judgement below is worthless if it was the first. `docs/capturing.md` correctly
  untouched: it documents the `shot` CLI and the preset report, and has no `diagnostics.log`
  shape section. **Minor:** `standalone/src/overlay/tests.rs`'s `PANEL_BOTTOM` is hand-copied
  arithmetic over `core/src/render/overlay.rs`'s private constants, and its comment claims *"a
  panel that grows fails this deliberately"* — it would not; core's constants are private,
  nothing couples them, and the two agree at 240 px today only because someone transcribed them
  correctly. The check is worth keeping; the claim is the ADR-0071 prose failure one level down. **Nits:**
  the stale-header test's comment still says the rows carry "fourteen" columns (fifteen) and that
  the seeded stale header names "eight" (three) — pre-existing drift the sweep passed over; and
  `overlay::capture_line` formats a fresh `String` every frame the overlay is up, which matches
  what `queue_frame_text` already does around it and is why it is a nit rather than a finding. **Curation
  (step 3b): no preset content landed and no engine defect was fixed — nothing owed.** `core/` is
  untouched, no `aspect` appears in the diff, and nothing added runs on the capture thread: the
  verdict is known before the callback exists.

- [0079 — The attractor learns new figures: the tuple roster with per-tuple framing, and measured
  morph paths](done/0079-the-attractor-learns-new-figures.md) — closed 2026-08-13 (all six
  phases, **both `human` gates run and both producing a verdict rather than a default**). Review: **no
  blockers, no majors, four minors, two nits**. **A tuple becomes content, framing and all**
  ([ADR-0093](../adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md), closes
  [backlog 0055](../design-backlog.md) **in full — both halves**): each map family carries a
  curated roster whose entries hold their coefficients *and* a **measured** projection + seed
  box, selected by a CPU-quantized `tuple`. The wall the entry named is gone — the rho ≈ 100
  Lorenz that Plan 0075 cohort 5 measured as physically unreachable (centred on `z ≈ 102` against
  the canonical framing's `25`, spanning twice its extent) renders centred and in frame. **"Zero
  baselines move" is structural rather than argued**: entry 0 *is* the pre-roster literals, and
  `roster_entry_zero_is_the_canonical_tuple_unchanged` spells them out so a tidying refactor
  fails loudly instead of as a golden diff. **ADR-0037 is unreachable here by construction** —
  the per-entry scale is a *ratio* against the canonical figure's own extent, so aspect handling
  is bit-for-bit whatever entry 0 already did, and no `aspect` appears in the diff. The Plan 0062
  coupling survives *by derivation*: `jitter_extent` hangs off the entry's own box, asserted
  twice — as a fraction (`kick / half == JITTER_FRACTION`) and on the GPU as a measured mean
  `|dy|` matching the entry's prediction while **failing** the canonical framing's, which is the
  non-vacuity half. **The curation kept all 50 candidates** (*"honestly I love them all"*) after
  a four-per-family shortlist was drafted and **rejected** — judged in motion in the app, not off
  the sheets, because a still freezes one instant of a rotating figure. **The morph half's
  accepted research risk did not materialise**: of twenty swept pairs, four were refused *by
  measurement* before any eye reached them (a mid-walk tuple can collapse to a fixed point, which
  has zero extent and no scale to render at — all four on the discrete maps), four were judged in
  motion and ship as presets, and twelve strips are recorded as **rendered but unjudged** rather
  than waved through. The finding worth carrying: a walk holds where a roster steps a **single**
  coefficient (Thomas's `a`, Lorenz's `rho`), because there neighbouring entries are neighbouring *figures*.
  Three things beyond ADR-0093 are in its dated
  [Outcome](../adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md#outcome--2026-08-13-at-plan-0079s-close):
  framing alone was **not enough** (a measured entry seeds from the 4096-point on-attractor bank
  its own measurement collected — ADR-0087's IFS argument extended to a figure with no
  closed-form fixed points; without it rho ≈ 100 wanders to **2.2x its own extent** for seconds),
  the walk drives the **existing** `morph` param rather than a new one (so the param surface grew
  by the +1 the ADR budgeted, with a tuple path on an IFS a load error), and the roster's real
  costs (51 tuples of maintenance; ~3.7 ms per entry measured at preset load in debug, never per
  frame). Determinism held to the GPU by differential, not by reading:
  `the_cpu_step_mirrors_the_shader` runs both and compares, and
  `the_ode_substeps_agree_between_rust_and_wgsl` pins the constant a measurement would otherwise
  silently diverge on. **Minors:** the content lane's own `references/systems.md` had not learned
  `tuple` — the *identical* minor from Plan 0078's, 0080's and 0081's closes, and load-bearing
  because this plan's Followup is that lane binding it (repaired in the close series, table row +
  walk note); the two new `scripts/tuple-{sheets,paths}.mjs` had no operator-doc home (repaired
  in `docs/capturing.md`, with the caveat their output lives under gitignored `target/`); and two
  scope drifts recorded in the plan header rather than absorbed — **eleven presets shipped
  against the plan's own "does NOT do"** (user-approved as each landed, and ADR-0081 makes it
  legal without a plan, so a deliberate widening), and `presets/README.md`'s `tuple` row landed
  at Phase 1 rather than Phase 4 because the doc gate runs immediately and leaving it red across
  a `human` gate was not acceptable. **Nits:** `select_tuple` returns a `bool` nothing reads;
  `roster_len` widens the rlib's public surface (justified in place, C ABI untouched). **Curation
  (step 3b):** the attractor family went 6 presets → 17 of 37, **46 % of the library on one
  system** — the sharpest single-family convergence the set has seen, and the number to weigh
  before more attractor content lands. `attractor_dejonggallery` and `attractor_cliffordgallery`
  are near-twins by construction (identical `tuple`/`brightness`/`fade`/`reseed`, differing only
  in family and palette); all four galleries step on a **wall clock** (`mod(floor(time * 0.33),
  N)`) with audio only on secondary levers, which makes them demonstrations of the roster rather
  than worlds. That is a judgement for the content lane, not a re-tune here. The workaround grep
  over all 37 headers finds **nothing** citing the tuple wall — expected, since nothing could
  express a workaround for a figure that could not be reached at all.

- [0081 — The sky gets a galaxy: the backdrop paints a curved
  band](done/0081-the-sky-gets-a-galaxy.md) — closed 2026-08-12 (all five `dev` phases; **Phase 6
  is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors,
  two minors, two nits** (minors, both repaired in the close series: `docs/capturing.md`'s *"One
  golden baseline is lit, and it is the exception that proves the rule"* was falsified by this
  plan's own fixture — there are now two, which is the same doc drifting in the same paragraph
  that Plan 0080's close repaired; and this README's standing baseline-drift control still said
  "27 baselines" against a directory holding 28. Nits: `backdrop_ramp.rs`'s `0.25`-row bounds on
  the straight-band spread and the bow's shear are the only two thresholds in the new suite
  without a stated derivation, and the tighter of them has 1.6x headroom over its measured 0.16
  in a statistic the dither now feeds; and the shader's `select` evaluates both arms, so a
  backdrop with no band pays two extra LUT fetches per pixel where an `if` would give
  byte-identical output and skip them). **The backdrop paints one soft curved band**
  ([ADR-0095](../adrs/0095-the-backdrop-paints-a-curved-band.md)): seven params —
  `bg_band_amount`/`bg_band_angle`/`bg_band_pos`/`bg_band_width`/`bg_band_curve`/`bg_band_hue`/`bg_band_hue_span`
  — drawn **additively over the ground and under the scene**, which is what unresolved starlight
  is, so the four-role look (ground, band, stars, figure) becomes authorable without widening
  [ADR-0090](../adrs/0090-a-preset-composes-two-scene-layers.md)'s one-`[layer]` cap. **Every
  default is an arithmetic identity and it was proved three times**, once per code phase, by the
  bless-to-bless control (bless twice on the branch differing only by reverting the change —
  never a `git diff`, since eight baselines drift from their committed bytes on this box anyway).
  The identity is **structural rather than arithmetic**: `bg_band_amount = 0` takes a `select`
  arm, so the pre-band expression is the *untaken* branch and the inertness test asserts `worst
  == 0` over the whole frame with every other band param bound off its default. **The widened
  build condition (`bg_bright > 0 || bg_band_amount > 0`) is the plan's one-line change and it
  has its own test** — a band over an unlit ground, the only thing in the suite that can see it,
  with the `amount = 0` companion coming back the plain black clear. Three axes now share **one**
  `axis_pos()` and both palette fetches one `palette_at()`, deliberately so ADR-0037's trap
  cannot be fixed in one axis and left in another. **Two plan-accuracy findings, both recorded by
  `dev` rather than absorbed, both in [ADR-0095's
  Outcome](../adrs/0095-the-backdrop-paints-a-curved-band.md#outcome--2026-08-12-at-plan-0081s-close):**
  (i) the plan, this README's roster row and the ADR all framed the along-band normalizer as *not*
  cancelling at the default angle, making Phase 2 the sole possible sighting of ADR-0037 on that
  axis — **it cancels** (numerator `ndc.x * aspect`, denominator `aspect`), confirmed by
  re-running the bow measurement with the aspect forced to 1.0 and reproducing every digit; what
  Phase 2 actually catches is a wrong normalizer *form*, verified to bite (dropping the aspect
  from the denominator alone shears the arc 1.36 rows and fails the first edge assertion), and
  the trap itself is one property across all three axes because they read the single aspect the
  pass is handed. (ii) ADR-0095's table ("its own coordinate in the same `[palette]`", absolute)
  and the plan's Phase 3 done-when ("leave the band on the ground's own coordinate", an offset) **cannot
  both hold**; `dev` raised the fork and **the user chose absolute**, on the authoring argument
  that the ground's coordinate varies along the band's path, so an offset would drag the ramp's
  sweep into the arc. **Every numeric done-when is a differential, never a magnitude.** The `1/e`
  half-width goes into the *control* — a flat band at the upper width rail carrying `amount/e` —
  because the tonemap and the sRGB encode sit between the envelope and the 8-bit write, so an
  asserted ratio of 0.368 would be a claim about the tonemap's shoulder; the two frames agree to
  1 level at the crossings and differ by 21 at the centre. The bow is *located* column by column
  with a **luma-weighted centroid rather than an argmax**, a direct consequence of [0082] landing
  first: a gaussian's peak is flat, `pos = 0.5` puts the centre between two pixels, and the
  dither's ±1 LSB makes argmax report rows 31, 32 and 33 for three columns of one straight band.
  The fifth `EXTRA_FIXTURES` entry is **appended, never inserted**, so no pre-existing baseline
  is rendered from different device state, and **its seven values were tuned against the suite's
  own tolerance rather than for looks** — each reverted to its default in turn and re-measured,
  all seven clearing `MEAN_TOL = 0.02`, the tightest by 1.1x; two needed the tuning (a wide
  `bg_band_width` scored 0.0055 on its own revert, and the colour pair fights itself through the
  repeat addressing). Blessed only after comparing adapters, which the `48 → 80 B` uniform growth
  owes: WARP `152.200 120.540 086.088` against hardware `152.198 120.550 086.114`, 0.026 of one
  level, and the other 27 baselines restored to their committed bytes after the un-scoped
  `LMV_BLESS`. The ADR-0058 enumeration and `shot --report`'s generic binding walk were **confirmed,
  not edited** — a changed answer would have been the finding. Phase 5's sweep of
  `.claude/skills/preset-author/references/**` was a **done-when rather than a reviewer's
  catch**, which is the fix for the identical minor raised at both Plan 0078's and Plan 0080's
  closes; it landed. **Curation (step 3b):** no preset content landed — only `presets/README.md`
  and `docs/preset-palettes.md` — so no near-duplicate sweep owed; the workaround grep over all
  27 headers finds **nothing** citing a missing band or a painted-in galaxy, which is expected,
  since nothing could express the shape to work around.

- [0082 — The gradient stops banding: the display write
  dithers](done/0082-the-gradient-stops-banding.md) — closed 2026-08-12 (four `dev` phases plus a
  self-repair, and the `human` Phase 5 **answered rather than left standing**). Review: **no
  blockers, one major, five minors, three nits** — and every finding is a consequence of
  something the *plan* got wrong, recorded honestly by `dev` rather than absorbed. **The tonemap
  dithers** ([ADR-0096](../adrs/0096-the-display-write-dithers.md)): ±1 **encoded** LSB of TPDF
  noise from an integer hash of the pixel coordinates, divided by the sRGB transfer function's
  local slope because `Rgba8UnormSrgb` means the *hardware* encodes after the shader. One site,
  always on, not a param, no time term. The dusk ground's dark tail went **7.5 px/level and a
  58-px plateau → 2.1 and 20**, wide plateaus 17 → 3, still 0 % rail-pinned; the user's by-eye
  verdict on the held frame was *"looks fine"* on both halves, which **retires ADR-0096
  Alternative F** (the animated dither) as a followup. **The major is the ADR, accepted with a
  dated
  [Outcome](../adrs/0096-the-display-write-dithers.md#outcome--2026-08-12-at-plan-0082s-close)
  that falsifies two of its claims.** (i) Its "three parts, each load-bearing" is **four**: the
  dither must **fade at the rails**, which the ADR never mentions — at a rail the value is
  already exactly representable and the write clamps, so half the noise is discarded and what
  survives is a **DC lift**, and an exactly-black frame came back at mean **0.18/255** over a
  suite where nearly every fixture runs `bg_bright = 0`. Caught by two *existing* guards (the
  emitter burst test at lead peak 0.1827 where it asserts empty, and bloom roundness), not a new
  one; `dither_offset`'s fade is provably inert at and above code value 1, because below the knee
  the slope is the exact constant 12.92 so `min(l, 1-l) * slope * 255` **is** the encoded byte
  value. (ii) Its third Positive consequence — the headline argument that an integer hash buys **byte-for-byte**
  adapter agreement, sharper than the 0.02 drift floor — is **false**. The hash is exact (65 536
  float values, zero differing bit patterns), but the **hardware sRGB encode downstream of it**
  is not: DX12 permits tolerance in float-to-sRGB8 and WARP's approximation departs from the true
  curve below ~byte 20, so **212 of 2 049 408 re-blessed channels move by 2**. The integer hash
  still earned its place (Alternative C would have diverged on essentially *every* pixel), but
  the promised instrument does not exist. **Three of this plan's own numeric done-whens were
  wrong**, which is the architect-side lesson: "the golden suite goes red here" — it did not, the
  guard is a **tolerance** guard and a one-level shift reports 0.0007-0.0013 against 0.02, three
  orders of magnitude inside, so Phase 2 was a deliberate **re-pin** rather than the repair of a
  red build; "a delta of 2 anywhere is a finding" — bounded-by-one is a **hardware** claim and
  the shipped assertion reads its bound off `is_software()`; and the TL;DR's "hairlines", which
  the honest 20 px replaces. **The first deliberate full re-bless in the project's history**
  landed alone in its own commit, measured **bless-to-bless** (8 of the 27 rewrite against their
  committed bytes on a clean local bless, so a `git diff` would have charged that drift to the
  dither). Verified at the close rather than taken on trust: golden 27/27 green, `backdrop_ramp`
  6/6 so the `<= 1` tolerances survived, all four byte-equality tests pass including Plan 0075's
  `depth_fade` no-op against a live Lorenz control — the property that made *static* the right
  choice — and both dither tests reproduce their recorded numbers exactly. Two scope calls at the
  Step-1 gate, both right: `mix32` / `unit01` promoted to `gpu::HASH_WGSL` (the shared home
  [0077]'s close asked a *third particle scene* to build, arriving instead from the display
  write), and Phase 3's guard placed **in-crate** because an integration test cannot reach a
  `#[cfg(test)]` field and a public off-switch is exactly what Alternative G rejects. No ADR-0058
  entry was owed — the amplitude reuses the `Ctl` uniform's already-zero `.z`, so the layout
  shape is unchanged. Minors repaired in the close series: `docs/on-device-validation.md` had not
  learned the dither (three `pow` per pixel on a fullscreen draw, never measured on weak
  hardware; and the grain verdict was taken on **one display**, where a 6-bit + FRC panel running
  its own temporal dither is the case it cannot speak to), and the two "before" figures for the
  dusk probe disagree 2.3x with the scan axis unrecorded — written down as a **stated
  discrepancy** rather than an invented explanation. Nits: the corrected WARP mechanism's
  disproof lives in prose only and nothing asserts it; both dither tests request the software
  adapter so the tight hardware bound never runs automatically (fine — the derived-1/3 mean is
  the adapter-robust guard); and `core/Cargo.toml:31`'s last surviving mojibake, repaired. **Curation
  (step 3b):** no preset content landed — only `presets/README.md` — so no near-duplicate sweep
  owed; the workaround grep over all 27 headers finds **nothing** citing banding or a
  step-breaking stop, because no shipped preset binds the ramp params yet.

- [0080 — The sky gets a horizon: the backdrop paints a directional
  ramp](done/0080-the-sky-gets-a-horizon.md) — closed 2026-08-12 (all six `dev` phases; **Phase 7
  is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors,
  three minors, one nit** (minors, all repaired in the close series: the `preset-author` lane's
  own `references/systems.md` and `craft.md` did not know the ramp exists — the Plan 0078
  `ink_gamma` minor repeated verbatim, and load-bearing because Phase 7's followup *is* that lane
  authoring a ramp world; `docs/capturing.md`'s "every golden baseline runs `bg_bright = 0`" was
  falsified by this plan's own fixture, the suite's **first lit golden baseline**; and this
  README's standing baseline-drift control still said "8 of 20" against a directory holding 26.
  Nit: `backdrop_ramp.rs:273`'s `+ 100.0` reversal margin is the one threshold in the new suite
  without a stated derivation, in a suite otherwise exemplary on
  [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)). **The
  backdrop paints a directional ramp**
  ([ADR-0094](../adrs/0094-the-backdrop-paints-a-directional-ramp.md), closes backlog 0091, a
  user-rejected workaround rather than speculation): five params (`bg_angle`, `bg_hue_span`,
  `bg_shade`/`bg_shade_end`, `bg_ramp_gamma`) turn one palette *sample* into a *segment* swept
  along one axis, with the hardcoded `mix(0.72, 1.0, ndc.y)` tilt **retired into** the shade ramp
  so there is one brightness ramp on the frame rather than two, and horizon placement authored by
  the `[palette]` stops' own `at` positions — no second placement mechanism. **Every default is
  an arithmetic identity, and that was proved four times**: all 26 pre-existing baselines came
  back hash-identical under a bless-to-bless control once per code phase (bless twice on the
  branch, differing only by reverting the change — never a `git diff`, on this box eight drift
  from their committed bytes anyway). **ADR-0037's trap was instrumented, not argued.** At
  `bg_angle = 0` the aspect term *provably* cancels (`d = (0,1)`, denominator `aspect * 0 + 1`),
  so no default-angle test anywhere could tell a right source from a wrong one; the control runs
  at π/4 on a 160x100 target **with `trails` active**, because with an empty chain `target.size` **is**
  `surface` and the control would be theatre — at that size the internal grid quantizes to a
  square 256x256, so the wrong source is aspect 1.0 exactly against the surface's 1.6. It was
  verified to **bite**: `composite_into` was temporarily re-pointed at `target.size` and the test
  failed on its *first* assertion at 20 levels, which is ADR-0037's symptom (turning a stage on
  changes the shape of the picture) stated directly. The asserted 99-column crossing is derived,
  not measured — Δndc.x = 2/A = 1.25, times (1 − 1/H) for the edge rows' half-pixel inset, times
  W/2. The exponent reuses [ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md)'s
  shipped `select(pow(s, g), s, g == 1.0)` where the branch is a **correctness requirement**
  (`pow(x, 1.0)` is `exp2(1.0 * log2(x))`, not bit-exact), and its test asserts *placement* as
  well as agreement — `e = 0.5` sits at `s = 0.5^(1/g)`, giving rows 15.0/31.5/52.2 over 64,
  measured 15/15, 32/31, 52/52; agreement alone would have passed with the exponent inert on both
  channels. The 32 → 48 B uniform growth needed no ADR-0058 entry (the enumeration records **whether**
  a `min_binding_size` is declared, deliberately not which) and the adapters agreed to **0.044 of
  one 8-bit level** before blessing. Two `dev` findings recorded rather than absorbed: the plan's
  "20 baselines" is 26, and `shot --report` needs no per-namespace list because it walks bindings
  generically (verified with a probe binding four `bg_*` names, one live gate and one dead). **Curation
  (step 3b):** no preset content landed — only `presets/README.md` — so no near-duplicate sweep
  owed; the workaround grep over all 27 headers finds **nothing** citing the missing gradient
  this plan supplies, which is expected: the rejected workaround was never landed.

- [0078 — The ink learns to bite: a contrast exponent on the terminal
  remap](done/0078-the-ink-learns-to-bite.md) — closed 2026-08-12 (both `dev` phases; **Phase 3
  is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors,
  three minors, one nit** (minors: Phase 1 substituted a structural argument for the
  bless-to-bless control its done-when named — the substitution is the *stronger* instrument and
  is recorded as such, not repaired; `.claude/skills/preset-author/references/systems.md`'s ink
  row did not know `ink_gamma` exists, repaired in the close series and load-bearing because
  Phase 3 is that lane retuning against that table; two shipped preset headers now describe as
  forced a workaround the engine no longer forces, which is Phase 3's roster rather than a fix.
  Nit: the mid-band mean's `+ 1.0` byte margin sits below `golden.rs`'s own `0.02`-normalized
  drift floor for a mean statistic — defensible here because it is a lower bound on 12-33-byte
  gaps through an *injected* deterministic ramp, not a rendered scene, and the tighter per-level
  `<= 1.0` tolerance has been measured on WARP). **`ink_gamma` lands**
  ([ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md), backlog 0084, hit 3x
  across two Plan 0075 cohorts): `mix(paper, ink, luma^g)`, endpoints invariant *by arithmetic*
  rather than by tuning, so the paper never moves at any value. **The default's exact identity
  had to be built, not inherited** — `pow(x, 1.0)` is `exp2(1.0 * log2(x))` and not bit-exact, so
  the shader takes an explicit `g == 1.0` branch; without it the zero-baseline sentence would
  have been false by a rounding step. **And the zero-baseline claim is structural in a stronger
  sense than the plan argued**: no golden fixture binds `ink_amount` at all and the stage builds
  its resources only when `active()`, so no committed baseline ever constructs the ink pass —
  which also covers the one thing a param grep cannot see, the `COPY_DST` flag added to `ink-src`
  for the endpoint test (the arrangement `tonemap-src` already carried). Endpoint invariance is
  asserted through the shipped WGSL across `g = 0.25/0.5/1/2/4` **and** over hostile values (0,
  negative, NaN, ±∞) on the CPU mirror; the test injects a 256-step ramp because no rendered
  scene reaches key 1 (ADR-0046's shoulder is bounded strictly below it). Two `dev` judgment
  calls the plan did not specify — the CPU-side crossfade of `gamma` alongside `ink_amount`, and
  the finite `0.05 .. 20` clamp — are recorded in ADR-0092's
  [Outcome](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md#outcome--2026-08-12-at-plan-0078s-close),
  which falsifies nothing in the ADR. The published mean-byte ladder (`147 / 180 / 209 / 229 /
  241`) reproduces independently from pure sRGB-to-linear arithmetic (`147.1 / 180.4 / 208.6 / 228.6
  / 241.0`) — a property of the math, not of a rig, which is why it is publishable without naming
  one. Two plan-accuracy drifts caught and recorded by `dev`: `schema.rs` needed no edit
  (`GLOBAL_PARAMS` aggregates `ink::PARAMS` by reference — verified, there is no second roster),
  and two unlisted files were required. **Curation (step 3b):** no preset content landed, so no
  near-dup sweep owed; the workaround grep names `reaction_etching` and `swarm_shatter`, both
  carried into the Standing entry.

- [0077 — The quiet sky: the sparse idiom becomes gateable and the swarm
  individuates](done/0077-the-quiet-sky.md) — closed 2026-08-12 (`dev` scope; **Phase 5 is
  `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, two
  minors** (both repaired in the close series: `docs/capturing.md` had not learned the report's
  new footprint block / `reactivity_footprint` JSON key — the operator-doc sweep `dev` correctly
  left for the close; and the gate's deliberate semantic change — backdrop-only drift no longer
  counts as animation — lived only in the test until ADR-0091's Outcome recorded it) **and two
  nits** (`report.rs`'s "the reading never sits below the mean" is a practical-regime claim, not
  a theorem — sub-`eps` differences on unlit pixels leave the numerator while the denominator
  shrinks; and the emitter's `unit` hash is now mirrored verbatim into `swarm.rs`, a deliberate,
  commented duplication that a third particle scene should promote to a shared home). **The
  sparse idiom becomes gateable**: the animation gate scores `metrics::footprint_diff` — the
  masked form, chosen over the quotient with the reason recorded on the function — with every
  constant carrying its derivation (floor at half the shipped minimum; the 139-pixel mask floor
  capping a one-pixel flicker at 0.0072), the `bg_*` strip re-learning ADR-0067 by measurement
  (backdrops on, the sparse probe's footprint read 65 % of the frame), the rejected fifth-density
  Squall draft **passing at 0.1049** where the whole-frame statistic priced it out at 0.0057, the
  static control failing on a zero numerator, both pinned as a standing non-vacuity test, and the
  whole-library sweep convicting nothing. **The swarm individuates**: `twinkle`/`size_spread` off
  the particle's index through the emitter's unit hash — deliberately not `SeededRng`, whose
  extra stream draw would re-scatter the field — exactly 1.0 at their zero defaults so the
  goldens pass unblessed, with the shimmer-without-breathing bound derived from the mechanism (`8 *
  TWINKLE / sqrt(N_visible)`, 16x under the sheet-flash signature). **The swarm gains `reseed`**
  with ADR-0066's disturbance semantics (±6 % domain-relative kick, never a box respawn),
  catching a live defect class en route: resetting `prev_reseed` in per-frame `reset_params`
  turns a held gate into an edge per frame (measured diverging, 105 % coverage gap at 10 s) — the
  omission is now commented on both scenes. **The report sees bloom** (backlog 0088): the mean
  columns stay untouched and a footprint reading lands beside them at zero extra GPU cost — the
  bloom-only fixture reads bass 0.161 against the mean's 0.004, unbound bands stay 0.000 in both
  readings, and the `flash`-lever house workaround is obsolete. Plan drift recorded honestly by
  `dev` in the phase commits (no `schema.rs` edit exists to make; the report machinery lives in
  `report.rs` since Plan 0061). **Curation (step 3b):** no preset content landed, so no near-dup
  sweep owed; the workaround grep lists two headers for the content lane — `fragment_vitrail`'s
  "report is bloom-blind" rationale (fixed by Phase 4) and Perseids' routed-out quiet sky (Phase
  5's own subject) — named in the Standing entry.

- [0075 — The content renaissance: the library is rebuilt as worlds, by replacement
  cohorts](done/0075-the-content-renaissance.md) — closed 2026-08-11. Review: **no blockers, no
  majors, two minors, two nits** (minors: rustfmt drift on two test files the lane touched,
  repaired in the close series as `6a5a9c6` — the "557/557 green" handoff claim was nextest,
  which does not check fmt, and the fmt-running pre-push hook never fired because the lane never
  pushed; the roster row's "the library is 28 worlds" against a measured 25 after cohort 5, moot
  with the row's deletion here. nits: `standalone/src/shot/report.rs` reaches the extent
  diagnostic through the deep `lmv_core::render::scenes::lines::renderer` path — a `render`-root
  re-export would keep the shell at arm's length; Phase 2's "Files touched" named
  `standalone/examples/shot.rs`, which Plan 0061 Phase 4 had already moved — `dev` caught and
  recorded the drift in the phase commit). **R6 lands: the library is rebuilt as 27 worlds — the
  brief's 9 keeps plus 18 authored fresh-slate — through six family cohorts, each landing its
  worlds through the [0067] route and retiring its named roster in the same series** (45 → 27,
  ADR-0089's mechanism held: the set was never hollow, the gates never went vacuous, and every
  cohort was judged live by the user before its retirements committed). Phase 1 ended the sanity
  floor's selecting-for-the-defect: `metrics::radial_shell_occupancy` (ten annuli over the
  inscribed disc) rescues a preset under its coverage floor at ≥ 4 occupied shells — the three
  retired ring mandalas at their honest tunings (frozen byte-for-byte, the backlog's exact pinned
  numbers 0.2442/0.2505/0.2544) read 10/10/9 shells, the frozen renders-nothing defect reads 0
  and still fails, and every constant states its derivation (ADR-0071). Phase 2 made `depth_fade`
  an exact no-op on flat families — asserted by **byte equality** with a live Lorenz control so
  the no-op cannot pass vacuously — recorded as ADR-0076's second dated
  [Outcome](../adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md#outcome-added-at-plan-0075s-close-2026-08-11);
  and the in-frame geometry fraction joined `shot --report` as the `geom` column, printed exactly
  where a line seam exists (JSON mirrors the omission). Phase 3 landed the measured depth-lever
  corrections (the `perspective` orbit and its ~0.3 ceiling, `depth_hue`'s three regimes, the
  `spin`×`fade` smear ceilings) in `presets/README.md` and `docs/preset-palettes.md`. Cohort 6
  shipped the library's first two layered worlds (Vitrail, Sumi) on [0076]'s `[layer]`.
  Retirement commits froze the test fixtures they orphaned (Star Rosette's ladder source, the
  honest mandala tunings) rather than leaving dangling `include_str!`s. Engine feedback routed
  out as designed: backlog 0084–0089 plus re-raises, promoted to
  [0077](done/0077-the-quiet-sky.md)/[0078](done/0078-the-ink-learns-to-bite.md)/[0079](done/0079-the-attractor-learns-new-figures.md),
  nothing absorbed into the plan. Suite 665/665 after the merge with `main`; fmt + clippy clean. **Curation
  (step 3b), from a fresh `--report` run over the final 27 at this close:** zero near-duplicates
  below shape 0.08 in all nine families, every gate branch taken under the 110 BPM probe, no
  clamp saturated, `occ` 0 across the set; the workaround grep finds no header citing an
  already-fixed defect — three cite approved-but-unbuilt fixes (Perseids → [0077], Shatter's
  rebuild → [0077], Etching's duotone → [0078]), each already named inside its fixing plan. No
  curation action owed; the set ships as authored.

- [0076 — The second layer: a preset composes two scenes (R3)](done/0076-the-second-layer.md) —
  closed 2026-08-11. Review: **no blockers, no majors; one minor** (Phase 2's commit message
  attributed the memory measurement to WARP when it ran on the hardware adapter — corrected of
  record inside Phase 4's own commit, nothing left to fix) plus close-time roster/link staleness
  repaired in the close commit. **R3 lands: a preset composes a second scene through one optional
  `[layer]` table**, joined `under` (same target, one extra draw, one substance) or `over` (own
  offscreen, linear-light blend between kaleidoscope and bloom —
  `add`/`screen`/`multiply`/`overlay` fixed at load, `mix` bindable). **Per-preset scene
  instances ended the one-instance-per-system roster** (the user's call in ADR-0090), and the
  Phase 2 discovery is recorded where it was found: a shared `LineRenderer` is *not* shareable
  between two live line draws (`Queue::write_buffer` applies before the submission's passes), so
  a layer line scene carries its own. **Layerless presets are byte-identical by construction and
  by count** — backdrop + scene + tonemap is still exactly 3 draws, and `mix = 0` is
  byte-identical through both junction positions. The routing junction stays a pure function,
  unit-enumerated over all eight active-flag combinations without a GPU. Reachability, saturation
  and the report walk `[layer]` bindings under their own namespace; a dead layer gate flags.
  Measured on this box, stated as measurements (ADR-0071): attractor+RD ~11.9-12.9 ms/frame at
  1080p Floor, layered RD pair +303 MB peak working set (debug/WARP; the release-hardware
  expectation is the ~33 MB texture arithmetic), layered gate fixture +0.9 ms/frame. Two golden
  fixtures pin both joins, adapter-compared before blessing (WARP vs hardware mean 0.0002). WARP
  aliases the same-system pair's identical layouts, so the independence guard runs on hardware
  and skips-with-notice on software (ADR-0058 posture). The Phase 5 verdicts: pre-bloom `over`
  reads as intended, all four modes ship under their names, and the fullscreen-`under`-occludes
  finding became authoring guidance in `presets/README.md`'s new `[layer]` section. No shipped
  preset declares a layer yet — that is [0075] cohort 6's work, now unblocked. Curation (step
  3b): no preset content landed and no engine defect was fixed, so no workaround sweep was owed;
  verdict "no content change".

- [0053 — The suite stops blessing what WARP gets
  wrong](done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) — closed 2026-08-09. Review: **no
  blockers, two majors, four minors**. **The plan was written to prove the collisions benign and
  instead found two live mis-renders and fixed them.** `background-bind-layout` collided with the
  fullscreen scenes' single uniforms and rendered the fragment field a **flat grey** on WARP
  (`142.712` on all three channels against hardware's `131.010 170.559 141.381`);
  `blend-bind-layout` collided with `trails-bind-layout` and rendered a dissolve between two
  `trails`-binding presets wrong (`08.999 38.682 45.794` against `17.572 48.211 50.116`). Both
  fixed by an explicit `min_binding_size`, each isolated against a no-collision control that
  agrees to **0.02 of one 8-bit level**. That makes ADR-0058's "nothing is observed to be wrong"
  and "it fixes nothing that is currently broken" **false**, recorded in its
  [Outcome](../adrs/0058-bind-group-layout-collisions-carry-evidence.md#outcome--2026-08-09-at-plan-0053s-close).
  Nine colliding pairs became **four** allowlisted, each carrying a measurement; the shape the
  test keys on widened from binding *kinds* to kinds + visibility + whether a `min_binding_size`
  is declared, the first forced by ADR-0058's own boxed note and the second **measured here,
  twice independently**. Guard confirmed in the reverted direction at the review: dropping either
  fix back to `gpu::uniform` re-collides its pair and fails. **Zero baselines moved** — a
  bless-to-bless control differing only by reverting the two fixes came back hash-identical
  across all 20 PNGs. The line seam's lit-backdrop guard went from **15 channels** of reach to
  the whole stroke footprint (779 of 28 173 post-fix against 28 178 pre-fix) via a fourth capture
  at zero emitted light, and the swarm took the same arm. **Two majors left as findings, both
  consequences of the fix rather than defects in it:** the allowlist's `RIG` names neither
  adapter, in a repo whose ADR-0074 `Outcome` established five days earlier that this box's WARP 10.0.19041
  is the outlier and CI's 10.0.26100 is not — so four `AGREES` entries grant permission on a
  build never measured; and `core/tests/background_composite.rs` still skips on every software
  adapter citing the quirk this plan **fixed**, so a check CI has never run may now be liftable
  (unmeasured — the attractor half is a different layout group). **Phase 3 is `human` and was run
  by `dev` under the user's explicit authorization at the gate.**

- [0046 — Transformed feedback: the past learns to move](done/0046-transformed-feedback.md)
  — **done 2026-08-09**, Mode 4 review **no blockers, no majors, four minors, two nits**. Five phases
  as `f2e6ed6` / `24f4bfc` / `0816516` / `429396d` / `16802ae` (the Phase 5 verdict), in the
  `lmv-plan-0046` worktree. Full gate re-run at the close **after `git merge main`** — the first
  moment this lane's code met Plan 0068's, which had landed an hour earlier at `v0.48.1`: doc links,
  `fmt`, `clippy --workspace --all-targets -D warnings`, `nextest --workspace`. Closed **second** by
  design, so it merged an already-advanced `main` and took its own bump.

  **What landed.** Both accumulation buffers — the engine `trails` stage and the attractor scene's
  internal trail — stop sampling the previous frame at the identical uv and sample it through an
  **inverse per-frame transform** instead: the affine `fb_zoom`/`fb_rotate`/`fb_dx`/`fb_dy` about a
  bindable `fb_center_x`/`fb_center_y`, plus a curated `[feedback] warp` family
  (`swirl`/`ripple`/`fisheye`) whose strength is the bindable `fb_warp`, plus a selectable
  `[feedback] blend` (`max` default, `add` for summing echoes under ADR-0046's headroom). Every rate
  is per-second on the injected real `dt`. Phase 5 judged it on the wall at Rich tier over live
  audio: **passed**, *"very good"* on the `swirl` + `add` echo — the vortex reads as depth and flow,
  which is the reference look R2 exists to reach — with fps median 165.0, minimum 114.3, **zero** of
  158 rows below the NFR §1 floor and **zero** of 28 698 frames dropped.

  **The standing defect it retired on the way past.** `trails` `fade` was applied once **per frame**,
  so a `0.9` tail ran a third as long at 144 Hz as at 48. It is now retention per 1/60 s raised to
  the `dt`-relative power. The exponent is written `dt / FALLBACK_DT` rather than `dt * 60` so it is
  `x / x` — exactly `1.0` in IEEE — and the `== 1.0` arm short-circuits `powf`, which is not required
  to return `x` for an exponent of one. That is *stricter* than the plan asked for and stricter than
  the attractor's own long-standing `powf(dt * 60.0)`, which keeps neither guard: harmless today
  (`(1/60f32) * 60.0` does round to `1.0`, and every attractor baseline is hash-identical) but the
  two sinks are **not** using the same form, which is the opposite of what the plan and the ADR both
  say. Recorded as a minor rather than repaired — it is a pre-existing line this plan did not write.

  **ADR-0037, for the third time, and this one has a negative control.** `Trails::resolve` had been
  **ignoring its `surface` argument**, on a premise that was documented, reasoned and — until this
  change — true: neither of its passes computed geometry. A rotation does. The aspect now comes from
  the render target on both sinks (the attractor's is `Scene::render`'s own `aspect` parameter), and
  the guard is a measurement rather than a claim: a rotation spun into a closed ring at a **portrait
  100x160** target boxes **45x46**, against **44x71** with the transform's aspect deliberately forced
  to `1.0`. That is the shape this rule needs every time — a value sourced from two places that agree
  at 16:9 cannot be tested at 16:9, which is precisely how it shipped twice before.

  **One contract narrowed, and the code is what stands.** Phase 3 promised "one vocabulary, two
  buffers". True of the seven `fb_*` params and of `warp`; **not true of `blend`**. The attractor's
  deposit has been additive since the scene was written — its points draw through an additive
  pipeline over the decayed bed, in one pass — so there is no `max` to select without a **second draw
  pipeline**, which is exactly the coexisting-pipelines-with-matching-bind-layouts shape
  ([ADR-0058](../adrs/0058-bind-group-layout-collisions-carry-evidence.md)) the one-shader warp
  family exists to avoid. Paying a known WARP hazard to make a sentence literally true is the wrong
  trade, so the sentence changed. The asymmetry is documented where an author meets it, and is in
  [ADR-0048](../adrs/0048-transformed-feedback.md)'s `Outcome`.

  **The ADR overcharged for including the attractor, and that is the transferable finding.**
  Alternative D accepted the attractor at a stated price of "a second shader and test surface". It
  cost neither. The arithmetic, the seven names, the identity predicate and the aspect correction all
  live **once**, in `core/src/render/feedback.rs` — which is **not a new file**: it already existed as
  the `PingPongField` home, the ADR-0012 seam ADR-0048's own Supplements line says this was always
  for. The WGSL is one snippet concatenated into both shaders, and both sinks delegate `set_param` to
  the shared `Transform` rather than matching seven names each. So the two buffers cannot drift on
  what `fb_rotate` means **by construction**, which is more than Alternative D was promised.

  **The identity claim, and how it was proved.** `(x * aspect) / aspect` is not `x` in `f32`, so the
  shader `select`s the literal `in.uv` on a CPU-computed flag whenever every `fb_*` sits at its
  default and no warp kind is live. Measured the [0071] way at every phase and again at this close:
  `LMV_BLESS` on clean `main` against `LMV_BLESS` on the merged branch, hash-for-hash across
  `--test golden --test composite --test line_joints` — **all 20 pre-existing baselines identical,
  exactly three added** (`composite_warp_swirl/_ripple/_fisheye.png`). This is the form to reuse and
  the naive `git diff` is not: **8 of 20 baselines drift from their committed bytes under a bless on
  clean `main` on this box**, so a diff convicts eight files no change touched.

  **Seven deviations, all self-reported in the phase commits.** The two that changed a contract are
  above. The other five: the motion guards live in a new `core/tests/feedback.rs` rather than in
  `composite.rs` (a separate binary because building GPU resources mid-run perturbs what the trails
  stage resolves to on WARP, and these guards need a portrait target and several consecutive
  multi-frame runs — the exact perturbation `composite.rs`'s own module docs warn about; it pins no
  baseline); routing grew `ParamRoute::StageAndScene(usize)`, the enum's second fan-out and the
  mirror of `SceneAndBackdrop`; `PostStage` grew `set_dt` and `set_feedback` and `Scene` grew
  `set_feedback`; docs were written incrementally rather than all in Phase 4 (forced, and correctly:
  `core/tests/preset.rs` asserts every declared param appears in `presets/README.md`, so a phase that
  adds a param cannot leave the suite green without documenting it); and Phase 1's file list named
  `scenes/mod.rs` for param routing, which was simply wrong — it is `render/mod.rs` plus `post.rs`.
  All corrected in the plan text at the close.

  **The edge policy the ADR left open: transparent border, by shader rather than by sampler.**
  `ClampToEdge` re-deposits the border texel every frame and compounds it into a permanent bar of
  colour along the two long edges — worst at exactly the portrait shape a tunnel wants empty space to
  travel into (ADR-0047's lesson, applied). Off-frame reads contribute nothing, implemented as a
  shader test because `AddressMode::ClampToBorder` is an **optional wgpu feature** this project
  cannot require on every adapter it ships to.

  **Two minors that are measurements rather than defects, both now backlog entries.**
  [0082](../design-backlog.md): `frame_ms_p99` spikes to **25.0 ms** on preset switches and the
  fullscreen toggle while `frame_ms_avg` never passes 8.7 ms and no frame drops — GPU resource
  rebuilds, not steady-state cost, but **the not-yet-built quality governor is specified to read that
  column**, and it would demote a preset running at 165 fps. [0083](../design-backlog.md): RSS grew
  **385 → 663 MB** over three minutes against ADR-0010's ~327 MB driver floor — too short and too
  switch-heavy a window to call a leak, and this plan adds two accumulation buffers so *some* growth
  is expected, but it was **never measured against a no-feedback control**, which is what makes it
  worth keeping rather than quoting either way.

  **One finding routed to the content lane, and it is R6's to resolve.** The scratch echo fixture
  needed a **narrow palette band** to keep `add` from flattening the fragment field — three drafts of
  it were an even green rectangle, because a fullscreen source under an additive deposit sums into a
  flat wash at every exposure. So `blend = "add"` and a rich palette pull against each other, no
  shipped preset has had to resolve that trade, and [0075]'s feedback cohort is where it gets
  resolved. Its sibling fixture's *"needs more colours and saturation"* is the same finding seen from
  the other side; nothing there asks for an engine change, since `[palette]`, `saturation` and
  `palette_mix` all already reach this surface.

  **Close-ceremony step 3b.** The plan touched `presets/` only through `presets/README.md` — no
  `.toml` landed, so there is no new content to judge against the set. The staleness sweep was run:
  no shipped preset is working around either defect this plan fixed (the `fade` frame-rate
  dependence is invisible at capture `dt` and no preset compensates for it; the `resolve` aspect bug
  was unreachable before a transform existed). **Verdict: nothing to curate, nothing stale.**

  **The index bullet, moved here verbatim by Plan 0105 (2026-08-16).** [0046 — Transformed
  feedback: the past learns to move](done/0046-transformed-feedback.md) — closed 2026-08-09.
  Review: **no blockers, no majors, four minors, two nits**. **R2 lands, and with it the last
  engine gate on [0075]** — both accumulation buffers now resample their past through one shared
  transform, so a zoom is a tunnel and a rotation a spiral, and the standing `trails` frame-rate
  defect (a `fade` applied once per *frame*, a third as long at 144 Hz as at 48) is retired en
  route. **Zero pixels moved without opt-in**: all 20 pre-existing baselines hash-identical to a
  clean-`main` bless, three added, re-measured at this close after the merge. **[ADR-0037] was
  caught for the third time here** — `Trails::resolve` had been ignoring its `surface` argument
  on a premise a rotation falsifies — and the fix carries the negative control this rule has
  always needed: a spun ring boxes **45x46** at a portrait 100x160 target, against **44x71** with
  the aspect forced to `1.0`. **One contract narrowed:** `[feedback] blend` reaches the trails
  stage only, because the attractor deposits additively in one pass and a `max` there would cost
  a second pipeline — the exact WARP hazard the one-shader warp family avoids. The plan sentence
  changed, not the code. **Alternative D was overcharged**: including the attractor cost one
  shared `Transform` and one WGSL snippet concatenated into both shaders, not "a second shader
  and test surface". Two Phase 5 observations became [backlog 0082](../design-backlog.md)
  (`frame_ms_p99` spikes to 25.0 ms on preset switches, and the not-yet-built quality governor is
  specified to read that column) and [backlog 0083](../design-backlog.md) (RSS 385 → 663 MB over
  three minutes, with no no-feedback control beside it).

- [0068 — Why the downbeat rarely locks: an instrument, an ablation, and a verdict](done/0068-why-the-downbeat-rarely-locks.md)
  — **done 2026-08-09**, Mode 4 review **no blockers, no majors, two minors, one nit**. Four phases as
  `be39985` (the probe) / `c6a7de3` (the ladder) / `62ade74` (the verdict and the doc qualification),
  in the `lmv-plan-0068` worktree, with **Phase 3 run by the user on 2026-08-09**. Full gate re-run at
  the close after `git merge main` (a no-op — the lane branched from `main`'s tip and `main` had not
  moved): doc links, `fmt`, `clippy --workspace --all-targets -D warnings`, `nextest --workspace`.
  **No render file touched and no baseline in scope**, so the golden control was not owed here.

  **The plan shipped a diagnosis and no fix, on purpose, and the close verified the "no fix" half
  rather than taking it on trust.** `CONFIDENCE_THRESHOLD` is still `0.25` (`downbeat.rs:55`),
  `BASS_WEIGHT` still `0.7` (`:71`), and the 4/4 fold and the confidence measure are unchanged. The
  one production edit is a refactor: `effect_size` now delegates to a new private `effect` returning
  the raw share, the null share and the corrected value — checked branch by branch at the close as
  arithmetic-identical to what it replaced, including both early returns and the `null >= 1.0` arm.

  **The probe reads the gate rather than offering a second opinion about it**, which is the property
  that makes the whole plan trustworthy. `DownbeatTracker::terms()` takes `&self`, recomputes from
  state `process` already keeps, returns fixed-size arrays by value: no heap allocation, no clock, no
  field written, and no under-test branch anywhere in `process`. Its bit-for-bit claim is **a real
  assertion, not a comment** — `terms.effect_corrected.to_bits() == clock.confidence.to_bits()` on all
  four clean rotations and both unaccented cases.

  **ADR-0071 was respected exactly where it is easiest to break.** Every absolute confidence value in
  both tests is *printed*; every `assert` is comparative and dimensionless and taken inside one run —
  the fold-spread ratio (clean vs unaccented), "every axis degrades", "dropouts are steeper than
  contrast" swept across four assumed noise floors, and "the fold still names the true alignment at
  the rung where the gate shuts on it". The ratios pass ADR-0074's same-kind test: both terms of each
  are fractions of the same axis or of the same fold.

  **The verdict, and the reason it is more than a lock-rate number.** Phase 3 measured 98 minutes of
  unambiguous 4/4 through the live app on `v0.48.0`: 352 locked rows of 5900, **6.0 %** — which
  *sharpens* Plan 0048's approximate ~6 % rather than moving it, and establishes it as a ceiling, not
  a floor to improve on by picking better material. Split by genre it is **6.79 %** on
  four-on-the-floor techno and **0.14 %** on backbeat rock/pop. **That 48x inversion is the finding.**
  The intuition says the genre with the clearest loudness accent should do better; it does 48x worse,
  because the accent is 70 % bass and a backbeat's kick carries a *two*-beat periodicity that makes
  alignments 0 and 2 tie, while four-on-the-floor's kick marks every beat equally and flattens the
  fold outright. Neither genre's kick marks the **bar**. So the named cause is the accent feature, and
  the fold and the confidence measure are exonerated as the primary term.

  **The limit that keeps this honest, and it survived into the ADR:** the 1 Hz log records **band
  levels, not per-beat accents**. The identification therefore rests on the numeric match to contrast
  rung 1.00 across two genres plus the construction argument — *not* on a direct reading of the four
  alignment scores on real audio. `terms()` is the instrument that could settle it and is not wired to
  the diagnostics log. Three further limits are stated: bands were unsaturated (so this is not an
  input-scaling failure), two genres are not all of 4/4 (bass-marked material may well lock), and the
  mis-accent risk ADR-0050 exists to guard is **still untested**, for the same reason as before — the
  gate was shut ~94 % of the time here too.

  **One deviation, weighed and accepted.** `.config/nextest.toml` gained a third and fourth named
  override so the probe's output survives a *passing* run. It is outside both phases' file lists, but
  it is the same reasoning the file already carries for two existing reporting tests, and it is
  stronger here: this plan's deliverable **is** the printed decomposition, so a run that hides the
  output has run the instrument and thrown the reading away. Scoped by test name rather than
  profile-wide, as the existing entries are.

  **Two minors, recorded rather than repaired.** (1) The accessor shipped as `pub fn terms()` /
  `pub struct DownbeatTerms`, where Phase 1's file list said "`#[cfg(test)]` or crate-internal" — but
  the same line put the probe in `core/tests/downbeat_probe.rs`, an integration test that links the
  rlib externally and cannot see a `pub(crate)` item, so the two halves of that bullet were never
  jointly satisfiable. It is a widening of `core`'s **Rust** surface only: not the C ABI, not the
  grammar, and the type's doc comment says so. Corrected in the plan text at the close. (2) Phase 2's
  synthetic ladder found **dropouts** the steep axis while Phase 3 found real material sitting on the
  **contrast** axis at its extreme — not a contradiction (one asks which axis the estimator tolerates
  least, the other asks where real music sits), but the two readings are easy to quote against each
  other and ADR-0082's `Outcome` is the only place they are reconciled.

  **What it deliberately did not do.** The repair — a downbeat cue that is not bass energy, evaluated
  against the same ladder and the same two genres — has **no ADR and no plan**, decided at this close
  rather than skipped. The fork an ADR would exist to decide (a stronger accent feature versus a
  longer history window) is not a live fork yet: the `Outcome` exonerates the other two terms and
  names the route. It stays a pointer in [design-backlog 0042](../design-backlog.md), whose original
  entry is untouched and whose `ANSWERED` note is appended below it.

  **The index bullet, moved here verbatim by Plan 0105 (2026-08-16).** [0068 — Why the downbeat
  rarely locks](done/0068-why-the-downbeat-rarely-locks.md) — closed 2026-08-09. Review: **no
  blockers, no majors, two minors, one nit**. **It shipped the diagnosis and no fix, as designed,
  and that was verified rather than assumed** — `CONFIDENCE_THRESHOLD` is still `0.25`,
  `BASS_WEIGHT` still `0.7`, the fold and the confidence measure are untouched, and the
  `effect_size` split is arithmetic-identical on every branch. The probe reads the gate rather
  than second-guessing it: `terms()` is `&self`, allocation-free, clock-free, and its `to_bits()`
  equality against the published `BarClock::confidence` is asserted on all six cases. **The named
  cause is the accent feature's bass weighting**, and the finding that names it is that backbeat
  rock/pop locks **48x worse** than four-on-the-floor — 0.14 % against 6.79 % over 98 minutes —
  because a bass accent marks every beat in one and the half-bar in the other, so it hardly ever
  marks the *bar*.
  [ADR-0082](../adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) is **accepted**
  carrying that dated `Outcome`, including the limit that matters most: the 1 Hz log records **band
  levels, not per-beat accents**, so this is a ladder match plus a construction argument and *not*
  a direct measurement of the four alignment scores on real audio. **The repair has no ADR and no
  plan**, and that is a decision taken at this close rather than an omission: the route is named
  ([design-backlog 0042](../design-backlog.md), answered in place) but the fork an ADR would
  decide — a stronger accent feature versus a longer history window — is not yet a real fork,
  because ADR-0082's `Outcome` exonerates the fold and the confidence measure. It stays a backlog
  pointer until someone takes it.


- [0067 — The curation route](done/0067-the-curation-route.md) — closed 2026-08-09. Review: **no
  blockers and no code findings**. The one substantive item was a **factual error in the plan** —
  its claim that `bar` "stopped being a variable at ADR-0050" is false (`bar` is `VAR_NAMES[5]`,
  the beat phase in `[0, 1)`; ADR-0050 *added* `bar_phase` alongside it) — struck in the plan and
  in [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)'s
  `Outcome`, which repeated it. **The gate is now worth leaning on**: `reactivity` drives PCM
  through the real analyzer, with a non-vacuity test that has a positive control, and ADR-0081's
  gate-strength Negative is discharged. **Phase 1d is a recorded negative result** — the
  resolution ladder is flat because `frame_diff` scores occupancy and occupancy is
  scale-invariant, so `ANIM_FLOOR` and `SIZE` did not move and CI pays nothing; [backlog
  0009](../design-backlog.md) now needs a coverage-aware statistic, which is the earned question.
  Two costs the lane refused to absorb went to [backlog 0080](../design-backlog.md) (reactivity 1.8x,
  ~85 % of it warm-up hops that are rendered and thrown away) and the Coral Oracle's gain
  exception to [backlog 0081](../design-backlog.md) (the house gain rule is written down
  nowhere).

- [0064 — The symmetry stage and the banded
  palette](done/0064-the-symmetry-stage-and-the-banded-palette.md) — closed 2026-08-09. Review: **no
  blockers and no code findings**; the three items raised were all stale text in the plan itself,
  corrected at the close (its "fourteen" baselines are **19**, its five LUT sample sites are **seven**,
  and its "duplicated at five sites" risk is **three** WGSL copies plus four calls into the one
  Rust function). ADR-0037 verified clean at `kaleidoscope.rs:1116` — the aspect comes from the
  render target, in the stage that has shipped that bug twice. All 19 pre-existing baselines
  byte-identical. **[ADR-0077](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md) was
  accepted with an
  [Outcome](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md#outcome--2026-08-09-at-plan-0064s-close):
  its "the inner rings alias severely" was inferred from a texel ratio and never observed** — six
  cutoffs, three sources, no visible onset — so `kaleido_inner` ships as styling with a
  protective side effect, not as the rescue. **First run of close-ceremony step 3b**: no
  near-duplicate geometry in any of the nine families, and nothing in `presets/` still pays for a
  fixed defect.

- [0071 — Light that adds without covering
  (`occlude`)](done/0071-light-that-adds-without-covering.md) — closed 2026-08-09. Review: no
  blockers, two majors, three minors, two nits. **Phase 5 is `human` and deliberately outstanding
  — see Standing.** The default stayed `1.0` by the user's look, no preset binds it, and **no
  golden baseline moved** (measured as a bless-against-a-clean-`main`-bless, all 19
  hash-identical). Three ADR-0085 claims were falsified and are in its
  [Outcome](../adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md#outcome-2026-08-09-after-plan-0071):
  there was no "one seam" (six sites, four shaders), the `Scene` trait *did* widen
  (`set_occlude`, its fourth optional method — it meets
  [ADR-0030](../adrs/0030-scene-target-size-hot-path-hook.md)'s three conditions, checked at the
  close), and the families *did* drift (additive scenes with an empty chain are unoccluded
  whatever they bind). It also emptied
  [ADR-0058](../adrs/0058-bind-group-layout-collisions-carry-evidence.md)'s `[Texture, Sampler]`
  group — see [0053]'s row.

- [0072 — The backdrop joins the palette](done/0072-the-backdrop-joins-the-palette.md) — closed
  2026-08-09. Review: no blockers, no majors, four minors, three nits. **The last surface outside
  `[palette]` joined it**: no cosine copy remains in `background.rs`, and `saturation` /
  `palette_mix` fan out to the sky through one binding. Two of the plan's own claims were
  falsified and are recorded in
  [ADR-0086](../adrs/0086-the-backdrop-colours-through-the-preset-palette.md)'s `Outcome` — **the
  two fixtures it ordered re-blessed pin no pixels**, and its "fifteen" is 18 by its own grep (16
  in scope, 3 moved). Zero golden baselines changed.

- [0074 — The figure colours by how far it has come](done/0074-the-figure-colours-by-how-far-it-has-come.md)
  — **done 2026-08-08**, Mode 4 review **no blockers, four minor items (three repaired at the
  close)**. All six phases as `79c08a9` / `6c928f6` (the Phase 2 gate verdict) / `22956e0` /
  `776d6da` / `57965df` / `dcc88ba`, on `main` rather than in the `lmv-plan-0074` worktree — that
  lane was created and never used, and is stale at the approval commit. Full gate re-run at the
  close (`fmt`, `clippy --workspace --all-targets -D warnings`, `nextest` 589/589, doc links), and
  **`attractor_ifs.png` is the only baseline that moved** — twice, as the plan predicted — verified
  as a `git diff` of `core/tests/golden/` against `v0.43.0`.
  **The gate paid for itself twice, and that is the transferable finding.** Phase 2 is a `human`
  gate placed after *one* `dev` phase because Plan 0073 spent five phases on a channel that did not
  read. It passed — the gradient reads on all five figures as *depth into the figure*, and is more
  legible under a long trail than bare, which is exactly where the age channel died. It also wrote
  down the one comparison it could not run (`root_hue` did not exist yet), and Phase 6 ran that
  comparison and **reversed the gate's conclusion**: `root_hue` at the fern's full `map_tint` beats
  the budget split the gate had found, so **both shipped IFS presets bind `root_hue` and neither
  binds `root_tint`**. A gate that records what it could not test is worth more than one that only
  records a verdict.
  **The general property behind that, and it outlives this family:** `root_*` is **anchored** at
  zero rather than centred (measured — `root01` tops out at `0.41`–`1.05` per figure, so centring
  would slide the figure rather than spread it), and an anchored coordinate term therefore only ever
  pushes *up* the ramp, spending the palette's bright end by construction. Its escape, a negative
  binding, runs into an **undocumented repeating-LUT wrap** — below about `root_tint = -0.38` on the
  fern the darkest region wraps to the ramp's brightest stop as cream speckle. Whether a palette
  *coordinate* should clamp rather than repeat is an **engine question touching every LUT-sampling
  scene** and is [backlog 0075](../design-backlog.md); the authoring half of it is now documented in
  `presets/README.md`.
  **`Particle` has one spare word left** and `PARTICLE_ATTRIBUTES` six entries, so the next
  per-particle channel is a struct change to a type four families share — ADR-0088 Alternative A
  (two-step map history) is the standing claimant. `age_tint` / `age_hue` are gone; `Particle::age`
  stays, and now drives a bindable `emergence`.
  **Three close repairs, all doc/asset:** the two operator docs still presented the gate's
  `map_tint 0.46 -> 0.22` split as the shipped fern tuning ([backlog 0076](../design-backlog.md),
  raised by Phase 6 against its own Phase 5); `core/tests/fixtures/attractor_ifs.toml` had picked up
  a **UTF-8 BOM** at Phase 5 (the standing `Set-Content` trap); and `core/tests/attractor.rs`'s
  header comment still named `age_tint` where the body binds `root_tint`.
  **The fourth was repaired the same session** (`3ca736f`), at the user's ask: the step shader
  claimed the reseed dispatch skipping `root` was "a stronger version of the same reason" it skips
  `map`, where it is **weaker** — `map` survives a kick because sub-copy membership does, `root`
  does not because it is a pure function of position, so a kicked particle carries a stale distance
  for one fixed step. The comment now also names the trap in the obvious repair: the jitter slot is
  handed `StepUniform::NO_IFS`, so calling `ifs_root_distance` there returns an exact `0` for every
  particle and flashes the figure to the palette's anchor colour on every reseed.
  **The stale `lmv-plan-0074` worktree and its branch were removed at the same time** — `-d`
  succeeded, confirming the branch was fully merged.
- [0073 — The fern unfurls and colours by what made it](done/0073-the-fern-unfurls-and-colours-by-what-made-it.md)
  — **done 2026-08-06**, Mode 4 review **no blockers, two minor doc items repaired at the close**.
  All six phases as `c2c8c76` / `339a178` / `7ef5270` / `b69ca4e` / `50c4eda` / `6e335b2` on
  `plan-0073-the-fern-unfurls`, with `52b34e0` merging `main` mid-plan and `40fd1ee` amending
  Phase 2's done-when. Full gate green on the merged tip (`fmt`, `clippy --all-targets -D warnings`,
  `nextest`, doc links resolve), and **`attractor_ifs.png` is the only baseline that moved** —
  verified as a diffstat against `main`, not taken on report.
  **`Particle` is now 48 bytes** with `age` and `map`, two words still free *at this close* (one
  after [0074]), so anything touching `particles/mod.rs` inherits a struct four families share and a
  `PARTICLE_ATTRIBUTES` constant that **replaced `vertex_attr_array!`** — that macro lays attributes
  out consecutively and would now fetch `map` from the padding word, silently and with no compile
  error. Read that constant before adding an attribute; [0074] added the sixth. [backlog 0064](../design-backlog.md)'s startup rectangle is **gone by
  construction**: the IFS initial fill seeds at most four distinct positions where a box fill seeds
  one per particle, asserted as a count with no statistic in it.
  **Half of [ADR-0087](../adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) was
  falsified by its own implementation, and that is the finding worth carrying forward.** `map_tint` /
  `map_hue` work and are bound in both shipped IFS presets; `age_tint` / `age_hue` render as
  per-particle speckle with no gradient, because the 8-step emergence ramp deliberately hides
  exactly the first steps where age correlates with position — the two constants the ADR treats as
  independent look knobs are in opposition, and lengthening the lifetime cannot help. The ADR is
  **accepted with a dated Outcome section** rather than edited; three candidate repairs (authorable
  ramp length / a genuinely spatial distance-from-restart-point channel / retire the two params) are
  in [design-backlog 0074](../design-backlog.md) and the second is a new plan. Nothing ships
  defective — both params default to the identity and no preset binds them.
  **Two doc repairs at the close**, both in the sweep Phase 5 owed: `presets/README.md` asserted the
  falsified reading as fact four paragraphs above its own correction, and `docs/preset-palettes.md`
  was never swept at all — its attractor coordinate formula
  `hue_center + (seed - 0.5) * hue_spread` had silently stopped being the whole expression, which
  matters because `map_tint` **competes with `hue_spread` for that same number** (`attractor_fern`
  had to drop from `0.16..0.42` to `0.05..0.125` before its fronds separated).
  Followups the plan names and did not take: a bindable churn rate (Phase 6 did not ask for it), and
  the `IfsFigure::frame()` comment fix Plan 0062's review raised.
- [0065 — The mandala interior: `star_pattern` stops being hollow](done/0065-the-mandala-interior.md)
  — **done 2026-08-06**, Mode 4 review **no blockers**. Phases 1, 2, 4, 5, 7 as
  `33e5efc` / `1904469` / `419418f` / `a35485a` / `b026ff3` on `plan-0065-mandala-interior`, with
  `3c0e56a` recording the Phase 3 human verdict and `d4030b2` merging `main`. Full gate green on the
  merged tip (`fmt`, `clippy --all-targets -D warnings`, **566/566 nextest, 0 skipped**, doc links
  resolve) and **no golden baseline moved or was added** — `git diff main -- core/tests/golden/` is
  empty and `LMV_BLESS` was never run.
  [ADR-0079](../adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md) is
  **accepted with an Outcome section**. **Closes
  [design-backlog 0007](../design-backlog-archive.md) in full** — its hollow-interior half, live
  since 2026-07-26 under the user's *invest, do not cut* call, and its composition question, answered
  by shipping both (`star_mandala` was the ornament alone, `star_weave` the same roster inside the
  twelve-fold interlace — both since retired, see below).
  - **Phase 6 (`human`, "judge it against music") did not run as a plan phase** and the plan closed
    anyway, at the user's call, with the live pass happening immediately after, outside the plan.
    Part was already answered from the running app — the washed-out first draft rejected, the
    solid-stroke retune approved, eight rings cut as lace, `rings in weave` kept against the
    reviewing session's reading of the sample. Still open going in: counter-rotation against real
    music, and `glow` + `thickness` together on adjacent thin rings.
  - **That pass ran the same day and retired all three presets** (`654304a`). It came back against
    neither of those, but against the mechanism — the motifs are sampled polylines, so the vertices
    show and a circle reads as a polygon, *after* the solid-stroke retune had already removed the
    inflated-glow explanation ([backlog 0073](../design-backlog.md)). The `star_pattern` coverage
    floor reverted to `0.34` with them, so **Phase 7's re-derivation lasted about six hours** — a
    round trip worth knowing about before treating any floor move as settled. **The plan's
    deliverable is not reopened**: `rings` ships, `star_pattern` is not hollow, and the shell
    measurement holds. The mandala look now ships as `reaction_gilt` — analytic iso-contours folded
    by `kaleido_order`, no geometry and so no vertex. **No preset uses `rings` today.**
  - **The gate went red and the floor was re-derived rather than the plan held.** The three presets
    measure `0.2442`-`0.2544` against a `star_pattern` coverage floor of `0.34`. That floor is
    derived from the shipped library at half each family's sparsest member, and `MAX_FLOOR_SLACK`
    exists to force re-derivation when the minimum moves — so **Phase 7 was added at the close** and
    took it to `0.12` (slack `2.04`), which is what `coverage_floor`'s own doc comment prescribes.
    The presets were **not** inflated with `glow`/`trails` to clear the old number.
    [Backlog 0072](../design-backlog.md) stays **open at medium-high** as the measure's replacement,
    which this is not; two new numbers sharpen it — `thickness` alone first clears `0.34` at a base
    of about `9` (a 29-px stroke at 1080p, a blot well before that), and 54 % more geometry moves
    coverage 2.6 %.
  - **Two backlog entries this raised are live:** [0071](../design-backlog.md) (the scalloped
    boundary the user chose as a real curve primitive, which the engine does not have) and
    [0073](../design-backlog.md) (motif outlines show their vertices). **They were filed as `0070`
    and `0072` on the lane and renumbered at the merge** — `main` minted its own `0070` the same day
    at [0069]'s close — so this lane's commit messages cite the old numbers; the mapping is recorded
    in the entries' own header block.
- [0069 — The instrument that sees a figure leave the frame](done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)
  — **done 2026-08-06**, Mode 4 review **no blockers, three minor, one nit**. Phases 1-4
  `c3ce524` / `a359b67` / `9289a7c` / `1abf3a9` on `plan-0069-in-frame-geometry`; `main` was already
  an ancestor of the branch, so **no merge commit** — a straight fast-forward. Full gate green on the
  tip (`fmt`, `clippy --all-targets -D warnings`, **546/546 nextest, 0 skipped**, doc links resolve)
  and **no golden baseline moved or was added**, as the plan promised.
  [ADR-0083](../adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) is
  **accepted with an Outcome section**.

  **The headline result is narrower than the plan's own framing, and that is the transferable part.**
  The plan set out to replace a failed *gate* with a working one. The new measure convicts both
  frozen defects decisively — repairing them moves it `0.4975` (comb) and `0.7788` (corona), against
  the `0.055` pixel coverage had and could not use — but **it has no separating absolute threshold
  over the shipped library either**. `Rose Zoom` (`0.3492`) and `Rose Overflow` (`0.3659`) *bracket*
  the frozen over-scaled comb (`0.3563`), and both are working exactly as authored: a length fraction
  cannot distinguish "deliberately inside the figure" from "accidentally outside the frame", because
  they are the same picture. So it shipped as a **paired** instrument — a configuration against its
  own repair, matched by name — which is the question a content pass actually asks. Anyone adding
  `assert!(fraction > 0.5)` over the library fails two shipped presets, which is the same mistake one
  axis over. `docs/capturing.md` leads with what it cannot see, per the plan's Phase 4.

  **One done-when was not met literally, and the plan's arithmetic is why.** Phase 3 asked for a
  separation "at least an order of magnitude larger than the `0.055`"; the shipped bar is `5x` and
  the comb reads `9.0x`. A comb roots every bar on a shared baseline, so a fully-driven bar at
  `scale = 3.80` keeps about `0.47` of its own length in frame **whatever else is done to it**,
  capping the separation near `0.53`. The baseline-rooted fact that made this figure invisible to
  pixel coverage is still present, as a much weaker version of itself — so expect a *bounded* margin
  from any baseline-rooted family. That is an unchecked number in the plan, not a shortfall in the
  implementation, and it is the fourth close in a row where an architect-authored numeric done-when
  cost a round trip.

  **The `Nx` framing is presentational.** `separation / 0.055` divides an in-frame-length-fraction
  difference by a pixel-coverage-ratio difference, so per
  [ADR-0074](../adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md) it is
  not a dimensionless property and the assertion is really an absolute `0.275`. Safe here for a
  reason worth knowing before anyone tightens it: the fraction is a **pure CPU computation** over
  segment endpoints and an aspect, with no rasterizer in the loop, so it is machine-independent. The
  live exposure is *content* drift, with a factor of `1.8` of headroom on the tighter pair.

  **What to know before editing the line renderer.** `LineRenderer::draw` now costs one `Cell::get`
  on the shipped path and, when the thread-local switch is on, a CPU loop over the segment slice
  bounded by the renderer's capacity. `geometry_extent.rs`'s first test asserts a capture with it on
  is **byte-identical** to one with it off — that assertion is what makes "off" mean off, and it is
  not optional. `draws_segments()` is an **exhaustive match with no wildcard arm**, so a new
  `SystemKind` fails to compile until someone says which side of the split it is on. Two frozen
  defect fixtures now live in `core/tests/fixtures/` with a "do not tune" header apiece: bringing one
  back inside the frame would leave the gate true of nothing. One doc error was fixed at this close
  — `capturing.md` claimed both zoomed presets sat at or below the comb, where one is above it.

- [0070 — Shaped marks](done/0070-shaped-marks.md) — **done 2026-08-05**, Mode 4 review **no
  blockers, one minor**. Phases 1-5 `5d21e76` / `c15112a` / `564f3bd` / `d922ce1` / `a87e05b` on
  `plan-0070-shaped-marks`, merged `main` in as `7d5e43f` per ADR-0053 and fast-forwarded; the
  terminal `human` Phase 6 landed at this close as `20657a8`. Full gate green on the merged tip with
  the preset embedded (`fmt`, `clippy --all-targets -D warnings`, **538/538 nextest, 0 skipped**),
  doc links resolve, **no existing golden baseline moved** (one added).
  [ADR-0084](../adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) is
  **accepted with an Outcome section**.

  **The ADR's headline worry did not materialize, and the way it was measured is the transferable
  part.** It booked "a branch in the hottest fragment shader" as a known negative. Measured on an
  RTX 3080 Laptop in release at 1280x720 with 10 000 particles, the **matched-coverage isolate** — a
  12-sided polygon at 75 % quad coverage against the disc's 78 %, taking the full `atan2`/`floor`/
  `cos` path — reads 0.858 ms against the disc's 0.877, with a run-to-run spread of ~0.01 ms. The
  branch's arithmetic is **below the resolution of the measurement**. The plan's own figure, a
  7-pointed star, is 19 % *faster* because it lights 34 % of the quad. Naively measuring
  disc-against-star would have "shown" a 19 % saving from a branch, which is why the third probe
  exists: **any fragment-shader cost reading owes a matched-coverage isolate**, or it prices the
  silhouette rather than the code. Per ADR-0071 `core/tests/mark_cost.rs` reports and does not gate,
  and skips with a notice on a software rasterizer.

  **The done-whens were checked on pixels, not on the quantizer.** The seven-maxima claim is counted
  off a real capture at 5, 7 and 9 points, with a **disc as the noise control** — a circle must
  return exactly one lobe, or the star counts would be reading rasterization noise. The stepping
  claim is asserted twice and the second one is the load-bearing one: the eased `7 → 9` sweep is
  *rendered* and the frames grouped by exact equality, so a count re-derived fractionally further
  downstream would still be caught.

  **The one minor:** `marks.rs`'s tests carry a CPU mirror of the WGSL SDF, kept identical by
  inspection (the `kaleidoscope.rs` `edge_sample_radius` precedent). The pixel-level tests render the
  real shader for `disc` and `star`, so a mirror drift on `ring`, `heart` or the polygon inradius
  would go unnoticed. Not worth a change now; worth knowing before editing either copy.

  `disc` means something different on each scene on purpose — the emitter's is its pre-existing
  anisotropic *glint*, so its `spin` stays visible and every shipped emitter preset is untouched.
  Backlog 0033's silhouette half closes; its **fill-and-outline half is re-filed as
  [backlog 0069](../design-backlog.md)**, as that entry asked. Phase 6 also raised
  [backlog 0068](../design-backlog.md): the swarm has no per-mark variation, and the emitter — which
  has exactly the `twinkle` a starfield wants — cannot hold one, because its fixed source line needs
  ~2.5 s to fill the frame and every behavioral gate captures 0.5 s.

- [0066 — The level lever](done/0066-the-level-lever.md) — **done 2026-08-05**, Mode 4 review **no
  blockers, one minor**. Phases 1-4 `2a4f65c` / `2e2cc32` / `0f10f18` / `3502c2e`, the terminal
  `human` Phase 5 `d7bf78c`. Ran in a worktree on `plan-0066-the-level-lever`, merged `main` in as
  `a0c3486` per ADR-0053, then fast-forwarded. Full gate green on the merged tip (`fmt`, `clippy
  --all-targets -D warnings`, **538/538 nextest, 0 skipped**), doc links resolve.
  [ADR-0080](../adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) is
  **accepted with an Outcome section**.

  **The zero-pixels claim held exactly, and it is the reason to trust the rest.** `git diff
  --name-status` over the whole plan adds two baselines and modifies **none** — the `brightness`
  multiply is by literal `1.0` and the bright-pass's exposure multiply is by literal `1.0`, both
  IEEE-754 identities, and both are asserted as such rather than inferred from a green suite
  (`the_brightness_factor_is_exactly_one_by_default_and_scales_linearly`, and the first half of
  `the_bright_pass_thresholds_exposed_light`, which states plainly that if it fails "every golden
  baseline has moved and Phase 3 is a re-bless rather than a check").

  **The one finding is a consequence the ADR did not anticipate, and the retune found it by
  looking.** The background pre-pass sits *upstream* of the tonemap, so it was scaled by the
  `exposure` these presets carried; `brightness` is scene-local and does not reach it. Moving a
  number from one to the other therefore multiplies the sky by `1 / old_exposure` — 33x on Lorenz,
  which would have turned a backdrop authored at ~1/255 into a grey wash under the figure. Phase 5
  divided the `bg_*` terms by the same 0.03 and said so in the header. **Check every
  upstream-of-tonemap term before making that swap on any other preset.**

  Also worth carrying: `bloom_threshold` on Lorenz moved from `8` — the ceiling, which its header
  called *capped, not tuned* — to a measured `0.4`, off a coverage sweep that simply did not exist
  before, because at the old ceiling every value in it rendered the same picture. So `MAX_THRESHOLD
  = 8.0` is answered as a non-constraint: a real preset now sits an order of magnitude below it.

- [0062 — The chaos game grows a fern](done/0062-the-chaos-game-grows-a-fern.md) — **done
  2026-08-05**, Mode 4 review **no blockers, one major, four minor**. Phase 1 `8c621fa`, Phase 2
  `7cdd34e`, Phase 3 `18a088c`, Phase 4 `b4aa911`, Phase 5 `daf59c6`, Phase 6 `7cab347`, the `human`
  Phase 7 content pass `cf977f9`. Ran **in the main checkout on `main`, not in a worktree** — the
  same ADR-0053 deviation [0063] took the day before. Full gate green (**521 tests, 0 skipped**),
  `fmt` and `clippy --all-targets -D warnings` clean, **no existing golden baseline moved**, doc
  links resolve. [ADR-0075](../adrs/0075-ifs-family-morphs-in-singular-value-space.md) is **accepted
  with an Outcome section**.

  **The engine can travel continuously from one figure to another, and the safety property is proved
  without a GPU.** Every map is carried as the SVD of its linear part, so contractivity is
  `max(|sx|, |sy|) < 1` — a comparison on two numbers rather than a property of a matrix. The sweep
  that matters runs the shader's own step **in Rust**: 25 ordered figure pairs × 33 morph positions
  × 10 000 iterations, asserting contractivity **separately** from boundedness, which is what
  distinguishes "converges" from "happened not to blow up in ten thousand steps". The failure being
  excluded is a permanently dead particle buffer, and a capture of a preset that diverges only on a
  loud passage would have passed.

  **The content pass falsified three of the ADR's own predictions, and the pattern is worth
  carrying.** `fern -> spiral` — the design's showcase pair — came **last of five** crosses swept as
  filmstrips, because a figure near the contractivity ceiling is a poor morph *target*: the spiral's
  arm contracts at only 0.93, so the intermediate spreads instead of settling. `sierpinski -> fern`
  came **first**, and the figure the ADR hedged might be "only a correctness fixture" earns a preset
  — not as a look but as an **endpoint**, its rigidity being the reason the dissolve reads. And
  `morph`'s visible rate is **front-loaded** (lit width 0.248 -> 0.448 across the first twentieth of
  the range), because per-map rotation compounds through the recursion. All three were reachable only
  by rendering; none is a defect. **The general lesson: a plan that names its own showcase before the
  content pass is naming a hypothesis.**

  **Two "the development configuration cannot see it" traps were found and closed unprompted.** The
  fit is aspect-aware and tested at portrait as well as 16:9 — the dragon and the spiral are twice as
  wide as they are tall and would hang out of a narrow window. And `STEP_SLOTS` gives every sub-step
  its **own** uniform slot: one reused slot would hand every sub-step of a stalled frame the same
  `step_index`, and at the steady 60 fps this box develops at, `pending_steps` is always 1, so no
  test here could ever have disagreed.

  **Numeric assertions in this plan are all properties, per
  [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md).** The
  measurements run on a fixed-seed CPU chaos game, so they are exact on every machine; the fit's
  under-measure is asserted as **the fraction of the frame the true figure occupies** rather than as
  the error itself; and the preset-switch budget is bounded by an **iteration count**, not wall
  clock — the obvious version of that test would have measured CI load and flaked.

  **The one engine finding is [backlog 0064](../design-backlog.md).** The predicted startup "haze" is
  in fact a legible hard-edged rectangle — ADR-0066's own artifact class, back on every switch *into*
  the family, with the ~1 s preset dissolve landing entirely inside it. Deferred to the successor
  plan's staggered respawn, which removes it as a side effect; the entry carries a cheaper interim
  and the coupling trap that interim has to avoid. **Backlog 0065 and 0066 are documentation and were
  discharged in the same commit** (`presets/README.md`'s IFS section). Review minors, all
  non-gating: `IfsFigure::frame()` and `AttractorFamily::projection()`'s IFS arm are unreachable in
  production but documented as live; the `morph` table row still reads "every value between is a real
  figure" ~40 lines above the correction; and `attractor_fern.toml`'s `[smoothing]` comment mentions
  easing a `morph` that preset no longer binds.

- [0063 — The attractor keeps its depth](done/0063-the-attractor-keeps-its-depth.md) — **done
  2026-08-04**, Mode 4 review **no blockers, one major, two minor**. Phase 1 `1f0fc41`, Phase 2
  `6cd0d52`, Phase 3 `c3c43d8`, Phase 4 `6f27462`, the `human` Phase 5 content pass `1855340`. Ran
  **in the main checkout on `main`, not in a worktree** — a deliberate ADR-0053 deviation, recorded
  here because it shaped the whole day: it occupied the main working tree, so [0036] had to open its
  own lane, and the two closes had to be sequenced rather than raced.
  [ADR-0076](../adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md) is **accepted with an
  Outcome section**. Full gate green (487 tests, 0 skipped) over both lanes' code.

  **The three-D families stop being bistable, and the property is pinned as algebra rather than as a
  picture.** `project()` computed the rotation's third output and discarded it, so the image at
  rotation `pi` was the exact `x`-mirror of the image at `0` — textbook structure-from-motion
  bistability, which is *why* Thomas and Lorenz read flat. The test asserts that identity and its
  destruction directly on the formula (`perspective = 0` mirrors exactly; `0.5` provably does not;
  a 2-D family is byte-identical at **every** perspective, stated as invariance because a 2-D map's
  half turn is a point reflection and the mirror identity was never theirs). Dimensionless — it holds
  on every adapter and resolution, which is the shape [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
  asks for.

  **The new fixture's sensitivity was DEMONSTRATED, and the first draft failed the demonstration.**
  `attractor_depth.toml` exists because the rostered `attractor.toml` is De Jong and therefore
  structurally cannot execute one line of the new code. Each of the four levers was neutralized in
  turn and the capture re-measured: at the drafted `depth_fade = 0.6` that row read mean 0.0091 /
  outlier 48 against tolerances of 0.02 / 48 — **inside both**, so a regression that killed the fade
  outright would have passed. At 0.95 it reads 0.0169 / 92 and fails decisively. The numbers are in
  the fixture header so a later weakening has to face them. This is the standard a new guard should
  be held to.

  **⚠ The content pass FALSIFIED the plan's and the ADR's framing claim.** Both said `perspective`
  enlarges the figure and "recovering the framing is a `zoom` edit". Measured over four spin phases:
  the centroid **orbits** by ~`0.9 x perspective` in NDC while the figure's size grows **6 %** across
  the entire legal range — so the dominant effect is a *phase-varying translation*, and a zoom (a
  static scale) cannot recover one. The usable ceiling is **~0.3**, not the `0.8` clamp. ADR-0076
  carries the measurement in its Outcome; `presets/README.md` was corrected at this close; the two
  structural remedies are [backlog 0061](../design-backlog.md) and unowned. Both presets paid a zoom
  cut for it (`attractor_lorenz` 1.32 -> 1.16, `attractor_thomas` 1.14 -> 1.02), and Lorenz's `sanity`
  coverage is now 0.2747 against a 0.18 floor — still passing, with less room than before.

  **Two minor items, both doc freshness, both fixed in the close commit.** `docs/capturing.md` still
  described the golden roster as "one frozen fixture per system" after `EXTRA_FIXTURES` made that an
  incomplete description (Phase 4 swept `core/tests/fixtures/README.md` but not this one), and
  `presets/README.md`'s `depth_hue` row said nothing about the three regimes the content pass
  measured ([backlog 0062](../design-backlog.md)): it is a *hue* cue only on a constant-lightness
  hue-travel ramp, it wraps above `2 * min(hue_center, 1 - hue_center)`, and it is dead under
  `ink_amount = 1`. One bookkeeping note, not a defect: Phase 4's `presets/README.md` sweep landed in
  a **parallel session's** commit (`9d2de68`) rather than in the phase commit, which the dev commit
  message says outright.

- [0036 — macOS and Windows release artifacts](done/0036-macos-and-windows-release-artifacts.md) —
  **done 2026-08-04**, Mode 4 review **no blockers, one minor, one nit**. Phase 1 `0329adf` (+ fix
  `be031eb`), Phase 2 `cc7a43f`, Phase 3 `aa9dfec`, on lane `plan-0036-release-artifacts` (ADR-0053),
  merged to `main` at `d081dfd` with the full gate green (487 tests) over both lanes' code.
  [ADR-0038](../adrs/0038-tag-driven-release-unsigned-universal-mac-app.md) is **accepted**.
  Approved 2026-07-26, sat unbuilt for nine days, and was taken the day the user asked whether a Mac
  build was possible.

  **⚠ Carried forward: Phase 4 (push the tag, send it, hear back — `human`) is NOT done.** The plan
  closes without it **by its own design**: its Followups section says the friend's report retires the
  Plan 0001 Phase 10 carry-forward "at a later close", so the report was always expected to outlive
  this plan. It also resolves a circularity — Phase 4 needs a tag, and the tag is minted *by* this
  close. Retire both carry-forwards together when the report lands.

  **The build path is CI-verified; the publish path is not.** A `workflow_dispatch` dry run
  (`30944179623`) went green on the first attempt: both jobs, and all seven of `bundle.sh`'s checks
  executed against real Apple tooling — `lipo -archs` naming both architectures, `plutil -lint`,
  `codesign --verify --strict`, the plist version equal to `[workspace.package]`, the zip's three
  top-level entries, the preset count, and no `.md`. **But the `release` job has never run**, because
  only a tag reaches it. `gh release create`, the exactly-two-assets guard and the `--clobber` re-run
  path are unexecuted, so two of Phase 2's done-when criteria are open and **the first tag push is
  their test**. The third — "a `workflow_dispatch` produces both run artifacts and creates no
  release" — is discharged exactly as written.

  **What the dev lane caught that no local check would have.** Two defects, both found by running
  what could be run rather than by reading: `git commit -- <pathspec>` re-reads from the working
  tree and silently discarded a `git update-index --chmod=+x`, landing `bundle.sh` at `100644` on a
  clone with `core.filemode=false` — a `Permission denied` that would first have appeared on a
  runner, on a tag push. And `Compress-Archive` writes backslash separators on Windows PowerShell
  5.1 and forward slashes on pwsh 7, so the Windows verify compared against paths that could only
  match on one of the two hosts. Both are the ADR-0037 shape one level out: a value that agrees on
  the configuration you test at and disagrees elsewhere.

- [0055 — The fold edge becomes a choice](done/0055-the-fold-edge-becomes-a-choice.md) — **done
  2026-08-04**, Mode 4 review **no blockers**. Phase 1 `5eac2d7`, the `human` Phase 2 A/B judged
  2026-08-04 (**verdict preserved above** — it is a human decision no commit can re-derive), Phase 3
  `feba426`, Phase 4 `752eb69`, second adoption `2c618de`, on lane `plan-0055-fold-edge` (ADR-0053),
  **reviewed together with [0052] against one merged tip**.
  [ADR-0061](../adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) is **accepted with
  an Outcome section**. Closes [backlog 0037](../design-backlog.md); raises
  [backlog 0058](../design-backlog.md).
  **Five candidates in, three out.** `falloff` (0), `tile` (1, **the default**), `squash` (2);
  `vignette` and `mirror` deleted from the shader rather than left dead. The A/B falsified **two
  bets held by the documents that asked for it**: [backlog 0037](../design-backlog.md) had
  specifically asked the supplement to reconsider `vignette`, on ADR-0047's Outcome calling it the
  cleanest on a border-filling field — it lost on both scenes; and `mirror` was ADR-0061's own new
  contribution, argued as the one candidate answering both rejections at once — it also lost on
  both. Neither would have been rejected from stills. **What the entry got right stands**: a figure
  and a field want different answers, which is the whole argument for a selector.
  **Three things the implementation falsified in prose, all now in ADR-0061's Outcome.** `squash` is
  **not** the identity below `r_max` (`tanh(m) < m` for every `m > 0`, so it compresses the whole
  interior — only the deleted `mirror` left it untouched); a strict-monotonicity assertion on it has
  to stop at `m = 4`, because past `m ≈ 7.6` consecutive `tanh` steps land inside one f32 ulp and
  asserting further would assert a property of `f32` (ADR-0071); and the predicted wholesale
  re-bless never happened, for a reason now recorded as a fixture *property* rather than as luck.
  **One done-when was met by a different instrument than specified, and the substitution is argued.**
  Only `tile` carries the anti-smear guard, because only `tile` **can** be the smear — it is the sole
  treatment whose coordinate leaves `[0,1]`, `squash` is barred arithmetically from reaching an
  out-of-range coordinate at all, and the pixel statistic cannot separate them (measured: `squash`
  0.10 against a deliberately mis-wired `tile`'s 0.06, so a bound passing `squash` would pass the
  defect). The guard's own floor is **measured, not picked** — correct `tile` 0.35, mis-wired 0.06,
  floor 0.15 — and is a dimensionless ratio of the same statistic on the same image, which is what
  makes it a property rather than a machine measurement. The re-scoped disc guard was **verified**
  still non-vacuous against the pre-ADR-0047 shader (peak 199, 6052/6052 out-of-disc pixels lit).
- [0052 — The emitter: objects that spawn, fall on a parabola, and
  die](done/0052-the-emitter-objects-that-spawn-fall-and-die.md) — **done 2026-08-04**, Mode 4 review
  **no blockers**. Phase 1 `2470a50`, Phase 2 `d155615`, Phase 3 `52a756c`, Phase 4 `53a896e`, on
  lane `plan-0052-emitter` (ADR-0053), **reviewed together with [0055] against one merged tip**.
  [ADR-0057](../adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md) is **accepted**.
  Closes [backlog 0034](../design-backlog.md); [backlog 0033](../design-backlog.md) (shaped marks)
  stays open. The **first genuinely new scene idiom since the attractor**, and the half of the
  figurative gap that carries motion.
  **Its largest finding was not the scene.** It reproduced [backlog 0039](../design-backlog.md)
  live: an emitter bind-group layout written byte-identical to the swarm's — one `[Uniform]` entry,
  `VERTEX` visibility, `min_binding_size: None` — made the **swarm** read the *emitter's* uniform on
  this box's DX12 WARP build. `golden` came back with every other fixture at mean 0.0000 and `swarm`
  at **0.1803** with a max outlier of **175**, and `sanity` gave the swarm presets different numbers
  on each run. Merely *constructing* a seventh pipeline with the same layout shape was enough;
  hardware renders both correctly, so a bless would have committed garbage as the swarm's baseline.
  Distinguishing the descriptor — `VERTEX_FRAGMENT` visibility plus an explicit `min_binding_size`
  — restored it to 0.0000. **This is the third instance and the cheapest fix found so far**, and it
  is what [0053] now inherits.
  **Two open items it did not close.** One `STATUS_ACCESS_VIOLATION` in `sanity` under parallel
  threads during Phase 1, non-reproducing across four later full runs and **not dismissed as
  noise** — if it recurs, it has a precedent now. And `distinctness` is **structurally blind** to
  this scene: the report's unit is a pairwise matrix and the family ships one preset, so a 1x1
  matrix would say nothing. It is left out rather than lowered or waived, and the reasoning now
  lives in `core/tests/distinctness.rs` beside the curated family list.
- [0059 — Lorenz finds its plane, and the attractor can trade samples for
  curves](done/0059-lorenz-finds-its-plane.md) — **done 2026-08-04**, Mode 4 review **no blockers**.
  Phase 1 `357a17e`, Phase 1b `1c47de5`, Phase 2 `4fb4a81`, Phase 3 `642aec0`, and the `human`
  Phase 4 content pass `990fedc`.
  [ADR-0068](../adrs/0068-the-projection-basis-is-a-per-family-property.md),
  [ADR-0069](../adrs/0069-the-attractor-trades-sample-count-for-trace-length.md) (**with an Outcome
  section**) and [ADR-0070](../adrs/0070-a-feedback-pass-addresses-its-own-target-in-framebuffer-space.md)
  are all **accepted**. Closes [backlog 0048](../design-backlog.md); raises
  [backlog 0057](../design-backlog.md) and adds a fourth finding to
  [backlog 0049](../design-backlog.md).
  **The plan set out to fix a projection basis and found something older and larger.** Phase 1
  landed the correct x–z plane and **Lorenz still rendered as an X** — because the attractor's own
  trail mirrors itself: the decay pass sampled the accumulation target with the unflipped fullscreen
  prelude while the draw wrote it in clip space, so **every attractor has been rendering
  `figure ∪ mirror(figure)` for the life of the scene**. Both Plan 0057 Phase 4 and ADR-0068 had
  read the shipped X as "the two lobes edge-on" — a correct reading of the *wrong figure*. It
  survived every gate because a mirror-symmetric output conceals its own symptom, and it was caught
  by asking ADR-0037's question somewhere new: `pan_y` returns **two mirror copies** where a
  translation can only return one. That diagnostic is now the gate (ADR-0070).
  **Phase 4 was a first authoring, not a re-tune, and it answered three questions — two against the
  plan's own prediction.** `density` + `fade` **holds** a legible curve, so ADR-0069's Alternative D
  does not get its case and no successor is owed. The spin's dwell is fine, so the ADR-0068
  supplement is **closed unopened** — that risk was written against the fog and does not survive the
  trace. And **the reseed-streak A/B could never have been run**: flipping `RESEED_DRAWS_STREAK`
  gives byte-identical output under three stimuli, because `encode_jitter` precedes `encode_steps`
  and the step overwrites `prev` unconditionally. Nothing renders wrongly — but a claim about the
  running system had been written from the diff. **Retire the flag; do not reorder the dispatch.**
  **Two levers and one warning for the content lane.** Lorenz ships as a plotted trace
  (`density = 0.002`, `exposure = 0.03`) and Thomas as a pen drawing (`0.02`, `0.10`) — the **first
  two shipped presets to bind `exposure` at all**, which is why nothing had complained before that
  `density` is exposure-neutral in *total light* and not per texel. `presets/README.md`'s claim that
  you can re-aim `density` "without re-tuning `size`, `fade` or `exposure`" was **corrected at this
  close**; the wider gap — no scene-local deposit param, `exposure` crossfading across a dissolve,
  and `bloom_threshold` measured pre-exposure and clamped at `8.0` — is
  [backlog 0057](../design-backlog.md).
  **The coverage-floor conversation the plan expected did not arise**: the attractor family minimum
  is still Leviathan at `0.3442` against the `0.18` floor (1.91x slack), because Leviathan is
  untouched and nothing new sank below it.
- [0060 — a test number states a property, or names its
  machine](done/0060-a-test-number-states-a-property-or-names-its-machine.md) — **done 2026-08-04**,
  Mode 4 review **no blockers**. Phase 1 `1d56600` + `31073f6`, Phase 2 the `human` push (CI run
  **30903871856**, green on all three jobs), Phase 3 `a324b21` + `ae4c215`.
  [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) and
  [ADR-0074](../adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md) are
  both **accepted**, 0074 with an **Outcome** section (see below). Gate on `main` before the bump:
  `fmt` clean, `clippy -D warnings` clean, **459/459 nextest, 0 skipped**; no shipped code, no
  pixel, no golden baseline, C ABI unchanged at v4.
  **It ended five consecutive red CI pushes, and the shape of the defect is the durable part.** Two
  tests froze a number that only existed on the machine that measured it: four bit-exact `f32`
  literals taken on x86_64, which had **never once passed** on the arm64 runner and could not (the
  fixture's own input comes from platform libm and `rustfft` dispatches NEON there); and a
  pixel-difference floor of `0.01`, **half** the `0.02` this project already calls cross-rasterizer
  noise. Both now state a property or name their machine and skip elsewhere, printing what they
  observed — which is how Phase 2 could read the runners' numbers at all.
  **The plan's own route-back clause fired, and that is the process finding.** Phase 3 was to set a
  magnitude floor from the ratio Phase 1 built; the ratio came back **7.3x apart** between two
  builds of one software rasterizer (`0.268675` local, `0.036654` on CI), under the `0.05` fallback
  the plan had **named in advance**. So it cost one architect session and an ADR instead of an
  argument. Worth repeating on any plan that has to read a number off a machine it does not own.
  **The hardware measurement is the finding nobody went looking for.** Once ADR-0074's "hardware we
  do not have" premise turned out to be false, Phase 3 took the clean reading here — and it landed
  on the **CI WARP** number (`0.036542` against `0.036654`, matching to three figures on the ratio
  and **five on the control**), not on the local one. So WARP 10.0.26100 behaves like hardware and
  **this box's WARP 10.0.19041 is the outlier**, inflating the numerator — the opposite of the
  mechanism ADR-0074 recorded. Its decision stands (a number reproducing on two configurations is
  still a measurement), but its Context, one Negative bullet and Alternative A's rejection are all
  narrower than they read, and the Outcome section says so. **The consequence belongs to [0053]**:
  the golden suite blesses on the outlier build, and a `frozen`-only sequence with no dual-live
  asymmetry in it renders 1.54x differently here than on either other configuration. Nothing has
  looked at what else that moves.
  **One more thing left standing on purpose.** `dissolve_at`'s doc comment attributes the trails
  quirk to "the DX12 WARP rasterizer" and it is load-bearing — it is why the hardware-only sibling
  is hardware-only and why `background_composite.rs` skips. On this evidence it is a property of one
  WARP *build*, written as a property of WARP: ADR-0071's own error a level down, in prose. Not
  edited from a single contrary reading; [0053] Phase 3 is where a second configuration settles it.
  Both ADR corollaries are now in the `architect` skill's lens 4, as the plan's Followups asked.
- [0050 — in-app settings, live quality, and a browse overlay that
  fits](done/0050-in-app-settings-and-a-browse-overlay-that-fits.md) — **done 2026-08-04**, Mode 4
  review **no blockers, no majors**. All six phases landed including the `human` Phase 6:
  `14cd9e2` `Renderer::set_tier` + the `[` / `]` swap, `bed0274` the browser opens where you are and
  wraps and repeats, `46b38f6` the list flows into columns, `d81a24a` the settings modal on `S`,
  `19096f1` the operator docs, and the eyes-on pass on 2026-08-04.
  [ADR-0054](../adrs/0054-runtime-tier-switching-rebuilds-on-the-live-context.md) is **accepted with
  an Outcome section**. Gate on `main` before the bump: `fmt` clean, `clippy -D warnings` clean, full
  `nextest` green. C ABI stays v4 (the tier deliberately does **not** reach it), no new dependency,
  no golden baseline moved.
  **The app has an operator surface for the first time.** Five write-only hotkeys were the whole
  control surface; `A` printed to a stderr nobody watches during a show and did not even persist,
  and the dwell bounds were reachable only by editing TOML. Now `S` lists Quality, Auto-rotate, Min
  and Max dwell, Fullscreen, Display, Diagnostics and the resolved preset directory, every row
  applying immediately and persisting through the `Config::save` that already existed. Diagnostics
  is deliberately **session-only** — a live show coming up with the overlay painted because someone
  pressed `F3` last week is a worse default than pressing `F3` again.
  **`settings.rs` is a second pure state machine beside `overlay.rs`, not a generalization of it**,
  and the reasoning is worth keeping: the two modals' rows *mean* different things —
  pick-one-and-close over a filtered roster against edit-a-value-in-place over a fixed list — so
  merging would rewrite a green module to share ten lines of up/down/wrap. Where they should agree
  they do, and **that agreement is asserted rather than inherited**.
  **The list arithmetic was the whole problem and it is now a pure function.** At `ROW_H = 30` from
  `y = 94` a 1080p window holds 32 rows against a 35-preset roster, so the display this project is
  built on already scrolled presets out of sight while 1904 px of unused width sat beside a
  15-character longest name. `overlay::layout(...)` decides columns, rows per column and the
  whole-column scroll offset with no window and no roster, so the unit tests are the real check and
  the eyes-on pass confirms pixels rather than logic. `COL_W` is an **estimate that says so** —
  glyphon shapes a proportional font and `core` exposes no measurement API, so a name past the
  budget truncates with an ellipsis, which makes an underestimate cosmetic rather than a collision.
  **Key repeat is filtered one level down, where the key's role is known** — honoured only for a
  modal navigation key while a modal is open. Widening it would make a held `Space` machine-gun
  preset switches through a ~1 s dissolve each.
  **Phase 6 found the thing the plan was written to find, and it found it before any input.** The
  governor demoted `Rich → Floor` within seconds of startup: the shipped provisional `Rich`
  multipliers do not hold this display's frame budget, which is Plan [0044] Phase 4's unrun question
  answering itself the moment an instrument existed to hear it. It agrees with the session's other
  finding — `attractor_lorenz` at `Rich` is the worst thing on screen through a transition. **No
  field was tuned on it**, per that phase's own rule.
  **What Phase 6 did *not* deliver, and why that is not a hole:** the per-preset p99 table. Its
  instruction sheet in `docs/on-device-validation.md` named **`rose_kaleidoscope`**, retired in the
  2026-07-28 library pass, so the operator went looking for a preset that does not exist —
  `docs/design-backlog.md` had caught that name once and the on-device doc was never swept.
  Corrected here to `fragment_kaleido`. The table itself belongs to Plan [0044] Phase 4, carried in
  a checklist whose own status line says it **does not block plan closes**, and it is now **coupled
  to [0059] Phase 4**: `[particles] density` changes how many particles the attractor draws, so
  calibrating `RICH.attractor_particles` before the content pass measures a target about to move —
  and the attractor is exactly where both of Phase 6's signals point. Deferred deliberately, with
  the demotion recorded as its first data point.
  **One design note in the plan was wrong and `dev` refused it rather than implementing it.**
  `set_tier` was told to re-apply the current surface size; there is nothing stale to re-apply,
  because `render/mod.rs:543` calls `scene.set_target_size(...)` on the shared draw path every frame
  and every `PostStage` takes `surface` as an argument. The corroborating evidence was already in
  the tree — the governor's `apply_tier` has shipped without it since [0044].
  **The doc sweep discharged its negative half as evidence, not as a claim:** `presets/README.md`,
  `docs/presets.md` and `docs/preset-palettes.md` were confirmed *not* to need sweeping by grepping
  the four code phases' diff for a `[params]` name, rather than by assumption.

- [0058 — the gate can see an empty frame, and "loud" has to mean more
  picture](done/0058-the-gate-can-see-an-empty-frame.md) — **done 2026-08-03**, Mode 4 review **no
  blockers, no majors**. All four phases landed including the `human` Phase 4: `96a914f` coverage
  measures the scene, `a0ce1c9` the floors are re-measured, `8e79aa3` the excitation ratio ships as
  a report, `2efb80e` the comb and the corona come back inside the frame.
  [ADR-0067](../adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) is **accepted with an
  Outcome section**, implemented in full. Closes design-backlog **0053**, retires **0052**, raises
  **0054**. Gate on `main`: fmt clean, clippy `-D warnings` clean, **427/427, 0 skipped**. C ABI
  stays v4, no new dependency, nothing on the per-frame path.
  **The gate it replaced could not be failed, and the number is the headline.** `is_lit` was handed
  the frame's top-left pixel as its background reference, and `bg_vignette` guarantees that corner
  is the frame's **minimum** — so on 24 of 35 shipped presets the backdrop cleared the 0.01 sparse
  floor whatever the scene drew. Phase 1 drops every `bg_*` binding test-side and compares against
  black; `sanity_roster` panics if the `bg_` prefix ever matches nothing, so a rename fails the gate
  instead of silently restoring the backdrop. **No renderer API was widened** — `background.rs`
  already defaults `bright`/`vignette` to `0.0`, so this is *not applying three bindings*, and
  `golden`, `distinctness`, `reactivity` and `shot` are untouched.
  **The non-vacuity check is a test rather than a claim, and it asserts both halves.**
  `the_pre_repair_ridge_passed_the_old_gate_and_fails_this_one` freezes `spectrum_ridge` exactly as
  it shipped broken (`scale = 3.20`) and shows it **passing** the old corner-sampled gate (coverage
  `0.5421`, all four quadrants) before showing it failing the new one (`0.0000`, zero quadrants).
  Without the first half, "the new gate fails this" proves nothing about the old one.
  **Every floor was invalidated at once and re-derived**, each at half its system's lowest preset —
  the old numbers sat **11.9x to 84x** below the content they bounded. And because a floor is only a
  floor while the content is near it, that is now a check rather than a comment:
  `report_coverage_distribution` fails when a system's lowest preset climbs past `MAX_FLOOR_SLACK`
  (2.2x). **It fired for real within the day** — [0057] Phase 6's re-raise moved the attractor
  minimum from De Jong `0.2461` to Leviathan `0.3785` (slack 3.15x) and the gate failed the run with
  the number rather than letting it drift; the floor was re-derived `0.12 -> 0.18` from the printed
  distribution.
  **Two findings the plan asked for in advance and got, both uncomfortable.** The tonal-flatness
  re-measure answers "does removing the backdrop widen the 0.90 margin" with a **no**: `Spectrum
  Ridge` fell `0.8655 -> 0.1916` (it was never flat — it was a lit vignette measured as one, which
  retires backlog 0052), but `Rose Web` went the *other* way, `0.7645 -> 0.8839`, with nothing about
  the preset changed, because the vignette had been supplying mid-tones that diluted the share in any
  one band. Margin narrowed from `0.035` to `0.0161`. **`0.90` stays** — a preset drifting over it is
  a preset to route, not a constant to nudge.
  **And Phase 3 shipped as a report, not a gate, which is the finding worth carrying past this
  plan.** The plan authorized either outcome and left it to the numbers. `coverage(loud) /
  coverage(moderate)` reaches **none** of the three known-defective configurations: `Spectrum Comb`
  scores `1.0891` — it draws *more* when loud, because a comb roots every bar on a shared baseline
  and clipping the tips costs a rounding error of pixels; `Spectrum Corona` `1.0514`; the pre-repair
  ridge is `0/0`, undefined, being already off frame at moderate. Meanwhile the only content near a
  plausible threshold is **correct** (De Jong `0.8552`, Leviathan `0.9568` — the attractor's *peak
  buys structure* idiom). A gate at `0.80` would sit `0.055` from De Jong while catching nothing. So
  the ratio is printed and watched; what the second capture *does* support is enforced —
  `MODERATE_MIN_COVERAGE = 0.04`, a factor 2.23 under the library's lowest moderate coverage, which
  catches the inverse defect (in frame when driven hard, absent at the level music occupies). **Pixel
  coverage is the wrong measure for a figure whose tips leave the frame**, and the successor
  ADR-0067 already names — an in-frame geometry fraction — is [backlog 0054](../design-backlog.md).
  **Phase 4's repair is measured with the geometry, not with a stimulus that could not fail it.**
  `spectrum_comb` `3.80 -> 1.20`, `spectrum_corona` `5.20 -> 0.45`. The asymmetry is arithmetic
  rather than error: the comb's bars stand on `baseline` (-0.85) against a half-height of 1.0, so
  1.85 units are usable, while the corona spends most of its budget before the audio term gets any
  (`radius` breathes to 0.53, `base` reaches 0.20) and a spoke 60 degrees above horizontal has
  vertical extent `0.866 * tip` against a `zoom` reaching 1.10. **The stimulus is itself the
  finding**: under `--signal noise:7` the corona sits in frame at *every* value from 0.25 to 0.85,
  because white noise spreads energy across all elements and no single one takes the whole of
  `scale`. Tonal material does. **A broadband stimulus is the wrong instrument for a radial
  layout — it is the one that cannot fail it.**
  **No golden baseline moved, and the plan predicted two would.** `core/tests/golden.rs` renders one
  frozen fixture per `SystemKind`, deliberately *not* the shipped presets (ADR-0023), so an intended
  content tune cannot trip the engine-drift alarm. `LMV_BLESS` was never run; running it would have
  rewritten all 19 baselines to no purpose. Same shape as [0054]'s close.

- [0057 — the attractor's compute path: the deposit, the reseed, the butterfly, and one
  retune](done/0057-the-attractors-compute-path.md) — **done 2026-08-03**, Mode 4 review **no
  blockers, no majors**; three minors, all doc bookkeeping, fixed in the close commit. Phase
  commits: `8c95cf2` the two instruments, `4d77bff` the deposit, `5bb36c2` the reseed, `9d717fc` the
  Lorenz diagnosis, `b2be2d3` the one content pass.
  [ADR-0064](../adrs/0064-a-capture-may-pin-the-rich-tier.md),
  [ADR-0065](../adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) and
  [ADR-0066](../adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) are **accepted,
  each with an Outcome section** — and **two of the three record a premise their own implementation
  disproved**, which is the ceremony working rather than a defect. Closes design-backlog **0031**
  outright (both halves, on measurement), **0047**'s first half and **0050**. Gate on `main`: fmt
  clean, clippy `-D warnings` clean, **427/427, 0 skipped**, **no golden baseline moved anywhere in
  the plan** — the Decision said that was checkable in advance, and it was checked by re-running the
  suite without `LMV_BLESS`. C ABI stays v4 (`core/src/ffi.rs` byte-untouched), no new dependency,
  nothing added to the audio path.
  **Phase 5 was deliberately not written, and the plan closes without it.** Phase 4's own instruction
  was to stop and route back to `architect` if the Lorenz dust cloud turned out to be the shared 3-D
  view basis. It is — see the sequencing section above for what the successor owes, including the
  **stipple** finding a basis fix alone will not clear.
  **Phase 1 found both of its premises false, and that is the phase's real output.** `shot --tier`
  already existed (Plan [0044] Phase 3, missing only from `--help`), so it was **verified rather than
  rebuilt**: `--tier rich` and `--tier floor` differ on `attractor_clifford`, and omitting the flag
  is byte-identical to `--tier floor`. And ADR-0066's claim that `--signal click:120` never clears
  the shipped reseed gates was **retired by measurement** — it crosses `0.75` on **7 hops out of
  375**, one per beat; the claim was true on the *raw* onset scale and ADR-0049's peak normalization,
  whose attack is instant, invalidated it. So no generator was added. **The gap was aiming**: a
  `--strip 8` samples evenly and lands on one of those 7 hops by luck, and when it misses, a working
  reseed and a broken one render identically. Phase 1 shipped the `onset` row in the filmstrip level
  table (naming the hop it peaked on) and `shot --at <hop>,...` to capture that hop. With both,
  `attractor_ink --tier rich --at 44,46,48,54` renders the rectangle no capture in this project could
  previously produce.
  **Phase 2's invariance is asserted on the value, not inferred from pixels.** `deposit_scale =
  FLOOR_PARTICLES / active_count`, applied in the *vertex* shader (the draw uniform is `VERTEX`-only;
  the fragment's radial falloff is linear, so it is identical to scaling the emitted fragment).
  Clifford at 1280x720: mean display luminance `Rich` `17.372 -> 10.863` against `Floor`'s `10.337`,
  unchanged and **byte-identical** — which is the scalar being exactly `1.0` rather than
  approximately it, and why no golden moved. The unit test is written against `TierConfig` rather
  than a literal `1/3`, so Plan [0044] Phase 4's calibration will move the expectation with it.
  **Phase 3 found a WARP bind-group aliasing defect of exactly the class [0053] exists for, and
  refused the skip.** The jitter was first given its own uniform behind a second bind group sharing
  the compute layout; on WARP that aliases, so the *step* dispatch read the jitter slot, `count = 0`,
  and the cloud never moved — a plausible static box that **moved the golden baseline** and dropped
  three presets to ~0.000 in `animation`. The first response was a WARP skip asserting the attractor
  compute is a no-op there; the evidence was real and the conclusion was wrong. One bind group with a
  dynamic offset into one buffer has no aliasing surface, and the tests now run on WARP with
  hardware's numbers. **This is a hand-caught instance of what [0053] would gate.**
  **Its measurement is over the particle buffer, with the replaced behaviour as the control.**
  Converged De Jong fills **1.7 %** of its own bounding volume; off the figure after a reseed,
  jitter **0.0 %** against the old seed-box re-fill's **100.0 %**. Bounding boxes are the wrong
  instrument and the first draft used them — every seed box is sized to the native extent, so De Jong
  converges to `±1.499` against a `±1.5` box. **An attractor is a filigree**: a uniform re-fill is
  off the figure almost everywhere while staying entirely inside its extent.
  **Phase 6's finding is that the reseed *gates* were calibrated against a different event.** They
  sat at 0.50-0.75 because a reseed used to erase the drawing, so reluctance was protective; a
  disturbance is not destructive, so the same threshold only withholds the accent. Re-measured
  against what real material produces (onset means 0.033 / 0.153 / 0.391 across three stimuli; music
  near 0.20), the band moved to 0.28-0.45 with rank preserved. **The re-raise is halfway and a full
  revert is wrong**, rendered rather than reasoned: `00d99d0` did *two* things — it lowered three
  presets *and* added a bloom stage sized to the lowered figure — so restoring the old numbers puts
  Clifford's interior back to a flat salmon mass, the exact failure `00d99d0` fixed arriving by
  another route. Flatness **fell** on all three (Clifford `0.2053 -> 0.1302`), so the figures gained
  tonal structure, and reactivity at realistic levels roughly doubled.
  **One thing Phase 6 changed that has not been judged**, and it says so: `attractor_ink` and
  `attractor_thomas` had the slowest coefficient easing in the library (0.8 s and 1.0 s), so a 100 ms
  hit arrived as ~0.5 % of a term worth 4 %. Lowered to 0.45 / 0.55, magnitudes deliberately **not**
  raised in the same pass — speed and travel are separate levers. It does not show up in `--report`
  (those cells carry the `+` marker) and **needs a judgement in motion it has not had**.

- [0056 — clamp occupancy: the instrument that would have caught a saturated library, plus the axis
  anchor](done/0056-clamp-occupancy-and-the-axis-anchor.md) — **done 2026-08-03**, Mode 4 review
  **no blockers, no majors**; three minors, all doc bookkeeping, all fixed in the close commit.
  Five `dev` phase commits on the `plan-0056-clamp-occupancy` worktree branch: `a704d30` occupancy
  on the walk, `f607915` the `occ` column and `SAT` lines, `3430cdc` the HARD gate with its measured
  threshold, `d389c96` the axis anchor, `9b07ede` the tonal-flatness statistic in `sanity`.
  [ADR-0062](../adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) is **accepted with an
  Outcome section**, implemented in full;
  [ADR-0063](../adrs/0063-address-the-spectrum-by-frequency.md) is **accepted with an Outcome
  section but only half built** — the anchor landed, `bin_hz()` / `bin_range()` did not and are an
  unnumbered followup plan. Closes design-backlog **0043**, the guard half of **0044** and the
  second half of **0047**. Gate at the close, after merging `main` (carrying [0054]) into the
  branch: fmt clean, clippy `-D warnings` clean, **417/417, 0 skipped**, **no golden baseline
  moved** — the plan's "no pixels move" claim proved rather than asserted. C ABI **stays v4**, no
  new dependency, nothing on the per-frame path touched.
  **Both thresholds are measured, and each measurement is recorded on its own constant** — which is
  the plan's central discipline and the reason to read the constants rather than this summary.
  **Occupancy: `0.9`, measured on both libraries**, 339 clamped bindings each — today's and the
  pre-retune one at `80c5dff^` that Plan 0048 Phase 7 found saturated. Today's highest is `0.609`
  (`Aurora.warp`), next `0.444`, nothing else above `0.45`; the pre-retune set puts **145 bindings
  across 23 of 35 presets** above `0.9`. **So the answer to the plan's title is yes: the gate would
  have failed the build the day ADR-0049 landed**, naming `Glacier.glow`, `Dense.force`,
  `Storm.saturation` and 143 others, each with its own number. `0.75` would have caught 249 rather
  than 145 and was **declined** — it sits `0.14` above a shipped, reviewed preset, and a HARD gate
  that fires on good content buys exemptions, which are the thing that dulls the instrument.
  **No shipped preset needed the exemption**, against the plan's own expectation, so `[occupancy]`
  ships exercised only by fixtures.
  **Phase 5's honest answer is the finding worth carrying, and the plan asked for it in advance.**
  Of the four presets `00d99d0` repaired, **none** would have been caught by the new flat-frame
  gate — measured at `00d99d0^` through the gate itself, not reasoned: Clifford `0.231`, De Jong
  `0.444`, Leviathan `0.137`, Lorenz `0.256`, against a threshold this library's own distribution
  could never put below `0.9`. The reason is the one the plan anticipated: that saturation is a
  **`Rich`-tier** effect (150k particles into the same texels, `One/One` with no normalization) and
  `sanity` renders at `Floor`. There is a sharper form of it in the numbers — today's post-repair De
  Jong measures `0.644`, **higher** than its pre-repair `0.444`, so at `Floor` the repair made the
  frame *flatter*. A `Floor` capture carries no information about the `Rich` defect in either
  direction. The statistic is sound and the tier is wrong, which says unambiguously where the next
  instrument goes: [0057] Phase 1 / [ADR-0064](../adrs/0064-a-capture-may-pin-the-rich-tier.md).
  **The gate does catch one thing nothing else does, and it is a shipped preset.** `Spectrum Ridge`
  measures **`1.000`** — every lit pixel in one of 16 luminance bands. Not a degenerate fixture: its
  two siblings draw the *same* all-bands-at-1.0 data and read `0.31` and `0.44`. It is listed in
  `KNOWN_FLAT` and **asserted to still be flat**, so a repair tells you to delete the line rather
  than leaving a stale exemption — the entry closes itself. Routed to
  [backlog 0052](../design-backlog.md). Note the headroom while you are there: past it the library's
  highest is `0.830` (`Rose Trails`) then `0.765` (`Rose Web`), both trails-heavy line looks, so a
  content pass raising `trails` on a line preset should re-run `sanity` rather than assume.
  **The flat fixture is an additive stack, not an exposure stop, and that correction is measured.**
  The plan proposed driving a preset past the tonemap knee as the cheapest flat frame; sweeping
  `exposure` 1 → 65536 moved flatness `0.43` → `0.26`, the **wrong way**, because past the knee the
  background blows out with the figure and a background-relative metric correctly stops finding
  anything lit. What works is what the shipped flat frames actually did: `glow 20`, `brightness 16`,
  `thickness 44`, `trails 0.97` reads `0.98`.
  **Phase 4's anchor corroborates ADR-0063's damage claim to within its own rounding.**
  `attractor_dejong`'s `bin(0.10)` reads **67.9 Hz** — the ~65 Hz its header names as an earlier
  revision's mistake — and `fragment_aurora`'s `bin(0.14)`, chosen for the ~246 Hz low-mid so that
  loudness could *not* move the curtain, reads **86.9 Hz**, a kick probe. The position that reads
  that low-mid today is `bin(0.31)`. **Both shipped probes are still mis-pointed**: an anchor makes
  the next re-band noticeable, it does not repair the last one, and that repair is unclaimed content
  work. The test's comment says outright that a failure there is not a bug but a **content sweep**,
  and that the literals are updated *after* the sweep rather than instead of it.
  **One correction `dev` made that the plan did not ask for:** `--report`'s `ceils` counted
  "everything that is not a dead gate", which was a ceiling count only by coincidence — a third kind
  would have made it report a saturated clamp as its exact opposite. Every count is now by kind.
  **Three minors fixed at the close**, all the same shape: three places still taught the workaround
  for the gap this plan closed. `presets/README.md` and `docs/presets.md` both said "until
  `--report` grows the occupancy column, do the division by hand", and the plan's own file list
  named neither. Both now point at the column and the gate — while keeping the arithmetic advice,
  because the gate fires at `0.9` and a term pinned for half a track passes it.
  Version **minor 0.30.0 → 0.31.0** (a preset-facing surface, a report column and two gates).

- [0054 — the line scenes catch up: every one honours the palette, and the star stops cutting
  between shapes](done/0054-the-line-scenes-catch-up.md) — **done 2026-08-03**, Mode 4 review **no
  blockers, no majors**; three minors, all fixed in the close commit. Four `dev` phase commits on
  the `plan-0054-line-scenes` worktree branch: `e03598f` the L-system's generation-depth colour,
  `86bba60` the parametric curve's path axis plus the star's measured-flat radial one, `4df5d21`
  `variant` as a continuous contact angle, `9362793` the docs.
  [ADR-0059](../adrs/0059-line-scenes-colour-along-their-generator-axis.md) and
  [ADR-0060](../adrs/0060-star-pattern-variants-interpolate.md) are **accepted, each with an
  Outcome section**. Closes design-backlog **0026** and the *transition* half of **0007**.
  Gate at the close, after merging `main` into the branch: fmt clean, clippy `-D warnings` clean,
  **405/405, 0 skipped**. C ABI **stays v4**, `Scene` unchanged (`set_palette` already existed), no
  new dependency.
  **No golden baseline moved anywhere in the plan, and the plan predicted two would.** That is the
  headline, because it is the *reason* that matters: ADR-0060 kept the old vocabulary, so `variant`
  0 / 1 / 2 still name the `-24 / 0 / +24` degree offsets the three cached rosettes held and the
  fixture's `variant = "0"` asks for the same 11-degree rosette, vertex for vertex. The suite was
  re-run without `LMV_BLESS` and passed; the fixture header now records why a baseline **survived** a
  behaviour change — the Plan 0051 ceremony in its did-not-move form, which is the half that was
  missing from it. The colour half is likewise a behavioural superset but **not bit-exact**, bounded
  at one 8-bit level on 0.02-0.72 % of pixels by ADR-0021's LUT bake of the same cosine, and that was
  **stated rather than blessed through**.
  **Two ADR claims moved under measurement, and both were the ADR's fault rather than the code's.**
  The `lsystem` ramp normalizes over the figure's own deepest generation, **not** `visible_depth` as
  ADR-0059 wrote: `lsystem_fern` opens two branches per rewrite, so it reaches generation **11** over
  six depths and the ADR's divisor would have clamped five sixths of the figure at the palette's far
  end. And `star_pattern`'s radial axis is not "narrow" but **identically flat** — `2n` congruent
  segments about a centred origin, measured spread **1.2e-7** — so `hue_spread` is exactly the
  identity there. It ships anyway (the palette itself is a real gain for that scene) with the
  inertness in the module docs, in `presets/README.md`'s axis table, and in **a test that fails when
  the interior work lands**, which is the good failure. The interior itself stays open: the rosette
  empties the inner **60 %** of its disc at `star_rosette`'s angle and **87 %** at `star_lantern`'s,
  now pinned against the closed form `sin(a)/sin(pi/n + a)`. `lsystem_arrowhead` has no brackets and
  therefore one generation, so it gains the palette and no ramp — worth knowing, since backlog 0026
  was raised against Arrowhead specifically.
  **The step is 0.1 degrees and every constraint behind it is a number**, not a judgement: 1.14 px of
  worst-case vertex motion at 1080p on the sharpest reachable rosette, 480 steps across the `variant`
  range against a ~45 s shipped sweep, and a **0.34 us** rebuild at the reachable `n = 12`. The plan
  asked for the cost at `TierConfig::max_segments` and the honest answer is that this scene **cannot
  reach it** — the tiling vocabulary stops at 12-fold and a rosette is `2n` segments, so 24 is the
  ceiling, pinned by a test; measured at the unreachable cap anyway (282 us, 1.7 % of a frame).
  **One real-time consequence to carry:** `hankin::star_rosette` now runs from `Scene::update`, not
  only from `configure`. It was already panic-free and the hysteresis bounds the rate, but the
  property now holds *because of the cache* rather than by position in the lifecycle — its module
  docs and pragma comment said "off the hot path" and were corrected at the close.
  **Phase 4 went beyond its own file list, correctly.** It swept four files under
  `.claude/skills/preset-author/` that all asserted "`[palette]` is silently inert on the line
  scenes" — one of them carrying it as a filed engine gap. The done-when is "**no doc** says a line
  scene cannot reach `[palette]`", and leaving the content lane's own reference saying the opposite
  would have kept the capability unusable by the lane the request came from.
  **Two minors fixed at the close beyond the plan's list:** the `hankin.rs` hot-path docs above, and
  `docs/presets.md`'s "Curated presets" column, which was wrong in **six of eight** rows
  (`parametric_curve` read 11 against 6 actual). The column is deleted rather than corrected — a
  count re-drifts on every preset added and nothing fails when it does, which is this file's own
  count-free-phrasing rule applied where it had not been.
  **One follow-up routed rather than built:** [backlog 0051](../design-backlog.md) — both shipped
  `star_*` presets still `floor` a `mod(..., 3)` sawtooth, deliberately, because a bare `floor`
  removal turns one slow swap into a hard `2 -> 0` snap at every wrap. So the shipped library
  demonstrates **none** of the morph this plan built; the composition that does is a triangle wave
  over `0..2` with a smoothing constant, and it is a `preset-author` pass.
  Version **minor 0.29.0 → 0.30.0** (a feature plan).

- [0048 — analysis v2: the dual-resolution axis, normalized bands, phrase time, and the one retune
  that pays for all of it](done/0048-analysis-v2-and-the-retune.md) — **done 2026-08-03**, Mode 4
  review **no blockers, no majors**; four minors, all doc bookkeeping, all fixed in the close
  commit. Five `dev` phase commits plus the Phase 3 refactor, on the `plan-0048-analysis-v2` branch
  fast-forwarded into `main` as it went: `bfd892b` the dual-resolution axis, `ef3b772` normalization
  + the `*_raw` escapes, `910a6d1` the shared `Variables::from_frame`, `81b21d5` ADR-0050 Layer 1,
  `7a06676` the gated downbeat estimator, `909ae4a` the harness/docs recalibration. Then the two
  `human` phases: `0fb26d4` Phase 6's verdicts, and `80c5dff` + `bea5c1e` + `fc698cd` Phase 7's
  library retune, its backlog notes and the axis-block regeneration.
  [ADR-0049](../adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) and
  [ADR-0050](../adrs/0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) are **accepted,
  each with an Outcome section**. **Delivers the large half of roadmap R5** and closes design-backlog
  **0015**, **0020**'s structural half and **0035**.
  Gate at the close on `main`: fmt clean, clippy `-D warnings` clean, **388/388, 0 skipped**;
  reachability re-run on the versioned library at **17 flags, every one a `tempo` comparison under
  the single 110 BPM probe, 0 genuinely dead**. C ABI **stays v4** (`core/src/ffi.rs` byte-untouched
  across all seven phases), `Scene` unchanged, no new dependency.
  **The finding worth carrying is that nothing in this project could see the retune's real work
  list.** Phase 5's before-record named **9** broken gates — everything `--report`'s walker can
  structurally see, since it watches forks. Phase 7 found **368** mis-scaled terms: **263 of 332**
  clamped band terms pinned at the real-music median, and **14 presets with no live audio term at
  all** (Rose Web/Zoom/Trails/Overflow, all five `reaction_diffusion`, all three `spectrum`,
  Cathedral, Leviathan) — every one of them behind a **green suite and a clean report**, because
  every reactivity instrument we own diffs a driven band against *silence*, where a binding that
  saturates just above the noise floor scores perfectly. What exposed it was a contact sheet at
  three excitations compared *to each other*: quiet and typical were pixel-identical across all 14.
  That is now [ADR-0062](../adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) / [0056].
  **The same shape appeared on the axis.** Phase 1 silently re-pointed every sub-crossover `bin()`
  probe by about an octave and a half, and no instrument could notice: reachability watches forks,
  and `fft.rs`'s lookup test checks the layout function against the edge table that moved *with* it
  — internal consistency with no external anchor. Now
  [ADR-0063](../adrs/0063-address-the-spectrum-by-frequency.md) (`bin_hz` / `bin_range`) plus [0056]
  Phase 4's external anchor.
  **Phase 6 is the honest half.** Normalization **passes** and the release constant the plan flagged
  as most likely to move **stays** — all four levels span their range with medians well under their
  means. The downbeat estimator **does not mis-accent and also barely locks**: 3.1 % of audible
  rows, confidence mean 0.030 against a 0.25 gate. The stopping condition is therefore **un-fired
  rather than passed**, which the plan says outright. Build arcs on Layer 1; treat Layer 2 as
  decorative ([backlog 0042](../design-backlog.md)).
  **Phase 1 deviated from its own done-when and did it the right way:** "no band above the crossover
  moves by more than rounding" was false as written (v1's collapse fix-up overshot the log curve
  until band 32), so bands 20-31 move and *that movement is the defect being removed* — asserted
  with a counter-assertion that they did move, without which the test would pass if the crossover
  had swallowed the axis.
  **One hazard for anyone re-running the acceptance instrument:** a bare `--report` reads the seeded
  `%APPDATA%` copy, which is pre-retune, and prints a *cleaner* number than the truth. Run it as
  `LMV_PRESET_DIR=./presets`.
  Version **minor 0.28.1 → 0.29.0** (a feature plan).

- [0051 — the scene seam emits premultiplied alpha](done/0051-the-scene-seam-emits-premultiplied-alpha.md)
  — **done 2026-08-01**, Mode 4 review **no blockers**, one `major` (an operator-doc gap, fixed at
  close), four `minor`. Three `dev` phase commits **directly on `main`** rather than in a worktree
  lane — a deliberate exception for a three-commit single-session fix, so there was no merge to
  reconcile: `708b80b` the swarm seam plus the shared `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`
  constant and its guard, `63dd501` the line seam and its guard, `1828ac3` the docs.
  [ADR-0056](../adrs/0056-additive-scenes-emit-premultiplied-alpha.md) is **accepted with an
  Outcome section**. Gate at the close: fmt, clippy `-D warnings`, **388/388**.
  **One baseline moved, and the plan predicted none would.** The no-op argument was sound; the
  survey under it was not — `composite_kaleido.toml` runs `bg_bright = 0.55`, so it was the one
  baseline positioned to see the fix. It moved at mean 0.0009 against a 0.02 tolerance with only
  the outlier gate firing (73 against 48), and was re-blessed deliberately with the numbers and an
  eyes-on description in the fixture header; every other baseline re-encoded byte-identical. The
  lesson worth carrying: **that baseline could not have caught the defect either** — a mean-drift
  gate cannot resolve a hairline rim, so covering the right configuration is not the same as being
  able to see the defect at it.
  Two follow-ups routed rather than built: **[backlog 0040](../design-backlog.md)** — coverage-as-
  alpha darkens the backdrop wherever emitted light is dimmer than it, so `bg_bright` has a new
  ceiling set by the figure's dimmest luminance (verified by rendering; `presets/README.md` states
  it, and whether additive light should occlude at all is a look decision left open) — and
  **[backlog 0041](../design-backlog.md)** — the line guard discriminates on ~5 pixels where the
  swarm's gets 52 651, with a `glow = 0` fourth capture sketched as the stronger property.
- [0045 — linear light: the HDR composite, the bloom stage, and the fold that had to be fixed
  first](done/0045-linear-light-and-bloom.md) — **done 2026-07-31**, passed two Mode 4 reviews
  (**no blockers**; one `major` routed out as [backlog 0039](../design-backlog.md)). Six `dev`
  phase commits on the `plan-0045-linear-light` worktree branch, fast-forwarded into `main` at
  `2f4a804`: `6f282e7` / `b67b9c2` the fold, `c334b0e` the backdrop leaving the chain, `f7ab148`
  the float composite + tonemap, `96780e1` the bloom stage, `23703dc` the halo's alpha clamp.
  [ADR-0046](../adrs/0046-linear-light-hdr-composite-bloom-tonemap.md) and
  [ADR-0055](../adrs/0055-backdrop-leaves-the-post-chain.md) are **accepted**; ADR-0047 was already
  accepted with an Outcome. **Delivers roadmap R1** and closes design-backlog **0005**, **0010**
  and **0011**. Gate at the close on `main`: `nextest --release -p lmv-core` **316/316, 0 skipped**
  (`cargo test`, not nextest, segfaults on the lib binary from many GPU devices in one process —
  the documented crash mode, not a regression).
  **Phase 6's answer is that bloom is not the expensive part**, which inverts the plan's own
  worry. `star_lantern` — shipped during the plan, so the scratch-`LMV_PRESET_DIR` workaround
  Phase 6 was written around is obsolete — runs **164 fps, p99 8.2 ms** Rich-pinned and windowed on
  the discrete GPU, against `attractor_clifford` at 19.9 ms and `attractor_leviathan` at 19.0 ms,
  neither of which binds `bloom_*` at all. So what puts the heaviest shipped preset past a 60 Hz
  frame at `Rich` is the float composite plus the attractor's particle count. The fullscreen and
  `Floor`-pinned runs are carried to [`on-device-validation.md`](../on-device-validation.md).
  **The plan caused one defect, found after the merge and routed to [0051].** Phase 2b made the
  scene→chain seam's alpha load-bearing and two additive draw pipelines never emitted a meaningful
  one — ADR-0055's own first Negative bullet, at the one seam the plan did not reach. Three
  instances of that bullet in one plan (the fold's black fade, the recombine's over-1 alpha, and
  now this) is the signal worth carrying: **a lit backdrop is a distinct test configuration, and
  every seam that touches alpha needs its own guard at it.** Two got one inside the plan; the
  third was found by a `preset-author` session instead of by the suite.
  **Bookkeeping fixed at the close beyond the plan's own list:** `tier.rs`'s `post_cap` docstring
  charged bloom the pyramid (~11 MB) but not its own grid-sized `bloom-src` offscreen (16.6 MB at
  the floor cap) — `nfr.md` §12 had it right and `on-device-validation.md` repeated the short
  framing, both corrected to `16.6 + ~11 ≈ 28 MB`. `presets/README.md`'s bloom section gained the
  consequence it never stated: **a preset authored to the additive-ceiling habit gets nothing from
  the stage** — measured, a draft holding `brightness` under 1.0 rendered pixel-identical with
  bloom on and off — plus the warning that `--set bass=1` flatters a bloom preset far worse than
  any other stage, because the threshold makes the loud-frame/real-audio gap a cliff rather than a
  slope. Backlog **0031** was re-checked and is **still open**; see the entry.
  Version **minor 0.27.0 → 0.28.0** (a feature plan).

- [0049 — the analysis diagnostics surface: making [0048] Phase 6 measurable (and the kaleidoscope
  seam)](done/0049-analysis-diagnostics-surface.md) — **done 2026-07-30**, passed Mode 4 review
  (**no blockers, no majors**; four minors and a nit — three minors and the nit fixed in the close
  commit). Seven `dev` phase commits: `a335f35` the integral fold order, `69761b9`
  `AnalysisMetrics` plumbed to the render seam, `a387909` the overlay rows, `4dabb3f` the six log
  columns, and `38dc792` / `44c96a9` / `d5c9bd7` for Phase 5's four review items.
  [ADR-0052](../adrs/0052-analysis-diagnostics-are-native-only.md) is **accepted with an Outcome
  section**. **Unblocks [0048] Phase 6**, which is now the next thing to run.
  **The reproduction needed a non-zero `kaleido_angle`, and that is the finding worth carrying.**
  The plan described the tear without it. The mirrored wrap is an **even** function, so at
  `kaleido_angle = 0` the two rows straddling the −x ray map to the same folded angle and the `2*pi`
  branch-cut jump cancels **exactly, at every order** — a capture at angle 0 reads a perfect zero
  seam whether or not the bug is present, and `dev`'s first test draft pinned the angle at 0 and
  passed vacuously. The shipped case is the exposed one: 10 of the 12 presets with an active fold
  drive the angle off `time`; `lsystem_arrowhead` and `reaction_reef` pin it at 0 and are genuinely
  immune, `reaction_reef` despite easing its order through fractional values continuously. Both
  assertions were confirmed to fail on the unfixed code. `core/tests/kaleidoscope.rs`'s module docs
  say outright that **setting `ANGLE` back to 0 silently retires the test** — [0045]'s fold-domain
  phase touches this same file and should read that first.
  **Phase 5 item 3 found something at 96 kHz, and it is now [backlog
  0032](../design-backlog.md).** 44.1 kHz — the rate foobar hands the plugin — is indistinguishable
  from 48 kHz (crossover band 19 vs 20, same 8 bin-starved bands), so that half of the sweep found
  nothing, which is the good outcome. At 96 kHz the crossover moves to ~487 Hz and the bin-starved
  region grows from 8 bands to **21, a third of the axis**, because both windows are fixed in
  **samples** rather than seconds and the widening cascades through `fill`'s `prev_hi` chain. Not an
  ADR-0049 regression — that claim is about band edges in **Hz** below the crossover, which do not
  move — but a real difference in what a preset's `bin()` sees on a 96 kHz device, and invisible
  before this test. The fix (windows sized in seconds) re-opens an ADR-0049 decision, so it was
  recorded rather than taken.
  **Phase 5 item 4 chose the docs over a second counter, and pinned the behaviour it now
  describes.** `bar_index` is `(beat_index - alignment) / 4`, so a lock can repeat or skip one bar;
  a never-decreasing counter would need history-dependent state on the determinism-sensitive path
  and would give up the "one formula for both paths" property in `process()`, all to buy immunity
  from a rare one-bar repeat that is *already* the soft failure ADR-0050's gate exists to prefer.
  Softened in all three places, with a new test asserting the step is **exactly one bar** and never
  forward in that fixture.
  **The review's one substantive correction is to ADR-0052 itself**, recorded in its Outcome: the
  ADR's Negative says "the foobar plugin gets no analysis diagnostics", and that is only half true.
  The overlay is **core-drawn**, so a host setting `LMV_DEBUG_OVERLAY` paints all six rows on the
  plugin path too; what is absent is the **programmatic** half (no `lmv_get_metrics` counterpart, no
  log), so nothing there can compute a lock **rate**. That narrows the gap a future reader would
  weigh Alternative A's ABI v5 against.
  **`beat_in_bar` was deliberately left off the overlay.** The plan permits it and the interview's
  mock showed `bar 2/4`, but ADR-0052 pins `AnalysisMetrics` at exactly six values. Left as a
  widening for whoever needs it.
  **Content note for `preset-author`: `kaleido_order` is now a stepped parameter** — an eased sweep
  snaps at each half-integer, because a fractional wedge count cannot divide the circle and tears
  the frame. That is a visible content change made by an engine plan. `presets/README.md` documents
  it and points at `kaleido_angle` for continuous motion; the lane's `references/systems.md` table
  row was swept in the close commit.
  **Verified at review** rather than taken on trust: `fmt --check` + `clippy --workspace
  --all-targets -D warnings` clean; `cargo nextest run --workspace` **367/367, 0 skipped**;
  `core/tests/golden/`, every preset `.toml`, `core/src/ffi.rs`, `render/scenes/mod.rs` and every
  manifest **byte-untouched** (**C ABI stays v4** at 56 bytes, `Scene` unchanged, no new
  dependency); no `aspect` anywhere in the diff; every added `expect` is in test code; and
  `core/tests/hygiene.rs` already scans `core/src/diag`.
  **Minor left open:** the overlay truncates at `MAX_QUADS = 4096` with `.min()` and no diagnostic,
  and the analysis block is built **last** — so the block that would vanish first under any future
  growth is exactly the Phase 6 instrument. Measured at roughly 1900 quads today, so there is real
  headroom; worth knowing before the next thing is added to that panel.
  Version **minor 0.26.0 → 0.27.0** (a feature plan).

- [0044 — quality tiers: `Floor` and `Rich`, a governor, and the constants that
  move](done/0044-quality-tiers.md) — **done 2026-07-30**, passed Mode 4 review (**no blockers**;
  one major, two minor). Four `dev` phase commits: `e44b3a6` `TierConfig` + resolution + the post
  cap, `89d4ad4` the frame-time governor, `6292286` the remaining capacity constants, `3e807f4` the
  docs sweep. [ADR-0045](../adrs/0045-quality-tiers-floor-and-rich.md) is **accepted**.
  **Delivers roadmap R0** — the license every later richness item spends.
  **Phase 4 (`human`, the `Rich` calibration) did not run** and is carried to
  [`on-device-validation.md`](../on-device-validation.md); the shipped `Rich` values are therefore
  still the provisional multipliers, and `TierConfig::RICH`'s own doc comment says so.
  **The design worth carrying forward is that `Floor` is enforced by construction, not by
  discipline.** `Renderer::new_headless` has no tier argument — a capture *cannot* be blessed at
  another tier by forgetting a field, and `shot` deliberately does not read `LMV_TIER` so no ambient
  environment variable can move a baseline. That is why the whole plan landed with **zero golden
  re-blesses**, which is Phase 1's byte-identical done-when proved rather than asserted. The same
  shape as Plan 0047's `SaltMode`: make the compiler, not a reviewer, the thing that forces a new
  capture path to decide.
  **Non-vacuity was built into the tests, not bolted on.** `the_rich_tier_raises_the_grid_only_where_the_floor_cap_binds`
  asserts *both* directions — larger where the cap binds, exactly equal where it does not — and
  `the_overflow_message_names_the_cap_it_carries` states outright that the floor assertions would
  pass a reverted `Display` (the floor's cap *is* 20 000) and formats at the rich cap to break that.
  The overlay test checks the tier label's characters exist in the 5x7 glyph table, because
  `glyph` returns a blank cell for an unknown one and a "named" tier could otherwise paint as a gap.
  **The one major is a cross-module coupling nothing ties together:** the governor's `MIN_SAMPLES`
  (180, `render/tier.rs`) is only satisfiable because `diag::RING` is 240 — a private const in
  another module, documented as a tunable p99 window. Lower it below 180 and the governor silently
  never demotes, and the Phase 2 suite would not notice, because every case feeds a synthetic
  360-sample series that is 1.5x the ring's entire capacity. Two minors: a lost line-continuation
  putting 18 spaces mid-sentence in the demotion message (`standalone/src/main.rs`), and
  `docs/on-device-validation.md` missing from Phase 5's sweep (fixed in the close commit, along
  with two now-stale "lower this constant" pointers to constants that moved into `tier.rs`).
  **First field consequence, same day:** `Rich`'s 3x attractor particle count blows
  `attractor_clifford` out to white against the un-fixed additive ceiling — **[backlog
  0031](../design-backlog.md)**, and the reason [0045] is now the urgent one.
- [0047 — expression randomness: `hash`, `noise`, and the seed that finally does
  something](done/0047-expression-randomness.md) — **done 2026-07-30**, passed Mode 4 review
  (**no blockers, no majors**; three minors, the two doc ones fixed in the close commit). Three
  `dev` phase commits on the `plan-0047-expression-randomness` worktree branch, merged to `main`:
  `96d39c1` the two salted functions, `d72a4cc` `seed = "random"` + the capture pin, `8f7fc13` the
  docs sweep. [ADR-0051](../adrs/0051-seeded-grammar-randomness-with-per-run-opt-in.md) is
  **accepted**. **Delivers the first R5 item** — the grammar is 17 functions, and the
  incommensurate-sine non-repetition idiom is retired for new work (`noise(time * 0.3)` in one
  call; existing presets deliberately not rewritten — Plan [0048]'s Phase 7 picks them up
  opportunistically, and no shipped preset uses either function yet).
  **The design worth carrying forward is where the pin lives.** A preset carries *two* salts
  (`salt`, `pinned_salt`) and the choice between them is a `SaltMode` **parameter threaded from
  every entry point through `draw_frame`**, not a flag on `Renderer` — so the compiler, not a
  reviewer, is what makes a new capture path decide. Reviewed all six call sites: one `Live` (the
  on-surface `render`), five `Pinned`. Salting at the renderer rather than at load is likewise
  forced, not stylistic: `default_presets()` feeds both the live C-ABI path and the behavioral
  gates, so a load-time decision would be wrong for one of them. Entropy is
  `std::collections::hash_map::RandomState` — **no new dependency**.
  **Non-vacuity was proven rather than assumed**, which is the part most closes skip: pointing
  `SaltMode::Pinned` at the live salt fails `seed.rs`'s byte-equality, and the same fixture under a
  declared seed renders differently — so the equality is a pin, not pixels that ignore `hash`.
  **Two lens-4 notes.** (1) Every shipped preset has `salt == pinned_salt == 0`, so the entire
  golden/gate suite is structurally blind to whether a capture pinned — `core/tests/seed.rs` is the
  only thing that can tell, and it covers `capture_preset` only; `capture_at_clock`,
  `capture_preset_over` and `capture_audio` rest on code inspection. That is the plan's one open
  `dev` followup. (2) The `[generator] seed` key is **not** an L-system key despite its table:
  any system's preset may carry a `[generator]` table holding nothing else.
  **Docs swept at close beyond the plan's own list:** `docs/capturing.md` (the seed pin is a
  fourth ingredient in the headless-render purity claim) and NFR §6 (ADR-0051 promised the
  clarifying sentence; §6 was already true, so this states *why* rather than carving out).

- [0043 — the swarm gets a depth axis and a domain that follows the
  target](done/0043-swarm-depth-and-domain.md) — **done 2026-07-30**, passed Mode 4 review
  (**no blockers**; two minors and a nit — both minors fixed in the close commit). Four `dev` phase
  commits: `ae6f638` the target-sized domain, `7f54e2a` `field_freq`, `7eaa848` the depth axis,
  `de707cb` the family cut to three and re-authored. **Closes design-backlog 0029 and 0025 in full.**
  [ADR-0044](../adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) is **accepted**.
  **The user-reported bright bar is gone at its cause**, not hidden: `BOUND_Y = 1.0` *was* the NDC
  frame edge, so the toroidal seam was the one place on screen every wrapping particle was guaranteed
  to paint and the feedback stage integrated it. The half-extents now follow the render target's
  aspect times `MARGIN = 1.25`, chosen by measurement against the family's working `zoom` range
  rather than rounded — at margin 1.0 the 400-frame `dynamic:110` capture reproduces the bar, at 1.25
  it is gone at both 16:9 and 16:10. **Positions moved to normalized `[-1, 1)` storage**, which is
  what makes a resize a rescale instead of a mass teleport and keeps the seeded scatter
  aspect-independent (NFR §6).
  **ADR-0037 now reaches simulation domains.** This is its first application to one, and it produced
  the review's most useful finding: the plan told `dev` to source the aspect from
  `Scene::set_target_size`, and `dev` correctly refused — that hook carries the post chain's grid,
  quantized to a 256 px step, and every swarm preset composes `trails`, so it is exactly the
  quantized case. `Scene::render`'s argument is the surface (`render/post.rs:442`). The plan text is
  corrected in place; **the pattern to carry forward is that "the target-size hook" is not a synonym
  for "the target's shape"**.
  **Depth is a 2.5D fake and the tests say why that is cheap**: one `z` per particle driving sprite
  scale, an atmospheric fade, parallax against `zoom`/`pan_*` (near traverses ~1.9x faster than far),
  and a z-dependent flow-field phase offset — the term that separates volume from a sprite sheet at
  two scales. No sort, because additive blending is commutative. Parallax rides a per-instance vertex
  attribute, so the shader holds no depth constants and reduces to the identity at zoom 1 / pan 0.
  All seven `swarm` unit tests carry a counter-assertion that makes them non-vacuous — the replaced
  constants genuinely disagree with 16:10, a world-space store genuinely teleports by >2 world units,
  the identity transform genuinely is depth-independent.
  **One acceptance criterion is outstanding and is not a defect:** Phase 3 named NFR §1/§9's ≥ 60 fps
  @ 1080p iGPU floor, which no session can measure — the dev-box marginal cost is **+0.5 ms/frame**
  (1.03–1.09 → 1.56–1.58 over 5000 frames at 1920x1080), and it is not fill rate. Carried forward as
  a `docs/on-device-validation.md` item; if it misses, the lever is `PARTICLES` and that routes back
  here rather than being taken silently. **[0044]'s Phase 3 should treat the particle count as a live
  tier candidate.**
  Content: the family is **three** presets separated for the first time by *structure* rather than
  palette and band driver — Drift ~1.9, Storm ~3.0, Dense ~5.2 of `field_freq` — with `anim` improved
  on all three against the pre-plan baseline (0.090→0.098, 0.041→0.050, 0.044→0.051) and the reduced
  set passing `sanity` / `reactivity` / `animation` / `distinctness`. The counter-intuitive result is
  documented in `presets/README.md`: it is the **low** end of `field_freq` that reads busier, because
  a field structure larger than the frame packs it edge to edge.

- [0042 — reachability sees every comparison, and the library is re-audited against
  it](done/0042-reachability-sees-every-comparison.md) — **done 2026-07-30**, passed Mode 4 review
  (**no blockers**; one major, two minors, one nit — the major and both minors fixed in the close
  commit). Three `dev` phase commits: `8c170a3` observe every comparison, `e7a40b7` report a
  one-sided one unless a `select()` already names it, `f50e8cf` the Phase 3 re-audit. **Closes
  design-backlog 0028.** [ADR-0043](../adrs/0043-reachability-reports-comparison-nodes.md) is
  **accepted**.
  **The library is clean, and that is a measurement rather than an assumption** — re-run at review
  time, not taken on trust: 16 gate flags across the shipped set, **every one** the standing `tempo`
  single-BPM false positive, **0 genuinely dead**. The two negative results are the real deliverable
  and neither was obtainable before this plan: the band halves inside `min(tempo > 132, bass + mid >
  0.055)` and `min(tempo > 124, bass + treb > 0.1)` each stayed unflagged while their tempo half
  emitted a `COMP`, and all seven bare-comparison bindings (six `attractor_* reseed`, plus
  `rose_web.mirror_reflect`) score clean — direct confirmation the `e9a1c3c` content re-gain took.
  **The blocker on CI gating has changed identity:** the library was the precondition and it is met;
  what remains is the instrument, so a multi-BPM probe or an explicit `tempo` exemption is now the
  single thing between here and a meaningful green gate. `docs/capturing.md` no longer justifies the
  advisory posture with the nine-failing-presets figure this plan measured to zero.

- [0041 — `--report` reads at two levels, and expression reachability is measured on the
  AST](done/0041-report-two-level-stimuli-and-expression-reachability.md) — **done 2026-07-29**,
  passed Mode 4 review (**no blockers**; one major, four minors, three nits — one minor fixed in the
  close commit). Four `dev` phase commits: `1c8f216` the realistic-level stimulus, `5901e0e` probed
  evaluation, `27c84cd` the report surface, `efd25bb` the doc sweep. **Closes design-backlog 0022 and
  0027 outright, and 0020's harness half.**
  [ADR-0042](../adrs/0042-reachability-measured-on-the-expression-tree.md) is **accepted with an
  Outcome section**.
  **The instrument can now see the defect it was blind to**, and that was verified by re-running it
  rather than taken on trust: `--report --presets presets` flags `attractor_dejong`
  (`bass + mid > 0.34`), `attractor_lorenz` (`bass + treb > 0.38`) and `fragment_warp`
  (`bass + treb > 0.55`), and clears `fragment_kaleido`, `reaction_reef` and `lsystem_arrowhead`,
  which were recalibrated on 2026-07-28. That contrast — a library containing both, discriminated
  correctly — is Phase 3's acceptance test, reproduced at review. It also caught `lsystem_fern` and
  `star_rosette`, which the plan did not name.
  **Phase 2 shipped a third shape, better than either the plan's primary or its named fallback.**
  The plan offered a duplicated `eval` body pinned by an equality test, with a generic
  zero-sized-vs-recording observer as the fallback if the duplication looked fragile. `dev` took
  neither: the probe walk **only records**, and `eval_probed` returns `Expr::eval`'s own value. The
  divergence ADR-0042 named as this approach's main cost is **unrepresentable** rather than merely
  tested for, and the library-wide equality assertion survives as a regression guard instead of as
  the load-bearing proof. Said so in the phase commit, as the plan's risk entry required. The
  property that makes it work — a node's index does not move with the branch a run took, so an
  untaken subtree still occupies its slots — is load-bearing and carries a test the plan never asked
  for: without it two one-sided sibling gates merge into a healthy-looking two-sided reading and
  neither is reported.
  **The non-breaking claim is structural, not just diffed.** `Renderer::capture_preset` calls
  `reset_for_capture` first, so interleaving the four low-level captures between the full-scale ones
  and the `late` capture cannot move the existing columns; `FULL_LEVELS = [1.0; 4]` reproduces the
  old `band_stimulus` exactly, including the onset frame's `spectrum: [1.0; SPECTRUM_BINS]`.
  Corroborated independently at review: the run reads `Kaleido Field` bass **0.228**, exactly the
  figure backlog 0013 recorded before this plan. `docs/capturing.md`'s worked example block is a real
  measurement — `Aurora 0.110 0.010 0.020 0.001 0 9` and `Warp Drive 0.040 0.009 0.008 0.122 2 11`
  reproduce to the digit.
  **The layout call went to a second block rather than four more columns**, so the table stayed nine
  wide and every number a previous run printed is still in the same place — the "thirteen-ish
  columns" ADR-0042 worried about never happened.
  **Verified at review** rather than taken on trust: `fmt --check` + `clippy --workspace
  --all-targets -D warnings` clean; `nextest --workspace` **287/287, 0 skipped**;
  `core/tests/golden/` **byte-untouched**; `core/src/ffi.rs`, `render/scenes/mod.rs`, every manifest
  and **every preset `.toml`** untouched (**C ABI stays v4**, `Scene` unchanged, no new dependency).
  The hot-path pragma at `expr.rs:37` still covers the module, the new code adds no `unwrap`/`expect`,
  and both allocation-counter tests still pass. The probe is deterministic (`dynamic_groove`, no wall
  clock, no RNG) and the test sweep is a fixed-seed LCG. No `aspect` anywhere in the diff.
  **Major:** `standalone/examples/shot.rs:933-943` re-types the render path's nine positional
  `Variables::new` arguments from `core/src/render/mod.rs:1192-1203`, with nothing tying the two
  together. Add a tenth variable or reorder two and the probe binds different values than the engine,
  so every flag would describe an expression the renderer never evaluates — which is exactly the
  "the report describes a preset that does not exist" failure ADR-0042 named and that Phase 2 went
  out of its way to make unrepresentable one level down. Two sources that agree on today's
  configuration, and no test can say which one the code used. Wants a shared
  `Variables::from_frame(&AnalysisFrame, time)` in `core`, or a test asserting the two agree.
  **Minors:** (1) the ceiling check flags on strict `peak_fraction_of_bound < 1.0` where ADR-0042
  says "approached", so a bound reached at **99 %** is named among a family's "furthest from biting"
  (`Spectrum Corona.trails`); a named threshold would make the 159-flag count mean *decorative*.
  (2) `docs/preset-palettes.md` was not swept for backlog 0027's `color_center` half — the entry
  names that file explicitly and the plan's Phase 4 file list omitted it, leaving the canonical
  colour doc silent on the wrap while `presets/README.md` explained it. That file is one of the three
  the `preset-author` lane is pointed at *instead of* keeping its own catalogue, so the gap is the
  exact failure mode that sweep exists to prevent — **fixed in this close commit**, for both
  `color_center` and `hue_center`. (3) Nothing pins the new JSON fields:
  `standalone/tests/shot_cli.rs:406` checks brace balance and two top-level keys only. Verified by
  hand at review — 32 presets each carry `reactivity_low`, `reachability`, `dead_branches`,
  `unapproached_ceilings` and `probe`, with 159 `peak_fraction_of_bound` entries. (4) Phase 3's
  acceptance contrast has no regression guard, **disclosed by `dev`** on the reasoning that it is a
  fact about preset content the follow-up re-gaining pass is meant to change. That is the right call,
  and it means the pass will remove the only presets currently demonstrating the check discriminates.
  **Nits:** `Expr::node_count()` is `pub` and called nowhere outside its module — dead public surface
  on the shared core; `probe_reachability` hardcodes `48_000.0` for `hop_seconds` beside a `format`
  it just built with `sample_rate: 48_000`; and both `presets/README.md`'s table and `shot.rs`'s
  `LOW_LEVELS` comment print an `onset` mean of `0.002` while the constant is `0.0016`, so a reader
  checking the arithmetic against the table is off by 25 %.
  **⚠ Nothing new for the on-device pass** — `Expr::eval` is untouched and every addition lives in
  the `shot`/harness path; the app's render loop executes nothing new.
  Version **minor 0.21.1 → 0.22.0** (a feature plan).

- [0040 — Line joins, finished: the star's other half, and a pin under the reported
  defect](done/0040-line-joins-finish-the-job.md) — **done 2026-07-28**, passed Mode 4 review
  (**no blockers, no majors**; two minors, two nits). Three `dev` commits: `4c68bbd` the pixel pin
  under the polyline joint, `434ac1d` the join bits generated into the shader plus a swap test,
  `0bc33a6` the star rosette's contact points. **Closes backlog 0024.** No new ADR —
  [ADR-0041](../adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md) had already
  decided the mechanism and now carries a **Plan 0040 note in its Outcome**.
  **The star rosette is fully joined.** Both segments at a contact point take `JOINED_A | JOINED_B`,
  so all `2n` vertices of the closed chain are flagged rather than the `n` tips Plan 0039 reached.
  The shipped test was **replaced, not amended** — it asserted only that the two contact points
  *within* a petal are distinct, which is true and silent about the sharing *across* petals, so it
  passed unchanged both before and after the fix; the replacement asserts that segments `2k + 1` and
  `2k + 2` meet at a shared contact point and both declare that end joined, using `close` on the
  wrap-around pair because `contact(n)` and `contact(0)` are `cos(TAU)` against `cos(0)`.
  **Phase 3's stopping condition was exercised and it cleared — with the plan's expectation
  reversed.** `dev` captured the star at `contact_angle = 8` (the `CONTACT_MIN_DEG` floor) and at
  40, before and after, full-frame and at 6x/20x with thickness scaled to the zoom so magnification
  could not flatter the fix. At 8 degrees there is **no separable bead**: the two extensions run
  nearly parallel and merge into the already-bright core, turning a hollow flat cut into a point that
  ends in a point. At 40 degrees the dark V fills and leaves a compact bright dot. So the
  near-reversal case ADR-0041 and this plan both worried about is the **benign** one, and the bead is
  most distinct mid-range — recorded in the ADR's Outcome and in `presets/README.md`, which now names
  `[generator] contact_angle_deg` as the lever, not a `[params]` binding. **No route-back, no miter
  limit.**
  **Phase 2 chose generation over assertion, which is stronger than the plan asked for.**
  `shader_source()` emits `const JOINED_A` / `const JOINED_B` into the WGSL from the Rust constants
  at pipeline build, so a **renumbering** is unrepresentable rather than merely detected; a **swap**
  (still hand-writable in the shader) is caught by a new assertion that the stroke does not reach
  past the figure's own first and last points — the only endpoints carrying a single bit, since an
  interior joint carries both and renders identically either way. Lit-or-not is classified between
  two regimes measured **in the same capture** (background from the frame, stroke from the dimmest
  interior probe), so no constant is introduced, and the probe sits **half** an extension out rather
  than a full one, which would land on the exact end-cut a swapped quad draws.
  **The plan's Phase 2 done-when 2 premise was slightly wrong and `dev` said so instead of working
  around it.** A swap *is* caught by the pre-existing local-minimum assertion — at element 1 only,
  reading `0.4510` against `0.4588`. That `0.008` margin is inside the noise of any rasterization
  change, so "nothing catches a swap" was directionally right and literally wrong; the replacement's
  `~0.23` margins on both ends are the answer either way. `dev` disabled the old assertion
  temporarily to prove the new one fires on its own merits (`0.4911` against a `0.2294` cutoff), then
  restored it.
  **Phase 1's pin is ordered so the notch cannot be blessed back in.** `line_joints.rs` compares
  against `golden/line_joint_zigzag.png` following `composite.rs` — its own compare, its own
  `LMV_BLESS`, its own binary — and the **relative assertion runs first, including under
  `LMV_BLESS`**, so a reopened notch aborts before the bless can run. That ordering is the whole
  answer to "a baseline can always be re-blessed". One capture serves all three duties, deliberately:
  a second `Renderer::new_headless` mid-run is the thing `composite.rs` documents as changing what
  WARP resolves. **Bless scoping was checked rather than assumed** (one file written, ten baselines
  untouched, `golden.rs` not built by that invocation), and Phase 3's `LMV_BLESS` over `--test
  golden` did rewrite `fragment_field.png` and `swarm.png` on WARP noise — both restored before
  staging, the standing trap behaving exactly as [0039] and [0033] recorded it.
  **Verified at review** rather than taken on trust: `fmt --check` + `clippy --workspace
  --all-targets -D warnings` clean; `cargo nextest run --workspace` **280/280, 0 skipped**; the new
  baseline reproduces **bit-exact** (mean `0.0000`, outlier `0`) and the probe still reads ADR-0041's
  recorded numbers (joint `0.6431`/`0.6440` against interiors `0.4885`/`0.4588`); `git diff --stat`
  over the range confirms `star_pattern.png` is the **only** baseline that moved and
  `line_joint_zigzag.png` the only one added; both composite fixtures were opened and are
  `parametric_curve` figures with no star, and both still read `mean 0.0000`; `ffi.rs`,
  `scenes/mod.rs` and every manifest are untouched (**C ABI stays v4**, `Scene` unchanged, no new
  dependency, no preset `.toml` change). The join extension still takes its aspect from the render
  target's uniform, and the non-square coverage for it lives in `composite.rs`'s 160x100 rose
  fixtures — `line_joints.rs` is square **on purpose**, so its world coordinates are NDC and the
  probe arithmetic is derivable.
  **Minors:** (1) `dev`'s close report that `--test transition`'s 12 tests "are not executing on this
  machine" is **wrong, though the crash it rests on is real**. The abort reproduces at `3e4dec5` in a
  scratch worktree, so it is genuinely pre-existing — but it is the known in-process parallel-GPU
  teardown fault ([0035]'s close recorded the same thing for `--lib render::post`), and all 12 tests
  pass in 3.2 s under `cargo nextest run -p lmv-core --test transition`. There is no coverage gap.
  (2) `docs/capturing.md` is what licensed that conclusion — it said `cargo test -p lmv-core` is
  "also fine, except where noted below" and the note below covered only `preset`'s allocation
  counter — **fixed in this close commit**, which now names the `0xc0000005` abort as a runner
  artifact and says to re-run under nextest before concluding anything about coverage.
  **Nits:** the generated prelude shifts every WGSL line number by two relative to `SHADER_BODY`, so
  a naga compile error's reported line no longer matches what you count in `renderer.rs`; and
  `assert_no_notch` returns `f32::INFINITY` for an empty joint list, guarded by an assertion above it
  rather than by construction. **⚠ Nothing new for the on-device pass** — no instance count changed,
  the flag was already in the instance, and the shader work per vertex is identical; the plan
  disclosed that cost as asserted-negligible and claimed no number, which stands.
  Version **patch 0.21.0 → 0.21.1** (a fix/coverage/refactor plan, no feature).

- [0039 — Line joins: the stroke stops coming apart at every vertex](done/0039-line-joins.md) —
  **done 2026-07-28**, passed Mode 4 review (**no blockers**; one major, three minors, two nits —
  nothing fixed in the close commit, the major is deliberately backlogged instead). Four `dev`
  commits: `5dfc81c` the per-endpoint `joined` bitfield plus the shader extension, `b184021` the
  spectrum polyline and the new `core/tests/line_joints.rs`, `12e6ab2` the rose / L-system / star,
  `f78ff2f` the doc sweep. **Closes backlog 0023; opens backlog 0024.**
  [ADR-0041](../adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md) is **accepted with
  an Outcome section**. The design's load-bearing claim held in fact: with every producer flagging
  nothing, **no line-scene baseline moved** — so the later re-blesses are attributable to specific
  scenes rather than to the primitive, which is what a walking-skeleton phase is for. The new pixel
  test is threshold-free and was proven to fail first (`0.0000` at the joint against interiors of
  `0.4885`/`0.4588`), and it **measured** the overshoot the ADR accepted in place of a miter limit
  rather than leaving it a prediction: a sharp joint now reads `0.6431`, brighter than either
  interior. Two things the plan got wrong, both caught by `dev` and handled honestly rather than
  papered over: Phase 2's "re-bless the polyline goldens" was **vacuous** (the golden fixture takes
  the default `bars` layout, so no polyline golden exists — `spectrum_ridge` is guarded behaviorally
  per ADR-0023), and Phase 3's golden enumeration **missed `composite_trails` and
  `composite_kaleido`**, which draw a `maurer_rose` through a post stage and so moved from the same
  cause. The review's one major is the star rosette: its contact points are shared between adjacent
  petals, making the figure a closed chain whose every vertex is a joint, so Phase 3 flagged only
  half of them. **Phase 5 (`human`) is open** — `spectrum_ridge`'s `thickness` re-tune.
- [0038 — The line family's unreachable levers: `glow`, the readout's geometry, a level curve, and
  `log`](done/0038-line-family-unreachable-levers.md) — **done 2026-07-28**, passed Mode 4 review
  (**no blockers**; one major, five minors, two nits — the major and three minors **fixed in the
  close commit**). Ten commits: eight `dev` phases (`a1c67f4` `glow`, `f3945be` `span`/`baseline`,
  `c9121fd` `curve`, `e31ae88` `log(x)`, `a3f5d04` the doc sweep, `9739232` the settle gate,
  `4863bdd` the marked transient cell, `9a62754` the non-finite guard) plus `8e84acf` + `ea781d0`,
  the Phase 6 `preset-author` adoption pass. **Closes backlog 0016, 0017, 0018 and 0019 outright.**
  The plan's central safety claim **held in fact**: `core/tests/golden/` is **byte-untouched across
  the whole range**, so every one of the four new params really does default to the constant it
  replaced. `span` and `baseline` are **world** quantities and the ADR-0037 trap was avoided by
  construction — `grep aspect` over `spectrum.rs` finds it only being *passed through* to
  `LineRenderer::draw`, never read to size anything, and a unit test asserts doubling `span` exactly
  doubles an x coordinate and moves no y.
  **The plan's own risk entry did its job and the ADR lost.** Phase 3 measured ADR-0040's
  curve-vs-easing ordering with Plan 0037's probe, found against the stated rationale, **retuned
  nothing and routed to `architect`** exactly as instructed. The ruling:
  [**ADR-0040 is accepted with an Outcome section**](../adrs/0040-spectrum-level-curve-applies-before-the-easing.md#outcome-2026-07-28-after-plan-0038-phase-3s-measurement)
  — the ordering stands and no scene code changed, but "a perceptually even fall" is a property **no
  ordering can deliver**: for a step to silence both orderings are exponentials of identical shape,
  and an exponential covers the first half of its travel in `ln2 / ln10` = 30 % of its settling time
  whatever `curve` is. The real difference is that ease-then-curve would make the effective release
  `release / curve`, where the shipped order leaves `release` naming the same duration at any
  `curve`. **The general lesson is worth more than the ruling: a claim about the shape of a one-pole
  is arithmetic before it is a measurement**, and two lines of algebra on `Easing::step` would have
  caught it at design time.
  **Half of that measurement was the instrument, and fixing it is the plan's most durable output.**
  `metrics::frames_to_settle` normalizes against **the segment's own last frame**, so a response
  still travelling at the end of its window supplies a short total, crosses every threshold early,
  and returns a *plausible* smaller number — and because normalizing against the last frame
  guarantees the threshold is crossed inside the segment, `frames_to_settle(seg, f) < seg.len()` was
  a **tautology, not a check**. The guard written against exactly this (`fall_frames < WINDOW`,
  commented "clamped rather than measured") was unreachable by construction. `metrics::segment_settled`
  now extrapolates the geometric tail from **three points spread across the segment** — deliberately
  not from adjacent frames, because at 8 bits a response slow enough to outrun its window moves by
  *less than one code value per frame* near the end, so the adjacent-frame version reported a
  response settled with 37 % of its travel left. **The shared probe turned out to be truncated
  itself**: `easing_asymmetric.toml`'s `release = 0.5` against a 96-frame window is 3.2 τ, and the
  suite printed **61** where the closed form and its own fixture header both say **69**. `WINDOW` is
  now 180 (6 τ, a 0.25 % residual), every existing ratio bound survived untouched, and the suite
  costs 9.0 → 12.1 s inside the pre-push gate. `--report` **marks** rather than widens: a `+` suffix
  and `rise_settled`/`fall_settled` in `--json`, with each family naming how many cells marked.
  **Measured: 38 of 38 presets mark**, and the report says plainly that the suffix cannot separate
  its two causes — a window a release outran, or a scene whose own motion has **no asymptote at
  all**, which is the commoner one here. `tol` was not loosened to make the table quieter.
  **Phase 9 is a real live defect Phase 4 created and the review caught**: `log(0)` is `-inf`, which
  silence produces every time the music stops, and `Easing::step` computed `-inf + alpha * NaN` =
  NaN — **absorbing**, because `raw > held` is false for every `raw`, so the release branch was taken
  forever and **the binding was dead for the rest of the preset's run**, recovering only on a switch.
  The guard sits in `Easing::step` (the single implementation both smoothers call) and checks **both
  operands**, because a stored `-inf` against a *finite* `raw` selects `attack` and computes
  `-inf + inf` on the very next frame. `sqrt(-1)` could already reach it, so the defect predates
  Phase 4 — but `sqrt` needs a contrived negative argument where `log` needs silence.
  **Verified at review** rather than taken on trust: `fmt --check` + `clippy --workspace
  --all-targets -D warnings` clean, `nextest --workspace` **273/273, 0 skipped**,
  `core/tests/golden/` byte-untouched, and `ffi.rs` / `scenes/mod.rs` / all four manifests untouched
  (**C ABI stays v4**, `Scene` unchanged, no new dependency). **Both new guards reproduced
  non-vacuously**: reverting `WINDOW` to 96 fails the shared probe on the **asymmetric fall only**
  (scalar both directions and asymmetric rise still pass), printing exactly the predicted `61`; and
  deleting the `Easing::step` finite check fails
  `a_non_finite_value_cannot_poison_a_smoother_permanently`. The `--report` table was re-run
  independently over `presets/` and matches `ea781d0`'s numbers.
  **Major, fixed here:** `spectrum_comb` and `spectrum_ridge` shipped headers whose stated
  arithmetic is for a **superseded** tuning — `ea781d0` retuned the bindings and swept only part of
  the prose. The comb argued `curve = 0.72` with a 4.2x / 1.9x lift while shipping `0.85` (where the
  same levels lift **2.2x / 1.4x**), and named `base` 0.08, `scale` 7.0 and a `12 -> 20` stroke
  against shipped `0.13`, `10.0` and `13`; the ridge's whole retune paragraph was for `curve 0.55`
  and described `scale` as a **2.75x cut** where it shipped as a **2.05x rise** (2.2 → 4.50) — the
  opposite direction. That matters because Phase 6 done-when 4 made "says so in its header, with the
  factor" a contract precisely so the content lane stops guessing, and `spectrum_corona` (whose
  numbers are correct) shows the intended standard. Every figure recomputed against the shipped
  bindings; **no binding changed**. `ea781d0`'s commit body carries the same 4.2x / 1.9x slip and
  stands as the historical record.
  **Minors:** (1) backlog 0016–0019 carried no closure markers though the plan header names them —
  **added here**; (2) backlog 0023 said `spectrum_ridge` "now carries `thickness = 3.2`" where it
  ships `4.2`, which is the entry [0039] builds on — **fixed here**; (3) `README.md`'s pre-push
  "~39 s" predated `WINDOW` 96 → 180 (measured 39.7 s of nextest now) — **fixed here**; (4) Phase 8's
  commit body answers "over which presets" for seven families totalling 35 and **omits `spectrum`**,
  the plan's own subject system, which also marks 3 of 3 (measured at review) — so 38 of 38, not 35;
  (5) backlog 0022 quotes `curve = 0.62` / `scale 1.75` in the present tense from a tuning superseded
  the same day — annotated here rather than restated, since the defect is structural and survives any
  exponent. **Nits:** `segment_settled` returns `true` for an **empty** segment ("no claim to
  invalidate"), which is the permissive direction for a gate whose purpose is refusing uncertified
  numbers — reachable through `probe_response`'s `unwrap_or_default()`, though today's window
  arithmetic never produces one; and `Placement::default()`'s doc says "the scene's own defaults
  rather than zeroes" while `radius` is exactly `0.0` against a scene default of `0.35` (tests only).
  **⚠ Unmeasured, as the plan disclosed:** the per-frame cost of up to 64 `powf` calls is asserted
  negligible against the existing per-element work, and no number is claimed. Nothing new for the
  on-device pass otherwise — the render loop's structure is unchanged and the probe work lives
  entirely in the capture/`shot` path. **Three entries route onward:**
  [0021](../design-backlog.md) (an even fall is unreachable with a one-pole in any ordering),
  [0022](../design-backlog.md) (`--report`'s reactivity columns are structurally blind to a `curve`,
  because its stimuli are full-scale and `1^curve` is `1`), and [0023](../design-backlog.md), which
  is already [Plan 0039](done/0039-line-joins.md) + [ADR-0041](../adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md).
  Version **minor 0.19.0 → 0.20.0** (a feature plan).

- [0037 — Verifying easing: a transient probe, a signal with dynamics, and the levels authors
  calibrate against](done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) — **done
  2026-07-27**, passed Mode 4 review (**no blockers, no majors**; four minors, four nits). Five phase
  commits (`ece3291` the time-varying stimulus + the step-response measure, `29bc035` the `--report`
  columns, `6de5ad0` `--signal dynamic`, `bca1457` the doc sweep, `b3f18a6` the `human` measurement).
  **`[smoothing]` is observable.** `Renderer::capture_preset_over(name, stimulus)` renders one frame
  per `AnalysisFrame` and reads each back; `metrics::step_response(rise, fall) -> StepResponse` turns
  two segments into frames-to-settle each way. The identity ADR-0039 opened with — the report is the
  same for any easing constant — is broken. The new method is a **sibling** of `capture_preset`, not
  a generalization: the old one reads back once per *call*, the new one once per *frame*, so folding
  them would have made `sanity`/`reactivity`/`animation`/`--report` an order of magnitude slower;
  they share `reset_for_capture`, and a test pins them byte-for-byte. The measure works in **linear
  light** and that is load-bearing — sRGB's concave transfer curve makes a symmetrically eased
  parameter cross 90 % of its *pixel* change early up and late down, and a unit test shows the sRGB
  reading skewed past 2x, so reusing `frame_diff` would have made every scalar entry read asymmetric.
  **Phase 2's done-when 2 is a reported partial negative, as the plan allowed for**: on the
  purpose-built near-linear fixture the pair reads `3 / 61` against the scalar's `34 / 35`, but over
  the shipped set the separation is **directional only** — asymmetric median `fall/rise` 1.02
  (12/24 with `fall > rise`) against scalar-only 0.61 (0/14) — and several presets read `fall` *below*
  `rise`, which is the scene's own motion being measured. **Both confounds were tested and neither is
  the cause**: `dev` reran at a 96-frame window and the separation got *worse* (scalar-only 0.60 →
  0.92) for double the wall clock; review rebuilt at `PROBE_SIZE = 192` and **every reading moved by
  at most two frames**, so `capturing.md`'s "what it measures is temporal, so resolution buys nothing"
  is earned. The scene's visual response is what hides the magnitude, exactly as ADR-0039 predicted,
  and it is why **no CI gate** ships. `--signal dynamic:<bpm>` is the first generator with dynamics —
  three layers on a beat grid under an 8-beat build-and-rest phrase, `max/mean` 2.67 / 3.07 / 5.45
  against `bass:60`'s exactly 1.000 — and its test asserts that comparison against noise measured **in
  the same run** rather than a remembered constant. Three design dead ends are recorded in `6de5ad0`
  and worth reading before touching it: a bare 220/277/330 chord puts almost nothing in `mid` (that
  band is a *mean* over 250 Hz–4 kHz), a 1 %-duty tick is "silence with a good crest factor", and peak
  normalization makes three layers **zero-sum**, so it soft-clips with `tanh` instead. **Phase 4
  (`human`) ran** and produced the calibration number the harness had wanted since backlog 0008: real
  material peaks where a full-scale sine does (808 bass `0.190` against `0.187`) but its **mean** is
  `0.007`, so *percussive* bindings calibrated against a tone are roughly right and *continuous* ones
  are badly over-gained — `capturing.md` now carries the ladder from `--set 0.8` (~100x) down through
  `dynamic:110` (~6x). **Verified at review** rather than taken on trust: `fmt --check` + `clippy
  --workspace --all-targets -D warnings` clean, `nextest --workspace` **263/263**,
  `core/tests/golden/` **byte-untouched**, and `ffi.rs` / `scenes/mod.rs` / all four manifests
  untouched (**C ABI stays v4**, `Scene` unchanged, no new dependency, no preset `.toml` change).
  **Non-vacuity reproduced independently** — swapping the two fixtures' `[smoothing]` tables fails at
  `easing.rs:194` reporting *rise 3 fall 61 (ratio 20.33)* where it demands symmetry — and **Phase 2's
  statistic recomputed** from the JSON report over `presets/`, matching `29bc035` to a rounding digit
  across a debug→release build change. **One edit outside a phase's file list, disclosed in its
  commit and accepted:** one `print_usage` line enumerating the `--signal` kinds. **The plan's seed
  swatches for backlog 0014 were wrong and are corrected in both places** — the recorded names are the
  ramp ~0.16 further along than `palette(t)` produces; `dev` settled it with a 15-point rendered sweep
  measuring median chromaticity, review re-derived it, and the 20-row table in
  `docs/preset-palettes.md` is the verified ramp. **Backlog 0008 (item 3), 0012, 0013 and 0014 all
  close here. Two entries route onward:** **0020** is new (the shipped library is gained against
  stimuli 6–100x hotter than real music *on the mean*; peaks are fine, and nobody has audited how much
  of the set is actually mis-gained), and **0015 is no longer documentation-only** — the user's call,
  from the listening test, is that the half-linear band axis is a real limitation, which makes it the
  repo's **next ADR**. **Minors:** (1) `README.md`'s pre-push gate said "~28 s" where the narrowed set
  now measures **38 s** — the dominant new cost is `shot_cli`'s full-library `--report`, not the
  `easing` binary — **fixed in this close commit**; (2) `core/src/signal.rs:106-110`'s rustdoc still
  describes the *abandoned* design (an off-beat tick, a three-note 220–330 Hz chord) that the inline
  comments 60 lines below explain being replaced; (3) `capturing.md`'s library-precedence section does
  not warn that `%APPDATA%` is seeded **write-if-absent** and never refreshed — measured at review, a
  default `--report` there ran **36 presets against the repo's 38** and read `Aurora` `1 / 1` against
  `34 / 16` from `presets/`, so the new columns are much more sensitive to that stale cache than the
  old ones; (4) Trap 2's "roughly four times" doesn't link forward to the new ~100x ladder.
  **Nits:** an unreachable `unwrap_or(StepResponse{0,0})` in the report loop that would report a
  mismatch as "no transient"; a determinism-test message naming "peak normalization" in the one
  generator that deliberately doesn't peak-normalize; "90 ms of decay" against an `exp(-t*16)` 62 ms
  constant; and `capture_preset_over`'s unbounded per-frame image `Vec` (~3.8 MB at the probe's size,
  a foot-gun at 4K). **⚠ Nothing new for the on-device pass** — the probe and the generator live
  entirely in the capture/`shot` path; the app's render loop is untouched.
  **[ADR-0039](../adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) accepted.**
  Version **minor 0.18.0 -> 0.19.0**.

- [0034 — Preset-reachable spectrum: `bin(x)`, a spectrum scene, and per-element
  evaluation](done/0034-preset-reachable-spectrum.md) — **done 2026-07-27**, passed Mode 4 review
  (**no blockers**; two majors, four minors, two nits, **all fixed in `ca99cb1`** rather than
  carried). Five `dev` phase commits (`a379b28` `bin(x)`, `2450c2a` the `spectrum` system, `a553b2e`
  the `[spectrum]` table, `6950c94` per-element `index`, `fe11659` the operator sweep) plus
  `ca99cb1` the review fixes and `4d41884` the band-axis documentation correction. Closes backlog
  0002, the capability the user asked for twice. The scoping claim **held**: no new DSP, no new
  render idiom, no `Scene`-trait change, **C ABI stays v4**, no new dependency — the 64-band array
  already existed on `AnalysisFrame`, every scene already received it, and `LineRenderer` already
  drew arbitrary segment lists, so an eighth `SystemKind` cost an exhaustive-match arm rather than a
  pipeline. `Variables` beat its own feared 264-byte per-binding copy by **borrowing with a
  lifetime**. **Major 1:** a gradient that *repeats* past its ends is not *continuous* there — all
  four stop-list palettes run dark to light, so a full `hue_spread` walk puts the sharpest transition
  on the ring; `Spectrum Corona` demonstrated the falsehood it was written to illustrate and its
  palettes were re-cut to return to their starting colour. **Major 2:** Phase 2 lit the band array in
  `sanity`/`reactivity`/`golden`/`distinctness` **but not in `shot`**, so the lane's own verification
  surface scored spectrum presets on their scalar bindings alone (`Spectrum Comb` bass 0.040 → 0.084,
  onset 0.000 → 0.119, coverage 0.664 → 0.913 once fixed). `--set` stays scalar-only **deliberately**,
  documented in `docs/capturing.md` as its third calibration trap. **The substantive postscript:**
  ADR-0036 and the plan both stated the band resolution profile **backwards**. The array is
  **35 Hz–18 kHz**, not 20 Hz–Nyquist, and `fft.rs` floors every band at one FFT bin *after* laying
  the log edges — 23.4 Hz at 2048 — which binds to **band 30 (~750 Hz)**, so **31 of the 64 bands are
  linear**. Band 0 spans 23–47 Hz, *a full octave in one number*; resolution peaks near 500–800 Hz and
  settles at ~1.7 semitones above 1 kHz, so **the low end is the coarsest region musically, not the
  finest** — and below the crossover the mapping moves with the sample rate. The error propagated once
  before it was caught (`037825d` annotated its probes from the log-edge curve, **up to 2.9x wrong**
  below the crossover; bindings were tuned by effect and unchanged, comments corrected).
  **[ADR-0036](../adrs/0036-preset-reachable-spectrum.md) accepted with an Outcome section.**
  **Verified at close:** `fmt --check` + `clippy --workspace --all-targets -D warnings` clean,
  `nextest --workspace` **251/251**, `core/tests/golden/` **byte-untouched**. **Two items routed to
  the backlog:** [0015](../design-backlog.md) whether the half-linear axis is a defect (**ADR-worthy
  if acted on**) and [0016](../design-backlog.md) the readout's missing `span`/`width` param. Version
  **minor 0.17.1 → 0.18.0**.

- [0035 — The composite's aspect is the target's: the grid-shape stretch, one grid policy, and a
  pixel guard for the post stages](done/0035-composite-aspect-and-grid-policy.md) — **done
  2026-07-26**, passed Mode 4 review (**no blockers, no majors**; four minors, one nit). Four `dev`
  phase commits (`d4f98f8` the aspect fix, `687621b` the two post-stage baselines, `bc11b23` one grid
  policy, `f9f9e79` the docs). Turning `trails` or `kaleido_*` on now changes the picture's
  **softness and nothing else**: `SceneTarget::aspect` comes from `surface`, the kaleidoscope folds
  about the render target's ratio, and `Scene::set_target_size` keeps the grid because that one
  genuinely is a texel count. The two stretches cancel — a scene told the target's aspect draws
  pre-squashed into a differently-shaped grid and the present's normalized blit is exactly the
  inverse. Plan [0029] Phase 5's attractor fix stops being conditional on no stage being active, and
  **[ADR-0037](../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)'s invariant is now
  checkable by inspection: no `aspect` anywhere in `core/src` is derived from a grid size**.
  `PostStage::resolve` takes **`surface`**, not the destination's own size — deliberately stronger
  than the plan's illustrative sketch, because every present down the chain is a stretch, so a stage
  added *after* the fold would otherwise re-plant the same trap. The composite's two stages have
  their **first capture-level coverage** (`core/tests/composite.rs`, its own test binary): one fixture
  composes `trails`, one composes `kaleido_*`, captured at **160x100** — a size whose grid is 256x256,
  so the defect was a 1.6x stretch there, stronger than the 1.28x at 1280x800 and at a sixty-fourth of
  the pixels. A square or 16:9 size comes back aspect-exact and would make the guard blind, which is
  exactly why the defect survived at 1920x1080; **do not "tidy" that size**. WARP was settled by
  measurement (both baselines byte-identical by SHA-256 across three consecutive runs), so they take
  the ordinary blessed posture rather than skip-on-software — chosen so the guard runs in CI, not only
  on developer machines. `composite_kaleido.png` **pins design-backlog 0010 on purpose** at order 6:
  there is no clean order (at order 2 a corner radius still overruns the rectangle along x, below 2
  the stage is skipped), and its header is candid that the baseline *looks* clean because a centred
  figure leaves the source border a smooth gradient. `post.rs::internal_grid_size` and
  `particles::trail_grid_size` are now thin wrappers over one `render/grid.rs::grid_size(surface, cap,
  step)`, **with their caps deliberately different** (1920x1080 for the stages under ADR-0034's
  dual-live arithmetic, 2560x1440 for the attractor under [0029]'s fill budget) and a test pinning
  that difference so an equalizing "cleanup" reads as the memory regression it would be — the
  duplication was not incidental to the defect, it was the mechanism. **Verified at review** rather
  than taken on trust: `fmt --check` + `clippy --workspace --all-targets -D warnings` clean; `nextest
  -p lmv-core` **171/171**; `git diff --stat` over the range confirms `core/tests/golden/` gained
  exactly the two new PNGs and no existing baseline moved; `ffi.rs`, `scenes/mod.rs` and both
  manifests untouched (**C ABI stays v4, `Scene` unchanged, no new dependency, no preset change**).
  **The non-vacuity was reproduced independently**: restoring the grid-derived aspect fails all three
  new guards, with the disc at 1280x800 reading 1.0000 skipped against **1.2800** through trails
  (predicted 1.280; `shot` measured 1.278), the 1920x1080 control unmoved under both, and the capture
  guard failing at `composite_trails` mean 0.0958 / outlier 255 and `composite_kaleido` 0.0356 / 135.
  The kaleido fixture's **5.9 % clamped-pixel figure was recomputed from the shader's arithmetic and
  is exact** (0.0590 at 160x100, order 6). **Minors:** (1) the kaleido fixture's claim that "a 0010
  fix must not pass silently" is **false as written** — applying backlog 0010's first candidate
  (`min(r, 0.5)`) leaves the guard green at mean 0.0189 against its 0.02 tolerance while consuming
  94 % of the drift budget, so the next unrelated fold change trips it with a misleading message
  (recorded in backlog 0010 with the number; a real guard needs a border-filling scene or a direct
  clamped-pixel assertion); (2) `render/grid.rs:22-26` and `post.rs:566-575` still justify the
  single-factor cap by *shape* while the same doc states ADR-0037's opposite conclusion — after this
  plan the surviving reason is **sampling density**, not shape; (3) `post.rs:1014-1018`'s aspect
  assertion is now a tautology whose message describes a property it no longer reads; (4)
  `README.md`'s "166 of 180 tests" went stale (the new `composite` binary is not in the pre-push skip
  list; it costs a measured **2.9 s**, so "~28 s" survives) — **fixed in this close commit**. **Nit,
  also fixed here:** backlog 0010's "fixing 0010 and closing major 3 belong in the same plan" — major
  3 closed here, 0010 still open. **Out of scope, correctly:** `trail_grid_size` keeps its `pub`
  (narrowing means moving `core/tests/attractor.rs` into the crate); `presets/swarm_dense.toml`'s
  false "six stays clean at 16:9" is `preset-author` content already in backlog 0010; and `cargo test
  -p lmv-core --lib render::post`'s `STATUS_ACCESS_VIOLATION` under in-process parallel GPU tests
  **predates this plan** (verified against a stashed tree at `a1e3e26`; `nextest` and
  `--test-threads=1` are clean). **⚠ On-device carry-forward:** the composite now rasterizes up to one
  256 px step of texels it does not show on one axis, on top of the full-resolution ping-pong [0033]'s
  item already watches; and Phase 4 put the RD reconstruction's ~45 texture fetches per fragment on
  `docs/on-device-validation.md`, with a failure routed to `architect` rather than an automatic revert
  because reverting costs the coral look. **[ADR-0037] accepted.** Version **patch 0.17.0 -> 0.17.1**
  (a fix/coverage/refactor/docs plan, no feature).

- [0033 — Internal resolution follows the target, plus the preset-surface and harness gaps behind
  it](done/0033-internal-resolution-and-preset-surface.md) — **done 2026-07-26**, passed Mode 4
  review (**no blockers**; three majors, four minors). Six `dev` phase commits (`978405a` `shot`
  reaches `tempo`/`novelty` + band levels, `cf65c4a` `{ attack, release }` smoothing, `8c0ff2b`
  Catmull-Rom reconstruction, `08714c7` the wrapped RD sampler, `3f3b652` target-sized post stages,
  `621fa7b` the operator sweep). **Phase 4 skipped** under ADR-0034's if-and-only-if — the
  reconstruction fix resolved the coral artifact, so the Gray-Scott grid stays 256, no coral preset
  takes a look change, and the ~4x sub-step cost is not spent. Takes four of the eight
  [design-backlog](../design-backlog.md) entries from the 2026-07-26 `preset-author` batch.
  `TRAILS_W/H` and `KALEIDO_W/H` are gone: one `internal_grid_size(surface)` policy (256 px step,
  single-scale-factor cap at **1920x1080** — not the attractor's 2560x1440, because the trails
  `Rgba16Float` field pair is charged twice during a dual-live dissolve) sizes both stages,
  `PostStage::internal_size` takes the surface, and `KALEIDO_ASPECT` moved into the uniform so the
  fold corrects by the live ratio. `lsystem_fern` with `trails = 0.75` at 2048x1152 now renders its
  finest fronds crisp with the feedback active — the combination the fixed 720p grid made impossible.
  RD's present pass reconstructs with **Catmull-Rom** (nine bilinear taps, every field read routed
  through `sample_v` — value, gradient, contour, hatch — because the gradient is where a C0
  discontinuity shows), and its sampler is `Repeat`, so `pan_*` is a seamless scroll over a torus the
  simulation has always been and `zoom > 1` tiles instead of smearing the edge row into bars.
  `[smoothing]` takes an `{ attack, release }` pair, hand-deserialized rather than `serde(untagged)`
  so a mistyped table is not harder to diagnose than a mistyped float. `shot --set` reaches
  `tempo`/`novelty` and every filmstrip now prints the min/mean/max the analyzer measured — a
  full-scale 60 Hz sine reads `bass 0.187` and a 120 BPM click peaks at `bass 0.011`, against the
  `--set bass=0.8` the lane had been calibrating against. **Verified at review** rather than taken
  on trust: `fmt --check` + `clippy --workspace --all-targets -D warnings` clean; `nextest -p
  lmv-core --lib` **97/97**; `golden` + `reaction_diffusion` **3/3**; `sanity` + `reactivity` +
  `animation` **3/3** (the floors that actually render through the changed stages, since
  `rose_trails` binds `trails` and two presets bind `kaleido_*`); `-p standalone` **58/58**; and
  `git diff --stat` over the range confirms `core/tests/golden/reaction_diffusion.png` is the **only**
  baseline touched, so both re-bless scope claims hold in fact (`LMV_BLESS` twice rewrote
  `fragment_field.png` / `swarm.png` on WARP variance and both were restored). **Three deviations
  accepted, each because the plan was wrong and the commit said so:** Phase 2's "90 % within two
  60 Hz frames" is unreachable for its own `attack = 0.02` (`alpha = 0.5654`, so two frames reach
  81.1 % and three reach 91.8 % — the test pins the arithmetic); Phase 6's "reports 2048x1152"
  contradicts the same plan's 1920x1080 cap, which ADR-0034 acknowledges as a 1.07x downscale; and
  Phase 3 shipped Catmull-Rom rather than the plan's named cheap quintic coordinate warp, which was
  built and measured **worse** (zero derivative at both ends pins the reconstruction gradient to zero
  at every texel centre, and `line_d`'s `fwidth` gain renders that as one scalloped step per texel).
  **Phase 3's done-when 3 is unmet and is not satisfiable as specified** — it asks a pixel-domain
  scanline second-difference statistic to detect a geometric property of a 1-D contour curve, and at
  8 bits the slope discontinuity over a smooth field is below one output quantum (five measured
  attempts in `8c0ff2b`). Accepted with cause; **no followup metric is owed**, since the RD golden is
  a real pixel guard for exactly this shader, it moved, and it passes. **Majors, all routed to a
  followup fix plan rather than reworked here:** (1) the 256 px round-up makes the grid's aspect
  differ from the target's, and because `post.rs:445` derives the scene's aspect from the **grid**
  while both stages present with a plain normalized blit, the whole frame is stretched whenever a
  stage is active — reproduced with `shot` on a trails-bound `rose_web`, **1.278x wider at 1280x800**
  (predicted 1.280) and **1.069x at 1280x720** (predicted 1.067, where the old fixed grid was
  aspect-exact); 1920x1080 / 2048x1152 / 2560x1440 / 3840x2160 are unaffected, which is why the
  plan's own display never showed it, and because the attractor reads this same `aspect` it also
  undoes [0029] Phase 5's fix on that path — the correction is one line, take the aspect from the
  surface and leave `Scene::set_target_size` on the grid; (2) `docs/on-device-validation.md`'s new
  trails item tells the tester no shipped preset binds `trails`, but `presets/rose_trails.toml:48`
  does, so the item guarding this plan's stated main risk sends them to build a needless workaround;
  (3) **no fixture in `core/tests/fixtures/` binds `trails` or `kaleido_*`**, so Phase 6's "goldens
  re-blessed" done-when referred to baselines that never existed and the headline change ships with
  zero pixel guard on the stages it rewired — which is how major 1 got through (not a blind fixture
  add: a feedback pipeline built mid-run resolves differently on WARP). **Minors:** fourteen preset
  headers plus `attractor_ink.toml:22` still teach the retired fixed-1280x720 rule (preset content —
  rides with Phase 8); `post.rs::internal_grid_size` is a line-for-line copy of
  `particles/mod.rs::trail_grid_size`, the opposite of ADR-0034's "one shared function" consequence;
  the RD reconstruction's real cost is **unmeasured** (the +16 % WARP figure was retracted in
  `3f3b652` as run-to-run noise — 193.6 / 224.2 / 105.2 s on the same suite — leaving 45 fetches per
  fragment as the only real number, and the on-device checklist gained trails items but no RD item);
  `presets/README.md`'s "an ultrawide keeps its proportions" overclaims under the round-up
  (3440x1440 comes back 1.88 against 2.39). **⚠ On-device carry-forward:** full-resolution trails
  against NFR §1's 60 fps @ 1080p floor, the working-set delta including mid-dissolve, and now the
  RD tap count — none reachable from WARP; the mitigation in all three cases is the one cap constant.
  **Core + standalone only; C ABI untouched (v4); `Scene` trait untouched; no new dependency; no
  preset re-tune** (Phase 8's job). [ADR-0034](../adrs/0034-internal-resolution-follows-the-target.md)
  **accepted with an Outcome section** correcting two claims implementation falsified, and
  [ADR-0035](../adrs/0035-asymmetric-attack-release-easing.md) **accepted**. **Phase 8 (`human`)
  remains open** — the aesthetic re-tune on the 2048x1152 display, restoring `trails` to the line
  presets. Version **minor 0.16.1 -> 0.17.0** at close (a feature plan).

- [0031 — Cleanup pass: testable `shot` helpers, one construction path, load-time param routing, and
  the accumulated close-review debt](done/0031-composite-cleanup-and-debt.md) — **done 2026-07-26**,
  passed Mode 4 review (**no blockers, no majors**; five minors, three nits). Six `dev` phase commits
  (`5244fd2` `shot`'s helpers into the lib, `83706a3` `from_context`, `6755014` load-time routes +
  `tau`, `64e7145` three per-frame stops, `fb024fc` `render/gpu.rs` + the attractor split, `609b9c9`
  the accumulated debt). Clears the non-blocking half of the 2026-07-25 codebase-health review **plus
  the minors four earlier closes logged and nobody returned for**. `standalone/examples/shot.rs` went
  1028 -> 803 lines: its pure helpers (the 16-bit WAV parse, `filmstrip_indices` + the strip layout,
  the arg helpers, the JSON emitter, the glyph table) live in `standalone/src/shot/` where `cargo
  test` actually reaches them — an `examples/` target's `#[test]` does not run — and `image` stays a
  dev-dependency because the *layout* arithmetic moved and the pixel blit did not (ADR-0011,
  ADR-0033 Alt E). `Renderer`'s three ~28-line constructors collapse onto `from_context`, with the
  `unsafe` boundary still wrapping exactly `RenderContext::new_unsafe`. **The per-frame binding loop
  no longer re-derives two load-time facts**: `tau` is folded out of the validated `[smoothing]`
  table onto each `Binding` at parse time (`Preset::smoothing` is gone), and a
  `ParamRoute { Background, Stage(usize), Ink, Scene, Unclaimed }` resolved by a pure
  `resolve_route(name, system)` replaces the chained `set_param(&str, ..)` fallthrough — so adding a
  composite stage costs an enum arm, not another link inside the hot loop. Three per-frame
  operations stop: the cap-overflow `format!` (an `OverflowContext` enum that formats only in
  `Display`, which is also what lets `CapOverflow` become `Copy`), the identity-mirror buffer copy
  (an O(1) `mem::swap` at `MirrorSpec::is_identity`), and the attractor's reseed on grid change.
  `core/src/render/gpu.rs` is one home for the bind-entry helpers, the single parameterized
  `fullscreen_pipeline`, **three** fullscreen-triangle vertex preludes (raw NDC / Y-flipped UV /
  un-flipped UV — the pasted stages genuinely disagreed, and a wrong flip is a vertically-mirrored
  effect) and a unit-tested `FixedStep`; `AttractorScene::render` went 228 -> 77 lines with no GPU
  call reordered. Phase 6 closed eight debt items: the frame's `Routing` is decided once in
  `PostChain::begin` and *consumed* by `resolve`, `GeneratorConfig`/`CapOverflow` moved up to
  `scenes/mod.rs` so **no module under `scenes/lines/` names `particles::`**, `cargo doc -p lmv-core
  --no-deps` is **warning-free**, `RoseParams` replaces eleven positional `f32`s, `Palette` drops
  `Copy` at 6 KB, and a **live** segment-cap overflow finally reports itself — edge-triggered on
  entry and once on recovery, because an unthrottled `eprintln!` at 130 fps is I/O on the render
  thread. **Verified at review** rather than taken on trust: **211/211** workspace tests green;
  **`core/tests/` byte-untouched across the whole range**, so "every golden baseline byte-identical,
  no re-bless" — the plan's central claim and the thing Phase 5 could have silently broken — is true
  in fact; `clippy --workspace --all-targets -D warnings` + `fmt --check` clean; `cargo doc`
  warning-free; Phase 5's grep proof re-run independently (one `fullscreen_pipeline`, zero local
  bind-entry helpers, the six surviving `BindGroupLayoutEntry` literals exactly the disclosed set);
  and **an independent non-vacuity check on Phase 3** — inducing `Stage(_) -> Unclaimed` fails three
  tests including `a_dual_live_dissolve_carries_the_outgoing_trail`, a *pixel-level* end-to-end, so
  the chain route is behaviorally covered and not merely unit-asserted. **`lmv-core` line coverage
  measured at 90.51 %**, up from Plan 0032's 90.13 %, so the `COVERAGE_FLOOR: 88` ratchet is safe
  despite the deletions. **Core + standalone only; C ABI untouched (v4); `Scene` trait untouched; no
  new dependency; no preset-visible change.** **Accepted deviations:** Phase 3's routes live on the
  render-layer `Roster` keyed by preset index rather than on `Binding` — a dissolve composites two
  presets in one frame and both need routes, and indexing by preset makes a side's routes
  structurally undriftable; Phase 3's plan note (unknown param "must keep today's behavior exactly:
  silently ignored… Plan 0019's job") was stale and correctly not followed, since 0019 landed and the
  load-time warning exists; Phase 1 shipped `filmstrip_layout` rather than the plan's
  `tile_filmstrip`, keeping the PNG codec out of `lmv.exe`; two approved scope expansions
  (`render/transition.rs` in Phase 5 and two `render/mod.rs` doc links in Phase 6, both postdating the
  plan's file lists and both required for the done-whens to hold literally). **Minors:** (1)
  `presets/README.md` still described the cap drop as surfaced "at load-time-style" — **fixed in this
  close commit**; (2) `render/mod.rs:1007`'s `cap_overflow()` doc still says the standalone surfaces
  it "at load", now false; (3) `poll_cap_overflow` edge-triggers on *presence*, and the configure-time
  overflow takes precedence, so a depth overflow masks a later mirror overflow; (4) `tile_filmstrip`
  itself stays untested; (5) `shot/json.rs`'s `num()` renders `NaN`/`inf` verbatim, which is invalid
  JSON (pre-existing, moved unchanged). **Nits:** `gpu::texture`/`gpu::sampler` hardcode
  `FRAGMENT` visibility while `gpu::uniform` takes it, which is why the attractor's vertex-visible LUT
  entries stayed inline; `render/mod.rs:56` still re-exports `CapOverflow` through `scenes::lines`;
  the lsystem early returns don't reset `mirror_overflow`. **⚠ On-device carry-forward:** Phase 2's
  `#[cfg(windows)]` HWND constructor is **build-checked only** (the plugin is not compiled here — the
  other two paths were verified live at 1592 frames / 165 fps and by the headless capture tests), and
  Phase 4's "resizing no longer restarts the point cloud" needs a real window. **No new ADR** — the
  plan carries out existing decisions. Version **patch 0.16.0 -> 0.16.1** at close (a fix/cleanup
  plan; its one behavior addition completes ADR-0007's never-a-silent-cut contract rather than adding
  a feature).

- [0032 — Testing strategy: full-chain e2e, `shot` CLI coverage, a core coverage ratchet, and a
  pre-push gate](done/0032-testing-strategy-e2e-coverage-and-pre-push.md) — **done 2026-07-26**,
  passed Mode 4 review (**no blockers, no majors**; four minors, two nits). Four `dev` phase commits
  (`332720f` the e2e chain suite, `108e21a` `shot` as a subprocess, `ee89905` the pre-push gate,
  `a4b7045` the coverage ratchet). Answers the three questions that opened it — coverage threshold,
  e2e tests, happy paths covered — with a number instead of an opinion. **`core/tests/chain.rs` is
  the first test that crosses the seam CLAUDE.md opens with**: synthetic PCM into a real
  `audio::intake` SPSC pair in 20 ms capture-callback-sized bursts, drained through `pop_samples`
  into a fixed scratch with the shell's own `pump_audio` policy, into a real `Analyzer`, into a real
  `Renderer`, to real pixels. Four claims: band routing survives the ring, determinism holds
  **byte-identically** across it, an oversized push is lossy-not-fatal (fills the ring exactly,
  never splits a frame, never blocks), and `intake` rejects 4 kHz / 0 ch / 9 ch with the exact
  `FormatError` and no panic. **`standalone/tests/shot_cli.rs` runs the built binary as a user
  does** — closing the fact that the CLI the `preset-author` lane self-verifies through had **no
  tests of any kind** (`#[test]` does not run in an `examples/` target). `shot` stayed an example
  rather than a `[[bin]]` (ADR-0033 Alternative E — a `[[bin]]` gets no dev-dependencies, so the
  `image` PNG codec would land in the shipped `lmv.exe`), located by walking `current_exe()`'s
  ancestors for the `examples/` sibling. Four GPU-free cases run everywhere; three rendering cases
  skip on an **adapter-error match rather than an OS check**, so an adapterless Windows runner is
  handled too. `.githooks/pre-push` runs `fmt` + `clippy` + a narrowed `nextest` in **~28 s**,
  opt-in per clone (`git config core.hooksPath .githooks`), printing the nine GPU-heavy suites it
  skipped. A `windows-latest` `coverage` job gates `lmv-core` line coverage behind
  **`COVERAGE_FLOOR: 88`** against a measured **90.13 %**, in one `env:` key with the ratchet rule
  in a comment. **Verified at review** rather than taken on trust: 166/166 green in the hook's
  narrowed set in 22 s wall and `nextest list` reporting 180 total (the README's "166 of 180" is
  exact); the coverage gate re-run locally at **exit 0 / 90.13 %**; `clippy --all-targets -D
  warnings` and `fmt --check` clean; **manifests untouched**, so "no new Cargo dependency" is true
  in fact (hand-rolled escape-aware JSON helpers bought it, and they carry their own negative-case
  unit test); the hook is mode **100755** in the index so it executes on a POSIX clone. **The
  band-routing test is non-vacuous, and more strongly than its own doc claims**: neutralizing the
  *treble* bindings drops the difference to 0.0243 and **fails** the 0.05 floor, while neutralizing
  the *bass* bindings still passes — so the treble half is load-bearing and the test cannot be
  satisfied by bass alone. **Core+standalone tests and CI only; no production code, no C ABI change
  (v4), no `Scene` change.** **Phase 3 deviated with the user's approval:** the plan's guessed
  narrowing list (`golden`, `attractor`, `reaction_diffusion`, `background_composite`, `ink`) is
  worth ~8 s of the ~98 s full gate and misses the real bottlenecks (`reactivity` 89 s, `animation`
  73 s, `sanity` 46 s, `distinctness` 41 s); the hook excludes the **measured** nine, and the plan
  text now says so. **Minors, three fixed in the close commit:** (1) the coverage job's summary step
  ran `cargo llvm-cov report` unscoped, which re-reports every object in the profile data —
  including `lmv-ring` — so the table under the "floor: 88 %" heading was not the number the gate
  enforces (confirmed locally; now `-p lmv-core`); (2) `ci.yml`'s own header comment still said
  "build, test, clippy, fmt" and "GPU rendering ... out of CI scope", the exact sentence this plan
  corrected in `docs/nfr.md` §7; (3) `CLAUDE.md`'s layout tree did not know `.githooks/` exists;
  (4) plan-text drift — Phase 1's done-when named `-E 'test(chain)'`, a *name* filter that matches
  three unrelated `render::post` tests and silently skips two of the four chain claims
  (`binary(chain)` is the suite selector) — corrected in the plan. **Nits:** `scratch()` never
  cleans up its `target/shot-cli-tests/<pid>-*` directories; the overflow claim asserts analyzer
  frames rather than *rendered* frames (which is what keeps it GPU-free — a net gain).
  **First-push discoveries:** the `coverage` job and the three GPU `shot_cli` tests have never run
  in CI, so whether `windows-latest` satisfies `shot`'s hardware-adapter request and how far
  instrumentation stretches the job are open (the suite degrades to a printed skip either way).
  **Phase 5 (`human`) is half done** — `core.hooksPath` is set in the user's clone; the refused-push
  half is observable on their next push. **Baseline for the followup coverage plan:**
  `render/overlay_font.rs` 0.00 %, `render/overlay.rs` 30.69 %, `render/context.rs` 34.71 %,
  `ffi.rs` 56.60 %, `diag/mod.rs` 65.75 %. [ADR-0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)
  **accepted**. **Version: no bump, deliberately** — every commit in the range is chore-class
  (`test`/`test`/`build`/`ci`), no production code shipped, so `docs/releasing.md`'s docs/chore-only
  rule applies; stays **0.16.0**.

- [0023 — Cross-preset visual transitions: MilkDrop-style dissolves between presets](done/0023-cross-preset-transitions.md) —
  **done 2026-07-26**, passed Mode 4 review (**no blockers, no majors**; five minors, one nit). Five
  `dev` phase commits (`2a40f83` ink leaves the chain, `4fefce3` the walking-skeleton controller +
  frozen crossfade, `918ae89` the blend library, `5ab441a` adaptive dual-live + the budget governor,
  `9c0d468` every switch path + re-entrancy + docs). A preset switch is no longer an index bump: an
  engine `Transition` drives it as a **dissolve** over ~1 s on the injected `dt` (no wall clock, so a
  captured show reproduces frame-for-frame), through a **deterministic rotation** over four blend
  kinds — crossfade, additive burn, luma-dissolve, wipe — each a variant of one two-input shader that
  **samples both sides** rather than alpha-compositing, which is what lets the additive line/particle
  families blend without colour corruption. Every switch path goes through it: `Space`, the browse
  overlay's pick, the director auto-rotate, and the C ABI `lmv_cycle_scene`. Policy is two code
  constants (`TRANSITION_KIND`, `TRANSITION_DURATION_SECS`); preset-declared `[transition]`,
  beat-quantized dissolves, and operator UI stay deliberate follow-ups.
  **The composite is now `background -> scene -> PostChain (trails -> kaleidoscope) -> [blend] -> ink
  -> present`** ([ADR-0032](../adrs/0032-ink-leaves-the-chain-blend-between-chain-and-ink.md), now
  **accepted**): ink left the chain to become a terminal engine post-pass symmetric with `Background`
  as the pre-pass, so the two-input blend can sit before it without widening ADR-0031's one-input
  `PostStage` trait. `STAGE_COUNT` is 2 and the chain is exactly the **per-preset** look; the blend and
  ink are the **engine-wide** passes the renderer drives. Ink's `ink_*`/`paper_*` crossfade by the same
  `t` — the poles interpolate in RGB inside the shader, feeding **one** remap of the blended frame
  rather than mixing two remapped frames (the non-linearity ADR-0032 rejects). **Adaptive fidelity**
  ([ADR-0024](../adrs/0024-cross-preset-transitions.md), now **accepted**): the outgoing side re-renders
  live through its own `CompositeSide` (background + a second `PostChain`) only when the two presets
  hold independent GPU state — `scenes::shares_resources`, which vetoes both the same `SystemKind` and
  any two of the three line scenes sharing one `LineRenderer` — **and** the smoothed frame time shows
  positive evidence of headroom under `DUAL_LIVE_BUDGET_MS = 18.0`; otherwise the frozen opening
  snapshot carries it. Demotion latches for the rest of the dissolve, so the mode cannot flicker. A
  zero frame-time reading (diagnostics off — every headless capture) is read as neither headroom nor
  overload, which is what keeps a capture free of any clock-dependent choice.
  `Renderer::select_preset_now` is the **instant-cut escape** (a path, not the code constant the plan
  suggested), used by the capture entry points so captures stay a pure function of their inputs
  (NFR §6). Re-entrancy: a switch mid-dissolve snap-finishes the one in flight and starts the new one;
  a hot-reload cancels cleanly to whatever `set_presets` resolves the active index to.
  **Core-only; C ABI untouched** (`lmv_cycle_scene` gains dissolves through its unchanged signature);
  **no new dependency**; `Scene` untouched. Verified cold at review: 137/137 `nextest -p lmv-core`,
  `clippy --workspace --all-targets -D warnings` clean, and **every golden baseline byte-identical with
  no re-bless** — Phase 1's behavior-preserving claim held in fact. The dual-live trail check
  **requests the default adapter and skips on software**: building the blend's pipeline + targets
  mid-run deterministically changes what the trails stage resolves to on WARP (the ADR-0016 posture
  `background_composite` already takes), and it was verified red against an induced restart bug.
  **Minors** (all carried, none blocking; full text in the plan's Close section): (1) `begin_transition`
  captures `from`/`to` **before** the snap-finish, so a second switch arriving inside one frame
  interval desynchronizes `from_index`, the snapshot and the roster, and `cycle_preset` absorbs the
  press; (2) the standalone's post-switch `warn_cap_overflow` reads the **outgoing** preset, so an
  oversized incoming L-system's truncation is announced one switch late (ADR-0007's "never a silent
  cut" slipping — the title's equivalent staleness self-corrects); (3) the stateful-incoming hitch was
  neither pre-warmed nor documented and `render/mod.rs:589` now claims the opposite; (4) `dissolve_mode`
  is untested (both its pure inputs are covered, the composition is not); (5)
  `docs/on-device-validation.md` gained no item for `DUAL_LIVE_BUDGET_MS`, which the code itself calls
  "the number to calibrate on a low-end rig". **Nit:** a resize mid-dissolve rebuilds the blend targets
  and discards the frozen snapshot, so that dissolve fades up from black. **⚠ On-device carry-forward:**
  the heavy attractor <-> reaction-diffusion pair holding 60 fps @ 1080p on a low-end iGPU, and the
  `DUAL_LIVE_BUDGET_MS` calibration — a WARP capture cannot speak to either. Version **minor
  0.15.1 -> 0.16.0** at close.

- [0030 — Composite chain + scene keying: a `PostStage` trait, an instantiable `PostChain`, and
  kind-keyed scenes](done/0030-composite-chain-and-scene-keying.md) — **done 2026-07-25**, passed
  Mode 4 review (**no blockers**; three minors, one nit). Four `dev` phase commits (`023777d` trait +
  pure routing + chain, `55c7109` the two-chain independence proof, `9c02953` kind-keyed scenes,
  `d711760` docs). `draw_frame`'s ~70-line composite branch ladder over `trailing`/`kaleidoing`/
  `inking` is gone: the three post stages sit behind a crate-internal `PostStage` trait in a
  `PostChain` array whose order is a **compile-time constant** (ADR-0018's feedback-then-fold plus
  ADR-0028's ink-last, pinned by `debug_assert`s so reordering the literal trips in a debug build).
  The routing adjacency is a **pure function** `route(&[bool]) -> Routing` with no GPU and no `self`,
  giving the composite its first real coverage — eight tests including the two invariants asserted
  over all eight flag combinations. `Routing` is fixed-size and `Copy`, so deciding a frame's routing
  allocates nothing. **The Plan 0023 unblock is proven, not assumed**: two chains built against one
  device accumulate independently (B comes back black after A's trail, then reproduces A's pixels
  byte-for-byte through the same history). `system_slot`'s magic-index lookup is deleted — scenes are
  keyed by `SystemKind` via an exhaustive `match` factory over a single `SystemKind::ALL` roster typed
  `[SystemKind; VARIANT_COUNT]`, so roster drift is a **compile error**, and `golden.rs`'s duplicate
  `SYSTEMS` list retired onto it. **Every golden baseline is byte-identical with no re-bless** — the
  plan's central claim, verified. 115 core tests green, clippy clean, C ABI untouched (v4), `Scene`
  trait untouched, no new dependency. ADR-0031 **accepted**. Version `0.15.0` → **`0.15.1`** (patch:
  production code ships, no feature, no behavior change). Open minors routed to [0031]: cache the
  `Routing` at `begin` so one routing decision per frame is structural; ten pre-existing `cargo doc`
  intra-doc-link warnings (all outside this plan's file list — `dev` correctly stopped at the scope
  boundary); the stale `schema.rs:137` routing narration. The debug overlay's p99 under the ~4 new
  vtable calls per frame is **unmeasured** — it needs a live window, so it goes on the on-device pass,
  not the close.

- [0019 — Preset expression grammar v2: branching, math functions, tempo, typo warnings](done/0019-preset-grammar-v2.md) —
  **done 2026-07-25**, passed Mode 4 review (**no blockers**; one major, three minors, one nit). Five
  `dev` phase commits (`c4f76fc` math functions + constants, `c33e996` comparisons + `select`,
  `b36a3de` `tempo`/`novelty`, `462422b` warn-but-load unknown params, `66b1abb` the `docs/presets.md`
  rewrite). The preset expression language roughly doubles, on the walls the `preset-author` lane
  actually hit (ADR-0020, now **accepted**): `cos`, `sqrt`, `pow`, floored `mod` (`mod(-0.2, 1.0)` is
  `0.8`, so a cyclic hue never jumps), `smoothstep`, the constants `pi`/`tau`, the six comparison
  operators at a **new lowest-precedence tier** each yielding a clean `1.0`/`0.0`, and
  `select(cond, x, y)` — which evaluates **only the taken branch**, so `select(x >= 0, sqrt(x), 0)` is
  safe where a `lerp` blend of both sides would poison the parameter with `NaN`. No boolean operators
  by design (`min` is and, `max` is or, `1 - c` is not). `VAR_NAMES` grows 7 → 9: `tempo` (the
  already-computed `AnalysisFrame.bpm`, which `render/mod.rs` simply never passed) and experimental
  `novelty` — **no new DSP**, both already pure functions of the input window, so determinism holds.
  Phase 4 kills the silent-typo footgun: each system declares a `PARAMS` const beside its `set_param`
  match, gathered by `SystemKind::param_names()`, and `is_known_param` unions in the four **global**
  compositing vocabularies (`bg_*`, `trails`, `kaleido_*`, `ink_*`/`paper_*`) so a curated preset
  setting a backdrop is not warned at. An unknown name is a **warning, not a rejection** (NFR 10) —
  the preset loads, the binding is kept, and `Preset::warnings` / `LoadReport::warnings` carry it to
  the standalone, which prints it on load and on every hot-reload. **Verified at review** rather than
  taken on trust: `cargo test -p lmv-core` green with the `preset` suite **22/22** and every new
  assertion body read — the math tests pin exact values (`smoothstep(0,1,0.25) == 0.15625`, the
  floored-`mod` wrap) *and* totality on degenerate input (`sqrt(-1)` NaN, `mod(1,0)` NaN, `e0 == e1`
  smoothstep stays bounded); the `select` test proves the untaken `sqrt(-1)` does not reach the
  result; the variable test binds a **distinct value per slot** so a mis-wired slot cannot
  coincidentally pass; `clippy --workspace --all-targets -D warnings` clean; the hot-path panic pragma
  on `expr.rs` intact (no new `unwrap`/indexing — the new `Call` arms use slice patterns and the eval
  falls back to `0.0`). The drift guard ADR-0020 asked for is real and **verified to fail on induced
  drift**: `declared_params_match_set_param` reads each `set_param`'s arms back out of the source, so
  it covers the GPU-backed scenes a headless test cannot instantiate, and a companion assert proves
  **every shipped preset is warning-free** so the check cannot cry wolf on the curated library.
  **Core-only; C ABI untouched (v4); no new dependency.** **Accepted deviation:** the plan's Phase 5
  was written against a 17-preset / 5-system library and 8 variables; `dev` documented the **current**
  35 presets / 7 systems and 9 variables, and restructured so the per-system parameter tables live
  once in `presets/README.md` with `docs/presets.md` as the authoritative *expression* reference —
  strictly better than the letter of the done-when. **Major (fixed in this close commit):** the
  required operator-doc sweep missed `README.md`, which still advertised "Ten curated presets ship
  across two systems" and pointed at `docs/presets.md` for "the two systems and their parameters" —
  now count-free and pointing at both docs. **Minors:** (1) `standalone/examples/shot.rs:267`'s
  `report_errors` prints `report.errors` but not `report.warnings`, so the headless CLI the
  `preset-author` lane self-verifies through is the one surface that still swallows a typo; (2) the
  drift guard hardcodes the four **global** stages' expected name lists inline instead of reading
  their `PARAMS` consts (those are `pub(crate)`, unreachable from an integration test), so a
  const-only edit to `background`/`trails`/`kaleidoscope`/`ink` can drift `is_known_param` away from
  `set_param` without failing — the seven per-system lists go through `param_names()` and are fully
  guarded; (3) [Plan 0031](done/0031-composite-cleanup-and-debt.md)'s Phase 3 note still tells its
  implementer that an unknown param "must keep today's behavior exactly: silently ignored" and that
  "making it a surfaced warning is Plan 0019's job" — true when drafted, stale now. **Nit:** the
  `preset-author` skill's own grammar reference is now a version behind (see the sequence note above);
  user-gated. Version **minor 0.14.0 -> 0.15.0** at close (a feature plan).

- [0015 — Preset-directory override + live iteration](done/0015-preset-dir-override-and-live-iteration.md) —
  **done 2026-07-25**, passed Mode 4 review (**no blockers**; one major, three minors, two nits).
  Three `dev` phase commits (`82d33dc` shared resolver + `LMV_PRESET_DIR`, `9e59211` `shot
  --presets` / `--preset-file`, `45bf613` docs). Editing a **version-controlled** `presets/*.toml`
  is now live in the running app within ~150 ms and in the next headless capture, with no rebuild
  and no relaunch. The per-OS resolver that was hand-copied into `main.rs` and `examples/shot.rs` is
  now one module, `standalone/src/lib.rs` (a new `[lib]` target beside the `[[bin]]`, per ADR-0014):
  `resolve_preset_dir() -> PresetDir::{Override, Default, Unresolved}` tells the caller **where the
  directory came from**, which is what lets the app seed write-if-absent into the per-user default
  and **never** into a user-owned override. `shot`'s precedence is `--preset-file` > `--presets` >
  `LMV_PRESET_DIR` > per-user dir > embedded; the two explicit flags **error** when they come up
  empty (an agent that named a folder wants exit 1, not a silent capture of some other library),
  while levels 3-5 keep degrading downward as the app does (NFR 10). The `[source]` label printed on
  every capture names the winner, so a PNG's provenance is never a guess. `main.rs` also stopped
  keeping its own per-OS copy for `diagnostics.log` / `config.toml` / `soak.log` — they call the
  lib's `preset_data_root` and, deliberately, do **not** move with the override. **Verified at
  review** rather than taken on trust: `cargo test -p standalone --lib` **3/3** with the assertion
  bodies read (they cover override-wins, override-survives-no-data-root, empty-is-unset, the per-OS
  default, and `Unresolved` — a superset of the Phase 1 done-when), and all of Phase 2's done-whens
  re-run live — `--presets presets --preset Aurora` exit 0 `[--presets presets]`, `--preset-file
  presets/fragment_aurora.toml` with **no** `--preset` exit 0, a bad `--presets` and a bad
  `--preset-file` both exit 1, and `--report` labelling `[LMV_PRESET_DIR ./presets]` set versus
  `[on-disk C:\Users\...\Roaming\...]` unset, which is the direct proof that app and `shot` share one
  resolver. Zero `core/` changes in the range; `plugin-foobar/foo_lmv.cpp` is **comment-only** (it
  now says in writing that it is the last independent copy of the path and does not honor the env
  var); **C ABI untouched (v4)**; no new dependency (polling, **no** `notify` — ADR-0014 C).
  [ADR-0014](../adrs/0014-preset-dir-override-for-dev-iteration.md) accepted at this close.
  **Accepted `dev` judgment calls:** the Edition-2024 `unsafe env::set_var` problem is handled by
  factoring the rule into a pure `resolve_preset_dir_from(env, root)` with two env-free tests plus
  **one** test holding both env halves (verified sound — the other tests never read env, and the
  bin's `director`/`overlay`/`capture_mac` tests are a separate process); `--preset` became optional
  for a one-entry roster (the plan's own done-when command needed it); malformed files in a loaded
  directory are now reported on stderr, matching the app. **Major (deferred by user call, rides with
  Plan 0019's close):** `.claude/skills/preset-author/SKILL.md` + `references/render-loop.md` still
  state this plan is unlanded and still teach the manual `%APPDATA%` copy-over dance — the lane this
  feature was built for. Architect cannot apply it (`.claude/skills/**` is denied); user-gated.
  **Minors fixed in this close commit:** `docs/presets.md` still said the poll was ~500 ms in a
  second place, `presets/README.md` still described the copy-into-`%APPDATA%` flow from inside the
  very folder the override targets, and `docs/capturing.md` still named the retired `SCENE_DT`
  (substance was right — headless steps at `scenes::FALLBACK_DT`). **Nits:** the lib's env-test
  SAFETY comment claims no test in the crate reads the environment, three lines above its own
  `preset_data_root()` read (reasoning holds, the sentence overstates); the plan and this index both
  labelled the untouched C ABI "v3" when it has been v4 since Plan 0014 — corrected. **Observation,
  unmeasured:** `poll_presets()` runs in the redraw path (`main.rs:311`) and does a synchronous
  `read_dir` + one `metadata()` per `.toml`, now 6.7x/s instead of 2x/s. Sub-millisecond on a warm
  local dir and pre-accepted by the plan; the uncovered case is a *slow* override folder (network
  share, sync-backed) eating a 16.6 ms frame. If it ever shows in the soak log the fix is an
  off-thread signature scan, **not** a looser interval — that would undo the feature. **Not covered
  by tests, by design:** "the aurora recolors on screen within ~150 ms" is the plan's stated
  on-device visual check. Version **minor 0.13.1 -> 0.14.0** at close (a feature plan).
- [0029 — Attractor resize cost + ink-stage followups](done/0029-attractor-resize-cost-and-ink-followups.md) —
  **done 2026-07-25**, passed Mode 4 review (**no blockers, no majors**; two minors, two nits). Five
  `dev` phase commits (`773d437` resource split, `9b927ea` quantize + aspect-preserving cap,
  `59aa298` ink golden, `dd74d41` rename + doc corrections, `e375c2e` project at the target aspect).
  Closes the two majors from the Plan 0027 review. `Resources` is split along the axis that actually
  varies: `PipelineResources` (four shader modules, every pipeline, the 50k-particle buffer, both LUT
  textures, the uniforms, the layouts and sampler) is built **once and survives every size change**,
  and only `FieldResources` (the `PingPongField` plus the four bind groups referencing its views) is
  rebuilt — so a live window drag costs a texture pair plus four bind groups instead of four WGSL
  compilations per frame. `trail_grid_size` became the whole size policy in one pure function:
  each axis rounds up to a **256 px step** (so most resize deltas cost two integer compares) and the
  cap is a **single scale factor** applied to both axes (the old per-axis clamp squashed a 3440x1440
  ultrawide target to 16:9, which the aspect-ignoring present stretched back — the shape changed
  discontinuously as the window crossed 2560 wide). `core/tests/ink.rs` is the **first behavioral test
  of the ink stage** (ADR-0028 had filed it optional): it asserts `ink_amount = 0` is byte-identical
  to the unbound fixture, then bands the ink-off frame by luminance and requires the band means to
  come back **strictly decreasing with the ends crossing** — the ADR-0028 property itself rather than
  a tuned constant. `Scene::resize` is renamed **`set_target_size`** with a doc that states what it
  actually carries (the trails/kaleidoscope internal grid when those stages are active, not the
  surface) and restates the ADR-0030 compare-first obligation; `presets/README.md` replaces the
  `"0.5"` recommendation with the honest note that a partial amount blends toward the near-black
  source and greys the paper (now agreeing with the warning in `presets/attractor_ink.toml`).
  **Phase 5 was added mid-plan by the Mode 4 review of Phases 1-4:** quantization broke the
  grid-equals-target identity the draw uniform's `aspect` silently relied on, so the attractor drew
  11% too wide at the default 1920x1080 window and 33% too wide at 512x384; the fix projects at the
  `aspect` argument `render` already received and discarded. **Verified at review:** `cargo nextest
  run -p lmv-core` **95/95 green**; `clippy -p lmv-core --all-targets -D warnings` + `cargo fmt
  --check` clean; both new tests read and confirmed non-vacuous, and the Phase 5 assertion confirmed
  to **fail before the fix** by temporarily restoring the grid-ratio projection (reported exactly the
  predicted 1.329 skew, 1.00 after) — it is the suite's first non-square capture, which is why every
  earlier square capture was blind to the defect. `core/tests/golden/attractor.png` is the **only**
  baseline that changed (the 128x128 capture now takes a 256x256 field), correctly scoped and noted
  in its commit. **Core-only; C ABI untouched (v4); no new dependency; no wall clock** (the debounce
  alternative was rejected for exactly that reason), so fixed-size captures stay byte-reproducible.
  The split is enforced by the types — `FieldResources::build` takes `&PipelineResources` and can
  reach only the layouts, sampler and decay uniform, so a pipeline cannot drift back into the rebuilt
  block. ADR-0028 and ADR-0030 were already **accepted** at Plan 0027's close. **Minors:** (1) the
  particle **re-seed** on a grid change (`particles/mod.rs:1403`) is no longer necessary — the buffer
  now survives the split — and is the surviving half of the "fullscreen toggle pops the cloud back to
  its seed scatter" symptom the plan opened with; determinism does not need it (a headless capture
  holds one target size, and a mid-capture stage flip stays a pure function of the frame index), so
  moving `needs_upload = true` into the first-build arm finishes the job; (2) the 256 px quantization
  **floors** at one full step, so every headless capture supersamples (a 128x128 target takes a
  256x256 field — that is why the golden changed) and no test exercises the grid-equals-target path
  any more. **Nits:** (3) `trail_grid_size` was made `pub`, widening `lmv-core`'s public API for a
  test's benefit (consistent with `AttractorFamily` already being public there, so not new drift);
  (4) this row's predecessor carried a stray fifth table column, fixed by this close. **Manual check
  (non-blocking):** the plan's user-visible payoff — a smooth live window drag that keeps the cloud,
  and a stall-free fullscreen toggle — needs a real window to confirm; the 256 px step is one
  constant if it reads wrong on device. Version **patch 0.13.0 -> 0.13.1** at close (a fix-only plan).

- [0027 — Attractor ink-on-paper (engine-wide final tone-remap) + crisp trails](done/0027-attractor-ink-and-crisp-trails.md) —
  **done 2026-07-25**, passed Mode 4 review (no blockers; two majors and four minors, all routed to
  [Plan 0029](done/0029-attractor-resize-cost-and-ink-followups.md) or [ADR-0030](../adrs/0030-scene-target-size-hot-path-hook.md)
  rather than reworked). Three `dev` phase commits (`0e3b84a` ink stage, `5f79dc6` surface-sized trail
  field, `5daddfa` curated preset + docs). Delivers the "ink on paper" look the `preset-author` lane
  could not reach: `render/ink.rs` is a final, skippable composite stage that reads each pixel's
  luminance as an *ink density* and repaints the finished frame to `mix(paper, ink, d)` — the
  **darkening** step the additive scene pipelines structurally cannot express. Default poles are
  white/black, so `ink_amount = 1` alone is a pure black-on-white invert, and `paper_*`/`ink_*` give an
  arbitrary duotone; it works on **every** scene because it operates on the composited frame, and it
  sits before the text/overlay passes so the HUD is never inverted. Phase 2 replaced the attractor's
  fixed 640x360 accumulation grid with one sized to its render target (capped 2560x1440), killing the
  soft upscale at 1080p+. **Verified at review:** `cargo nextest run -p lmv-core` 90/90 green
  (including `golden`, `sanity`, `animation`, `reactivity` over the new `attractor_ink` preset);
  `clippy -p lmv-core --all-targets -D warnings` clean; the Phase 1 passthrough claim confirmed —
  `core/tests/golden/attractor.png` is the only baseline the plan touched, so the `fragment_field`/
  `swarm` restore after an over-broad `LMV_BLESS=1` run was correct. **C ABI untouched**, no new
  dependency. The plan's "no `Scene`-trait change" line did **not** hold: Phase 2's ask for a
  resolution derived from the target size had no channel to travel on (`Box<dyn Scene>`,
  `Scene::render` receives only `aspect`), so a default-no-op per-frame target-size hook was added
  with the user's approval — the third widening and the first on the hot path, now recorded in
  ADR-0030, which replaces ADR-0007/0021's "this is the last one" countdown with three conditions any
  future widening must meet. The two majors: that undocumented widening, and a full GPU-resource
  rebuild (shaders and pipelines included) on every size change inside `render`. ADR-0028 accepted at
  close; version bumped 0.12.0 -> 0.13.0.

- [0025 — Full composite coverage: background + view transform for reaction-diffusion and attractor](done/0025-full-composite-coverage.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Five `dev`
  phase commits (`06b4007` RD alpha-present, `ae17d57` RD zoom/pan, `265045b` attractor alpha-present,
  `566fcf8` attractor zoom/pan, `6c570ec` docs) plus the pre-cleared `8d0e17a`
  (`Renderer::adapter_is_software()`). Finishes ADR-0018's engine-wide promise: both fullscreen/
  accumulating scenes switched their final present from opaque `REPLACE` to
  `PREMULTIPLIED_ALPHA_BLENDING` over the `bg_*` backdrop (RD alpha = the V-field `structure` term;
  attractor alpha = accumulated luminance), so coral voids / attractor negative space reveal the
  tintable gradient — and both now accept `zoom`/`pan_*` via `set_param` (RD in its present-pass sample
  UVs, the attractor in its world projection), exactly as `fragment_field` does. **Verified:** the
  `reaction_diffusion.png` / `attractor.png` golden fixtures are **byte-identical** (neither binds
  `bg_*`, so premultiplied-over-black equals the old opaque present) — the "re-bless" the plan
  anticipated was a confirmed no-op, pinned by the unchanged golden suite. View transform isolated +
  determinism-checked in the two scene tests (same field/seed, `zoom`/`pan` → `frame_diff > 0.02`,
  reproducible captures); the backdrop reveal asserted on real hardware (dual-hue differential,
  void-masked tint tracking) and skipped on the WARP software adapter in the new
  `core/tests/background_composite.rs` (the one added test file + the `adapter_is_software` accessor,
  both pre-cleared as outside the plan's file list). **No `Scene`-trait change; C ABI untouched;** no new
  dependency; hot-path-safe. `fragment_field` correctly stays fullscreen-opaque (bg_* no-op there, now
  documented); `mirror_*` stays line-only by design. **Minor:** stale "fullscreen opaque pass" comment
  on the RD present (`reaction_diffusion.rs:1048`) — swept into Plan 0027's tidy-up list. **Nit:** the
  plan's "goldens re-blessed" done-when wording diverges from the byte-identical reality (a
  strictly-better outcome). Unblocks Plan 0027 (hard-sequenced after it). [ADR-0026](../adrs/0026-full-composite-coverage-fullscreen-scenes.md)
  now **accepted**. Version **minor 0.11.0 -> 0.12.0** at close.

- [0028 — Parametric-curve shape params: radial offset + phase (audio-morphable rose geometry)](done/0028-parametric-curve-shape-params.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Two `dev`
  phase commits (`f37dde0` Phase 1 — core sampler + scene + tests; `20cd7f7` Phase 2 — docs). Added
  two named zero-defaulted per-frame **shape** params to `parametric_curve` (ADR-0029, now
  **accepted**, supplements ADR-0007): `phase` (radians inside the sine) and `radial_offset` (added
  to the radius), so the Maurer sampler becomes `r = sin(n*theta + phase) + radial_offset`. Threaded
  through the existing `reset_params`/`set_param`/`DEFAULT_*` machinery in `parametric.rs` into an
  extended `maurer_rose(...)` in `curves.rs` (new args grouped by role: `phase` by the frequency
  inputs, `radial_offset` by `scale`). Unlocks the reference's spiral/rosette/annular family and
  phase-morph as **audio-bindable** levers (bind to `bass`/`bar`/`beat`, and `tempo` once 0019 lands),
  not just color. Both **default 0.0** — a no-op reducing to the plain `sin(n*theta)` rose — so every
  shipped rose preset and the `parametric_curve` golden fixture stay **byte-identical** (no re-bless,
  the property a dedicated test pins). Verified: the 6 `curves` unit tests green — incl.
  `zero_phase_and_offset_reduce_to_the_plain_sine_rose` (the no-op pin), `radial_offset_shifts_the_
  radius_by_a_constant`, `phase_changes_the_geometry`, plus a `capture_preset` binding test in
  `render/mod.rs` (`shape_params_reach_the_parametric_scene`) proving both evaluated values thread
  into rendered geometry under a bass stimulus; `golden` **unchanged** (no re-bless, confirming the
  no-op); `hygiene` confirms the `curves.rs`/`parametric.rs` panic pragmas intact; `clippy -p lmv-core
  --all-targets -D warnings` clean. **Core-only; C ABI untouched; `Scene` trait untouched; no new
  dependency;** hot-path-safe (two pure `f32` adds, no indexing/division/allocation). **Minor:** Phase
  1 also edited `core/src/render/mod.rs` (+52) for the required Done-when-#3 binding test — expected
  (the capture harness lives there), just absent from the phase's "Files touched" list. **Nit:** Phase
  2 documented both `presets/README.md` (the live table) **and** `docs/presets.md` — the plan targeted
  only the latter, which was stale (no `parametric_curve` section); per the user both were updated now
  rather than deferring to Plan 0019's rewrite (which carries them forward). **`preset-author`
  followup (non-blocking):** revise the `rose_maurer_sweep`/`rose_overflow`/`rose_beat_bloom` drafts
  (untracked in the working tree) to use the new params and flag the strongest as a `dev` embed
  candidate. Pre-existing unrelated `every_preset_animates_over_time` Aurora failure (fragment_field,
  motion 0.0078) is not part of this plan. Version **minor 0.10.0 -> 0.11.0** at close.

- [0020 — Shared palette system: gradient LUT, named + custom palettes, bindable color (all four scenes)](done/0020-shared-palette-system.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; two minor, two nits). Six `dev`
  phase commits (`e64908c` shared `core/src/render/palette.rs` + fragment through a 256-entry baked LUT,
  `b518130` custom gradient `stops`, `81ede9e` swarm through the LUT, `53c944e` A/B `palette_mix`
  crossfade, `9281c23` reaction-diffusion + attractor through the palette, `d00ce16` palette-surface
  docs). Landed the shared color axis (ADR-0021, now **accepted**, supplements ADR-0002): a preset
  declares an optional `[palette]` (built-in `name` — `spectrum`/`ember`/`ice`/`mono`/`aurora` — **or**
  custom `stops`, validated at the load boundary) baked once into a 256-entry RGB LUT that **all four**
  shader-colored scenes color through — a 256x1 1D texture for fragment/RD/attractor (attractor samples
  it in the **vertex** shader), the identical CPU array for the swarm. Color is fully bindable via
  layer-2 named params (`saturation`, `hue`, fragment/RD `color_span`/`color_center`, swarm/attractor
  `hue_spread`/`hue_center`) plus an A/B `palette_mix` crossfade against an optional `[palette_b]`.
  Delivered through one thin off-hot-path `Scene::set_palette` hook (the **second** trait widening after
  ADR-0007's `configure`; ADR-0021 flags a third should prompt re-examining the seam). Default
  `spectrum` = the exact prior cosine, so shipped presets are **visually unchanged**. **Scope expanded
  2026-07-23** to fold reaction-diffusion + attractor into Phase 5 (was a followup) on `preset-author`
  coral-trio evidence; docs became Phase 6. **User decision (Phase 5):** reaction-diffusion's present
  used a *different* cosine than fragment/swarm; it was unified onto `spectrum` (its golden re-blessed),
  so RD is the one scene whose default look intentionally shifted — re-authoring the four coral presets
  is the documented `preset-author` followup. Verified: **51 lib tests + `preset` integration green** —
  incl. `spectrum_reproduces_the_prior_cosine` (the no-regression proof: default LUT within 0.01 of the
  analytic cosine at 8 sampled `t`), the Phase 2 malformed-stops/selector-clash load-error suite (never
  a panic; loader keeps the prior preset), `narrow_spread_makes_colour_coherent` (variance-measured
  swarm distinctness), and `palette_mix_crossfades_a_to_b`; `clippy -p lmv-core --all-targets` clean
  with the hot-path panic pragma on `palette.rs`. Only `core/tests/golden/reaction_diffusion.png`
  changed — fragment/swarm/attractor goldens byte-identical, the drift-guard no-regression proof. A DX12
  WARP layout-aliasing fix kept each scene's LUT bind group structurally unique (fragment's LUT moved to
  `group(1)`; RD/attractor LUTs in unique-arity present groups). Palette docs landed as a sibling
  `docs/preset-palettes.md` (Plan 0019 owns the `presets.md` rewrite) + a `presets/README.md` colour
  section, with a worked custom-stops example. **Core-only; C ABI untouched; no new dependency;**
  determinism preserved (bake pure/off-hot-path, `sample` alloc-free). **Minor:** (1) a `[palette]`/
  `[palette_b]` declared on a non-colored line scene (lsystem/star/parametric) bakes and is silently
  ignored by the default `set_palette` no-op — no author feedback that colour config is inert there
  (consistent with `set_param`'s silent unknown-name drop, but a `preset-author` footgun); (2)
  `docs/presets.md` still describes `hue` as an "offset into the looping cosine palette" — now a LUT
  sample coordinate — but that file's rewrite is explicitly owned by Plan 0019, so it was deferred, not
  updated here. **Nits:** (3) the `Scene` trait is now two optional methods past its ADR-0002 shape
  (`configure` + `set_palette`) — a third widening should trigger the seam re-examination ADR-0021 calls
  for; (4) Phase 4's crossfade was demonstrated with a time ramp rather than the plan's `bar` driver
  (the synthetic click track barely sweeps `bar`), a reasonable substitution since the crossfade math is
  unit-tested. **`preset-author` followups (non-blocking):** re-author the field/flock + coral presets
  to exploit named/custom palettes and `hue_spread`/`color_span`; refresh the skill's `systems.md`/
  `grammar.md` colour snapshots; consider OKLab interpolation in the bake (ADR-0021 Alt E). Version
  **minor 0.9.0 -> 0.10.0** at close.

- [0026 — Calmer scene rotation: hold one scene by default, longer dwell, softened drop bias](done/0026-calmer-scene-rotation.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Three `dev`
  phase commits (`f3dab1c` hold-one-scene default, `49600a2` longer dwell + softened drop gate,
  `f4fd2c7` operator docs). Reverses Plan 0009's "lively unattended show" default (ADR-0027, now
  **accepted**): `Rotate::default().auto` flips **`true` -> `false`** so a fresh install (no
  `config.toml`) holds one scene until the operator opts in via the `A` hotkey (`toggle_auto`) or
  `auto = true`; manual `Space` next-scene works either way. When auto **is** on the cadence is calm:
  default dwell **8/40 -> 20/90 s**, and the energy-drop bias is **softened, not removed** — gated
  behind `DROP_GATE_FRACTION` (0.25) of the min->max span past the min dwell (`min + 0.25*(max-min)`
  ~37.5 s at the default), so a drop just after a rotation can't rapid-fire another; the max-dwell
  timer and the novelty nudge are unchanged. The gate is **proportional to the span** (the
  fraction-of-span option the plan's Phase 2 explicitly sanctions over a fixed `DROP_GATE_SECS`), so
  it scales sensibly for custom dwell configs. Every `Rotate` field is `#[serde(default)]`, so a
  pinned `config.toml` keeps its behaviour — only fresh installs get the revised defaults. Docs
  (config.rs module doc + README Controls) state hold-by-default, the opt-in path, and the new
  cadence. Verified: **13/13 `director` tests green** — incl. `default_config_holds_one_scene_but_
  manual_overrides_work` (default config never auto-rotates over 200 s + a drop, yet `force_next`
  returns `Manual` and `toggle_auto` enables it), `steady_passage_rotates_at_max_dwell` (holds to the
  90 s cap), `energy_drop_rotates_earlier_than_the_cap` (a drop past the ~37.5 s gate fires
  `AutoDrop`), and the new `drop_between_min_dwell_and_gate_is_held` (a drop past min 20 but before the
  gate is **held**, asserting the softened gate prevents rapid re-fire). **Standalone-only**
  (`director.rs` + `config.rs` + `README.md`); **core, DSP, and C ABI untouched**; determinism
  preserved (injected-`dt` EMA, no wall clock, no unseeded randomness). **Minor:** the novelty nudge
  (`AutoBoundary`) is still bounded only by the min dwell (can rotate at ~20 s on a strong boundary),
  not by the ~37.5 s drop gate — intended per the plan ("novelty rides the same dwell"), but worth an
  on-device look since a lively `novelty` could feel quicker than the calmed drop path; `track_change`
  is on-by-default and experimental. **Nit:** two pre-existing tests (`auto_off_never_auto_rotates_
  but_manual_still_works`, `toggle_auto_flips_and_reports_state`) still construct explicit `8, 40`
  configs — correct (they exercise behaviour, not defaults), just no longer echoing the shipped
  numbers. **On-device followup (non-blocking):** tune the 20/90 dwell + drop gate during the next
  live soak; optional scene-lock/on-screen rotation-state indicator. Version **minor 0.8.0 -> 0.9.0**
  at close.

- [0018 — Engine-wide visual enrichment: zoom, atmosphere, easing, mirrors](done/0018-engine-wide-visual-enrichment.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; three minor, two nits). Eight
  `dev` phase commits (`bade3eb` shared `ViewTransform` + zoom/pan on line scenes, `0faa087`
  ViewTransform to fragment+swarm, `02b16e6` engine background pre-pass + scenes `Clear`->`Load`,
  `536b8c9` geometry mirror for line scenes, `822cc94` eased params via render-layer one-pole,
  `e67f217` feedback trails, `56d0460` screen-space kaleidoscope, `52673e0` curated presets + doc).
  Landed the **fixed-order engine composite** ([ADR-0018](../adrs/0018-engine-wide-scene-compositing.md),
  now **accepted**) — background pre-pass (owns the clear) -> active scene under a shared
  `ViewTransform` (zoom/pan, applied per family: line vertex shader, fragment sample coords, swarm
  particle positions) -> feedback trails (`PingPongField` max-decay) -> screen-space kaleidoscope
  (dihedral pixel-fold) -> present — every stage individually skippable to a **passthrough** (which
  also sidesteps the DX12-WARP multi-pipeline aliasing, like RD/attractor). Every scene switched
  `Clear`->`Load` so the background owns the frame. Plus the **render-layer easing seam**
  ([ADR-0019](../adrs/0019-eased-parameters.md), now **accepted**): an optional per-`(preset,param)`
  one-pole (`alpha = 1 - exp(-dt/tau)`) on Plan 0014's injected `dt`, between `eval` and `set_param`,
  reset on preset switch **and** capture rebuild — the expression layer stays pure/zero-alloc; a
  `[smoothing]` table (`param = seconds`, `tau = 0` default = today's instant behaviour) is validated
  non-negative/finite at load. The **geometry mirror** is line-only (segment replication under
  N-fold rotation + optional reflection *before* the cap, surfacing a per-frame `CapOverflow` through
  a new defaulted `Scene::mirror_overflow()` query); the **kaleidoscope** is the general pixel-fold —
  both ship, per the product decision. Six curated presets embedded (`rose_zoom`, `rose_atmosphere`,
  `rose_kaleidoscope`, `rose_trails`, `fragment_kaleido`, `fragment_smooth`) + a `presets/README.md`
  authoring note for every new param. **Core-only; C ABI untouched; no new dependency.** Verified on
  WARP: mirror `2*pi/n`-rotation invariance + exact cap-drop reporting, one-pole step/converge/reset,
  smoothed + trailed `capture_preset` **byte-identical recaptures** (NFR §6 determinism holds through
  the stateful stages), the 4 sparse-additive golden baselines re-blessed (lost their incidental clear
  tint, now owned by the default-black backdrop; fragment/RD/attractor byte-identical), embedded-parse
  + `every_preset_animates` green, hygiene pragma covers the three new `render/` files. **Minor:**
  (1) the mirror-overflow branch `format!`s a `String` per frame while an audio-driven `mirror_order`
  sits over the cap (normal fitting path allocates nothing — fix: store `order: u32`, format in
  `Display`); (2) a **live** mirror overflow never surfaces at the standalone (`warn_cap_overflow`
  reads only at load; the per-frame drop is exposed via `cap_overflow()` but nothing polls it live);
  (3) **zoom is semantically inverted** between the fragment field (`zoom>1` = out) and line/swarm
  (`zoom>1` = in) — deliberate, to honor the hard no-regression done-when (inverting the field would
  regress all 5 fragment presets + the golden fixture), documented in `presets/README.md`. **Nits:**
  (4) ADR-0018's "scenes gain no new *required* method" wording undersold the added (defaulted,
  ISP-clean) `mirror_overflow()` query; (5) `draw_calls` counts the passthrough background clear as a
  draw (diagnostic estimate only). **⚠ On-device carry-forward** (non-blocking, standing posture): the
  iGPU 60 fps @ 1080p floor under the passthrough/offscreen cost + active trails/kaleidoscope (NFR §1)
  — belongs on `docs/on-device-validation.md`. Trails/kaleidoscope run at a fixed 16:9 internal
  resolution presented stretched (same documented v1 limitation as the RD/attractor presents). Unblocks
  **Plan 0023** (cross-preset transitions append a blend stage to this composite). Version **minor
  0.7.1 -> 0.8.0** at close.

- [0024 — Single-source the foobar component version + refresh stale plugin descriptions](done/0024-foobar-component-version-single-source.md) —
  **done 2026-07-23**, passed Mode 4 review cold (**no blockers, no majors, no minors** — one nit).
  Two `dev` commits: `08df308` (Phase 1: `build.ps1` reads `[workspace.package].version` from root
  `Cargo.toml` and generates `build/foo_lmv_version.h`; `foo_lmv.cpp` includes it guarded with a
  `0.0.0-dev` fallback and feeds `FOO_LMV_VERSION` to `DECLARE_COMPONENT_VERSION`) and `a8effb9`
  (Phase 2: refreshed both stale scene-description strings). foobar's Components list stops showing a
  frozen `0.1.0` — the component version now **tracks the workspace version** through a build-time
  generated header (ADR-0025, now **accepted**). Verified: the `Cargo.toml` regex is anchored to
  `[workspace.package]` (a member/profile `version` can never match) and `throw`s on a miss;
  `/I "$build"` is on the `cl` line; the header is gitignored and untracked; **no `0.1.0` literal
  remains** and neither description mentions spectrum/pulse/starfield (both name the current families
  — fragment fields, particle swarm, line geometry, reaction-diffusion, attractors). **C ABI axis
  (`LMV_ABI_VERSION`) untouched** — no core/ffi/header change; plugin + build-script + docs only, no
  new tracked file. On-device confirmation that the Components list shows the current version is a
  user check (the plugin can't run headlessly). Version **patch 0.7.0 → 0.7.1** at close — which is
  now exactly the number the plugin will display, by design.

- [0021 — Decouple preset content from code: build-time embedding + single-source system names](done/0021-decouple-preset-content-from-code.md) —
  **done 2026-07-23**, passed Mode 4 review (**no blockers, no majors, no minors, no nits** — a clean
  landing). Three commits: `e1e4f1f` (Phase 1: `core/build.rs` generates `EMBEDDED` from
  `presets/*.toml`), `11798c3` (rustfmt of `build.rs`), `0241b7d` (Phase 2: single-source `SystemKind`
  name↔kind mapping). Shipping a preset stops being a code change: the project's **first build script**
  (zero-dependency std `read_dir` + sort + string emit) globs `presets/*.toml` at build time and emits
  `pub static EMBEDDED: &[(&str, &str)]` as filename-sorted `(name, include_str!(<abs path>))` tuples —
  rustc still embeds the bytes exactly as the old hand-written array did. Drop a `.toml` in `presets/`,
  rebuild, and it ships with **no Rust edit and no count to bump**. The path resolves from
  `CARGO_MANIFEST_DIR` (not CWD, so CI/rust-analyzer agree); `include_str!` paths are absolute and
  `{:?}`-escaped (Windows-safe); `rerun-if-changed` is registered for the directory **and** each file,
  covering add/remove **and** edit. The count assert is now **structural** (`core/tests/preset.rs`:
  every embedded preset parses, `default_presets().len() == EMBEDDED.len()`, `>= 8` floor) — never an
  exact number. Phase 2 is a behavior-preserving DRY refactor: `SystemKind::from_name` made `pub` and a
  new `as_str` added in `schema.rs` (the one place the mapping lives), so `shot.rs` deletes its two
  duplicate seven-arm matches and keeps only its friendly error text. Verified: the generated set
  reproduces the prior embedded set **exactly** — 22 filename-sorted entries, byte-identical file set to
  the old array (diff clean; `presets/README.md` correctly excluded by the `toml` filter);
  `cargo test -p lmv-core --test preset` 10/10 green (incl. `embedded_default_presets_all_parse`);
  `clippy -p lmv-core -p standalone --all-targets -D warnings` clean. Per ADR-0022 (now **accepted**):
  **no new dependency, C ABI frozen at v4** (no ffi/abi/header file touched; only `build.rs`, `mod.rs`,
  `schema.rs`, `preset.rs`, `shot.rs`). Simplifies the `preset-author → dev` curation handoff (embedding
  becomes "commit the `.toml`"). **Two documented followups** (neither blocks close): (1) update the
  `preset-author` skill's curation handoff (`references/api-feedback.md`) + ADR-0017's note so they stop
  instructing an `EMBEDDED` array + count-bump edit — **user-gated** (`.claude/skills/**` edits are
  blocked for the assistant); (2) when `docs/presets.md` is rewritten (**owned by Plan 0019**), describe
  the generated embedding instead of the hand-maintained array. Version **minor 0.6.0 → 0.7.0** at close.

- [0016 — GPU compute-particle scenes: strange attractors](done/0016-gpu-compute-particle-scenes.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; two minor, three nits). Five
  `dev` phase commits (`79b6cf0` skeleton, `937fdfb` trails, `9acc415` audio params, `7ec850a`
  family set, `aa34d25` coverage guard + contract). Landed the engine's **first GPU compute
  pipeline** (ADR-0015 idiom B, now **accepted**): a 50k-particle `wgpu` storage buffer stepped
  through a strange-attractor map by a compute shader each frame (`STORAGE|VERTEX|COPY_DST`, read
  back as an instance vertex buffer — no CPU round-trip) and drawn as additive point-sprites with
  fading trails via Plan 0014's `render::feedback::PingPongField` (**no second feedback
  mechanism**). Four families — De Jong + Clifford (2D discrete maps), Thomas + Lorenz (3D
  continuous flows, Euler-integrated + orthographic-projected) — **selected data-driven** via an
  optional `[particles]` table through the existing ADR-0007 `configure` hook: the shared
  `GeneratorConfig` gained a `Particles` variant, so **no new `Scene` trait method**. All knobs
  (`a,b,c,d,size,hue,fade,reseed`) are ADR-0002 layer-2 named params defaulting to the active
  family's coefficients; init is `SeededRng`-seeded and the compute step reads no clock
  (frame-rate independence via the fixed-timestep accumulator, NFR §6). Four curated presets
  embedded (18→22). Phase 5 also **closed the Plan 0022 half-enforced-coverage followup**: a
  `SYSTEMS`-rosters-every-variant guard in `golden.rs` (`VARIANT_COUNT` + an exhaustive
  compile-time reminder), verified to fail when a system is dropped from the drift roster. **Core
  only; C ABI frozen at v4; no new dependency.** Verified: `nextest 68/68` — incl. a dedicated
  `attractor_contract` (byte-identical **seed reproducibility**, **beat perturbation**, animation,
  De Jong + 3D-Lorenz coverage/spread), the attractor golden baseline byte-identical on WARP, and
  preset count 22; `clippy -p lmv-core -p standalone --all-targets -D warnings` clean; `fmt` clean;
  all four families eyeballed distinct via `shot`. **Minor:** (1) the trail field is a fixed 16:9
  offscreen presented stretched (aspect ignored, like the reaction-diffusion present) — correct on
  a 16:9 display, distorted otherwise; (2) the shared `GeneratorConfig` still lives in
  `lines/mod.rs`, so `lines` now references `particles::AttractorFamily` (a future tidy could
  relocate the enum to `scenes/mod.rs`). **Nits:** the coverage guard is compile-*nudged* not
  airtight (`VARIANT_COUNT` is a literal; full rigor needs an enum-iteration dep, out of scope);
  the first cycle to an attractor preset hitches once (lazy GPU-resource build, same as Plan
  0014's cycle-to-Coral); the present pass keeps a redundant clear under a full-screen triangle.
  **⚠ On-device carry-forwards** (non-blocking, like prior plans): 60 fps @ 1080p + low-end iGPU
  compute/additive-fill smoke → `docs/on-device-validation.md` (ADR-0015 Risks); the four family
  presets run hot at high energy — `preset-author`-lane tuning, not engine work. Delivers idiom B
  of ADR-0015's four-idiom catalogue; curl-noise flow fields, fractal flames, and boids remain
  follow-ups on the same compute path. Version **minor 0.5.0 → 0.6.0** at close.

- [0022 — Decouple the golden drift guard from shipped presets (per-system frozen fixtures)](done/0022-golden-fixtures-decouple-content.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Two `dev`
  phase commits (`def9b24` per-system fixtures + repointed golden; `19e7123` engine-vs-content doc
  split). Golden (`core/tests/golden.rs`) previously pinned baselines to three **shipped, curated
  presets** (`Aurora`/`Warp Drive`/`Drift`), so every intended content tune tripped the engine-drift
  alarm and reds CI (concretely `76a2fb4`). Repointed it at six **test-only frozen fixtures** under
  `core/tests/fixtures/` — one per `SystemKind`, keyed by an **exhaustive `match`** with no wildcard
  arm so a new scene fails to compile until its fixture exists — loaded via `set_presets` and
  captured by name; baselines blessed on WARP. Closes the prior **zero** golden coverage of the three
  line-family systems (`parametric_curve`/`lsystem`/`star_pattern`, each feeding the shared line
  renderer through a different generator). The three shipped baselines are **deleted**; no test pins a
  shipped preset by name, and the shipped roster keeps its behavioral floors (`sanity`/`reactivity`/
  `animation`, all iterating `default_presets()`). **Landing this greened `main`** from the `76a2fb4`
  drift. Verified: `cargo test -p lmv-core --test golden` green on WARP with a **real comparison**
  (not an adapterless skip); all six variants have exactly one fixture + baseline. Per
  [ADR-0023](../adrs/0023-golden-drift-guard-uses-frozen-fixtures.md) (now **accepted**). **Test +
  docs only — no production, C ABI (still v4), or `ci.yml` change.** **Minor (non-blocking followup):**
  the `SYSTEMS` iteration list is hand-maintained separately from the exhaustive `fixture()` match, so
  a new variant is compiler-forced to add a fixture *arm* but **not** forced into `SYSTEMS` — coverage
  is only half self-enforced (fix: assert `SYSTEMS.len()` == variant count). **Nit:** `FRAMES = 60`
  warms the stateless line/fragment fixtures needlessly (harmless). **Version: no bump** — zero
  shipped-artifact change (chore-only per ADR-0005/`docs/releasing.md`, a deliberate call, not a miss).

- [0014 — Reaction-diffusion feedback scene + frame-rate-independent render clock](done/0014-reaction-diffusion-feedback-scene.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; two minor, two nits). Six `dev`
  phase commits (`345be23`, `13148b7`, `39b6091`, `cb71057`, `9fcfc95`, `8a05cea`). Landed the
  engine's **first stateful feedback scene** — Gray-Scott reaction-diffusion on a reusable
  `render::feedback::PingPongField` (two `Rgba16Float` offscreen textures, fixed 256² grid) with an
  iso-contour + hatch + cosine-palette present look, driven by named params (`feed`/`kill`/`flow`/
  `inject` + look scalars, ADR-0002 layer 2) so bands steer the regime and beats stamp seeded growth;
  one embedded preset (`Coral`, roster **slot 5**). Threaded real **injected `dt`** through
  `Renderer::render(&frame, dt)` + a no-op-default `Scene::advance(dt)`, **retired `SCENE_DT`**
  (demoted to `FALLBACK_DT` for the ABI wrapper + capture), and made the CPU swarm
  frame-rate-independent (dt-scaled advection + `powf` damping, once/frame). Added **C ABI v4
  `lmv_render_dt`** ([ADR-0013](../adrs/0013-c-abi-v4-render-dt.md), now **accepted**; `lmv_render` =
  the exact `1/60` wrapper), header in lockstep, foobar shim QPC `measure_dt()`; new feedback render
  system per [ADR-0012](../adrs/0012-stateful-feedback-render-system.md) (now **accepted**). Both new
  scenes build GPU resources **lazily on first render** and beat injection folds into the sim shader
  (not a 4th pipeline) — documented DX12-WARP capture workarounds, real hardware unaffected. Verified:
  `fmt` / `clippy -D warnings` clean; `reaction_diffusion_contract` (sanity/animation/reactivity/
  **seed-reproducibility**), `ffi` v4, `preset` count 18, `hygiene` (both new `render/` files carry the
  panic pragma) all green. **Minor:** (1) the present pass ignores aspect (stretches the square grid) —
  an implicit choice the plan asked to document; (2) the lazy build makes the first cycle to `Coral`
  hitch once, against `cycle_preset`'s "never hitches" doc. **Nits:** stale `SCENE_DT` comment in
  `animation.rs:15`; the swarm "byte-identical at 60fps" claim is ULP-optimistic (moot — reproducibility
  holds, 0022 retires the swarm golden pin). **⚠ On-device carry-forwards** (like prior plans): Phase 2
  same-speed eyeball, Phase 4 "reads as the reference family" (dev verified via real-GPU PNGs), Phase 5
  live-foobar plugin `dt` (C++ shim not compiled here). **⚠ `main` stays red on `golden`** (pre-existing
  from `76a2fb4`, blessed cross-GPU; 0014's swarm `dt`-change also perturbs `Drift`) — **Plan 0022
  greens it**, not this close. Version **minor 0.4.0 → 0.5.0** at close.

- [0009 — Live performance features (standalone)](done/0009-live-performance-features.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; two minor deviations, both
  pre-flagged and reconciled). Five `dev` phase commits (`6e048d0` per-user config + borderless-
  fullscreen on a chosen display, `3891272` line-in / audio-interface capture selection, `bb9a1e2`
  drop-biased scene director + hotkeys, `d693c69` experimental track-change novelty nudge, `d49f377`
  `--soak` long-run instrumentation). Made the standalone drive a live DJ show: `Fullscreen::
  Borderless` on the config-selected display (`F`/`D` hotkeys, name-over-index monitor match),
  WASAPI **capture**-endpoint enumeration for line-in alongside loopback (`--list-devices`, graceful
  default fallback), a clock-free **`director`** module (MilkDrop-style dwell timer, drop bias, `A`
  toggle, manual `Space`, all a pure function of injected `dt` + `AnalysisFrame`), and a coarse soak
  sampler (elapsed/fps/RSS/heartbeat every 5 s, off the per-frame path). Core gained **one
  deterministic scalar** — `AnalysisFrame.novelty` from a new `dsp/novelty.rs` (normalized spectral
  distance from a slow running mean; pure, seeded, hot-path pragma) — consumed via the native API as
  a director *nudge* that never triggers alone. Operator choices persist in a per-user `config.toml`.
  **C ABI untouched (still v3), no ADR** (standalone-only; ADR-0001 layering applied). Verified:
  `cargo test -p lmv-core` green (incl. `novelty_spikes_at_a_spectral_boundary` + determinism
  extended to `novelty` bits); 11 `director` unit tests green (timing/bias, drop/novelty-before-min
  holds, steady-never-rotates-on-novelty, disabled-nudge, inverted-dwell clamp); `cargo clippy
  -p lmv-core -p standalone --all-targets -D warnings` clean; hygiene guard covers `dsp/novelty.rs`
  (recursive `dsp/` scan + pragma); `ffi.rs`/`ffi` tests zero-diff. **Minor (reconciled):** (1)
  `config.rs` edited in Phase 2 though its file list omitted it — the `[input]` schema was a genuine
  prerequisite (flagged in the commit body); (2) novelty is **spectral-only**, not the plan's
  "spectral/tempo" — a deliberate narrowing (a beatmatched set holds tempo across the blend, so a
  tempo term would fire on exactly the case the nudge must stay soft on; rationale in `novelty.rs`),
  fully satisfying the distinct-spectra done-when. **⚠ Carry-forwards (on-device, non-blocking, like
  prior plans):** Phase 6 (≥4-hour projector-rig soak, human); Phase 1 multi-monitor fullscreen +
  `F`/`D` + persistence-across-restart + config-delete fallback; Phase 2 line-in reactivity with an
  interface connected (loopback + `--list-devices` smoke-verified live); auto-rotate "feel" tuning
  (`NOVELTY_REF`, dwell/drop constants intentionally in code/config for on-rig calibration).
  Delivers roadmap item 2 (live performance features). Version **minor 0.3.1 → 0.4.0** at close.

- [0017 — Green CI: reasoned ttf-parser advisory ignore + adapter-skip for headless GPU tests](done/0017-ci-green-advisory-and-gpu-tests.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors, no minors; two non-actionable
  nits). Two `dev` phase commits (`95bf510`, `134d4e3`) unbreaking `main` (CI run 29985131075) after
  two **environmental** failures. **Phase 1** silenced `RUSTSEC-2026-0192` — `ttf-parser` flagged
  **unmaintained** (not a vulnerability), load-bearing via the glyphon text stack (`ttf-parser →
  fontdb → cosmic-text → glyphon → lmv-core`, both shipped targets, ADR-0009's `text` feature),
  upstream-pinned so undroppable without reversing ADR-0009 or waiting on upstream — with a single
  **reasoned, tracked `advisories.ignore`** entry (id + unmaintained-not-vuln framing + load-bearing
  path + revisit trigger) and corrected the now-false `deny.toml` `[graph]` pruning comment. **Phase 2**
  routed the three headless GPU-capture tests through a shared `headless_or_skip` helper that **skips
  on `RenderError::RequestAdapter`** (macOS lost its Metal adapter; no software fallback) and panics on
  any other build error, per [ADR-0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md) (now **accepted**) —
  keeping full assertions running on Windows WARP every push. Verified: `cargo deny check` exits 0
  (`advisories/bans/licenses/sources ok`); all three tests run their full assertions green on WARP under
  nextest; clippy `-D warnings --all-targets` clean with the production hot-path `#![deny(clippy::panic)]`
  intact (the `clippy::panic` allow is scoped to the `#[cfg(test)]` module only). **Config + test-only —
  no `ci.yml` change, C ABI untouched (still v3), no hot-path surface.** **Followup (standing):** remove
  the ignore once a glyphon/cosmic-text bump no longer pins the unmaintained ttf-parser. **Nits
  (non-actionable):** the `skipped:` notice is stderr-only (nextest hides it on pass) — the exact
  silent-no-op tradeoff ADR-0016 accepts; and the parallel session's untracked `skill-creator/` /
  `skills-lock.json` were surfaced-not-swept.

- [0010 — Line-geometry scenes: parametric curves, L-systems, star patterns](done/0010-line-geometry-scenes.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; three minor, two nits). Five
  `dev` phase commits (`110eab7`, `cd0e518`, `4b9ea05`, `1cc7fa1`, `3e2dcc1`) implementing
  [ADR-0007](../adrs/0007-line-geometry-generators.md) (now **accepted**). Added a **line-art
  category** to the built-in vocabulary on one shared `LineRenderer` (segments → thick glowing
  instanced quads, additive blend, fixed 20k-segment buffer) under two build models: a **parametric**
  system (`ParametricCurveScene`, the Maurer rose sampled per frame, allocation-free into a
  preallocated buffer) and a **generator** system built + cached at preset load
  (`LSystemScene`: grammar rewrite + turtle-walk cached per depth `1..=max_depth`; `StarPatternScene`:
  Hankin contact-angle rosette with precomputed variants). The `Scene` trait grew by **exactly one**
  optional off-hot-path method (`configure(&GeneratorConfig)`, default no-op) — the single widening
  ADR-0007 sanctions — invoked once at preset load from `configure_active_scene`. New optional
  `[curve]`/`[generator]` TOML tables validated at the load boundary (`schema.rs`), 7 curated presets
  (4 roses, 2 L-systems, 1 star) embedded + seeded, and a `presets/README.md` authoring note.
  Verified: `cargo test -p lmv-core` green — grammar exact-string (incl. Koch depth-2), turtle
  cap-truncates-and-reports, Hankin segment-count + 2π/n rotational symmetry, zero-per-frame-alloc,
  and bad-`[curve]`/`[generator]`-config rejection, all present and non-tautological; clippy
  `-D warnings` clean; hygiene guard covers all nine new `lines/*.rs` panic pragmas. **C ABI untouched
  (still v3).** **⚠ Carry-forward (minor, non-blocking):** (1) `LSystemScene::overflow()` and the
  parametric `samples` clamp *track* the segment-cap drop but nothing *surfaces* it at load — the
  plan's "never a silent cut" is unmet in the surfacing half (latent: shipped fern peaks ~6k/20k).
  (2) `presets/README.md` says `max_depth` is "clamped to 1..=7" but schema rejects `>7` as a load
  error. (3) `parametric` `configure` is skipped when `[curve]` is omitted, so `family` doesn't reset
  (harmless with one family). (4) `lsystem_fern` `visible_depth` only bumps depth at `bass == 1.0`
  exactly — an on-device tuning nit. The iGPU 60 fps @ 1080p confirmation is the standing hardware
  carry-forward (`docs/on-device-validation.md`).

- [0013 — Headless scene capture + differential visual QA + golden images + shot CLI](done/0013-headless-scene-capture.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Eight
  `dev` phase commits (`ecc50e5`, `ba68026`, `d11a7f0`, `889f4e3`, `26a3180`, `4b54d1e`, `8152943`,
  `4364464`) plus the `assets/test` gitignore (`a16be92`). Gave the agent a **windowless
  visual-feedback + QA harness**: `RenderContext::new_headless` (a surface-less device+queue, `None`
  surface so the on-surface present path is byte-unchanged) + a shared `draw_frame` extracted from
  `render`, feeding an offscreen `render/capture.rs` (clear-to-black → draw → 256-byte-aligned
  `copy_texture_to_buffer` → blocking map → tight `CaptureImage`). Two pure primitives —
  `capture_preset(name, frame, N)` (constant synthetic frame) and `capture_audio(name, pcm, fmt,
  at[])` (feeds PCM hop-by-hop through the **real** `dsp::Analyzer`, format validated at the intake
  boundary — source-agnostic rule preserved), each rebuilding scenes to their seed + resetting the
  clock so a capture is a pure function of its inputs. A dependency-free `render/metrics.rs`
  (`frame_diff`, recolor-robust Sobel `struct_diff`, `coverage`, `quadrant_spread`) powers **hard
  `core` tests**: per-band `reactivity`, `animation` (N vs N+k), `sanity` (coverage+spread against
  each scene's own sampled background — not tautological), and `beat` (a `core::signal` 120 BPM
  click track through the real DSP; a zeroed-beat-binding probe stays below the floor). Plus an
  **advisory** dual-metric `distinctness` report, `golden`-drift regression (software adapter +
  mean/outlier tolerance + `LMV_BLESS`, three eyeballed baselines), and a `standalone/examples/shot.rs`
  CLI (`--preset/--set/--frames/--size/--out`, `--all` contact sheet, `--report [--json]`,
  `--signal`/`--audio --strip` filmstrips via a hand-rolled 16-bit-PCM WAV reader). `image` is a
  **dev-dependency** in both crates ([ADR-0011](../adrs/0011-image-crate-for-capture-tooling.md), now
  **accepted**); the audio path adds **no** dependency; **C ABI untouched (still v3)**. Verified:
  full `cargo test -p lmv-core` green (18 lib unit + 8 integration binaries), `cargo clippy
  -p lmv-core -p standalone --all-targets -D warnings` clean (lints the example), hygiene guard
  covers the new `render/` files' panic pragma + the `image` exact-pin (`=0.25.10`). **⚠ Phase 9
  (human) outstanding — non-blocking:** the CC0 demo clip. Dev implemented a **safer variant** —
  `assets/test/*` is **gitignored** (tracked README only), so a clip is supplied and used *locally,
  never committed*; the `--signal` path already validates the whole audio pipeline with no asset.
  **Minor:** that gitignore supersedes Phase 9's literal "commit a WAV under `assets/test/`"
  done-when (reconciled in the closed plan). **Nit:** `core/src/signal.rs` is a new top-level core
  module outside the hygiene panic-pragma scan set and carries no pragma — acceptable, since it runs
  at capture-setup time (not per-frame or in the audio callback) and is written slice-index-free, so
  it's within the plan's own "only if it carries per-frame indexing" guidance.

- [0008 — In-app preset browse overlay (standalone)](done/0008-preset-browse-overlay.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors). Four `dev` phase commits
  (`3bef1a8`, `b0bb95e`, `43f3b39`, `9cc3234`). Landed the codebase's first text rendering:
  **glyphon** behind a non-default core `text` feature ([ADR-0009](../adrs/0009-glyphon-text-rendering.md),
  now **accepted**) via a reusable `render::text::TextLayer` seam (a second load-pass compositing
  positioned `TextRun`s over the scene in one frame; Plan 0009's HUD reuses it). The standalone draws
  the active preset name on-canvas, plus a Tab-toggled browse overlay: pure window-free `OverlayState`
  (open/highlight/filter → `OverlayAction`) with arrow-nav, case-insensitive substring type-to-filter,
  Enter selecting the **absolute** roster index, Esc closing, and hot-reload re-clamp. Selection landed
  as `Renderer::preset_names`/`select_preset`, both delegating 1:1 to a new crate-private, unit-tested
  `Roster` (the surface-free selection state — mirrors the FrameStats-behind-Diag pattern, since a live
  `Renderer` can't be built headlessly). **C ABI untouched** (`LMV_ABI_VERSION` stays 2); the plugin
  stays cycle-only. Verified: 41/41 tests (3 `Roster` + 11 `OverlayState`, incl. the absolute-index
  and no-wrap asserts) green under nextest; clippy `-D warnings` clean on the `--features text` build;
  `cargo tree` confirms glyphon absent from the default/plugin graph, present only in the standalone;
  hygiene guard covers `text.rs`'s panic pragma and the glyphon exact-pin. **⚠ Carry-forward (human,
  on-box):** Phases 1/3/4 visual done-whens ("legible on canvas", "switches the visual", "narrows
  live") and the NFR 4 binary-size delta (release `lmv.exe` ≈ 7.6 MB with glyphon; the pre-glyphon
  delta wasn't isolated — the standalone hard-enables `text`) are GPU/on-device judgments.
  **Follow-up (not blocking):** a `--features text` core build check in CI (the two-shape build's green
  is a local gate only this session), alongside the standing FFI/Miri CI notes. **Minor:** the browse
  overlay's type-to-filter drops whitespace, so a preset name containing a space can't be fully matched
  (Plan 0007's "filenames only" makes this latent, not live).

- [0012 — Measure the driver-memory floor + cull dead scenes](done/0012-memory-floor-measure-and-scene-cull.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors). Two `dev` phase commits
  (`50a7ea0`, `3de5611`); the third phase (human, low-end iGPU) was **extracted** to the standing
  `docs/on-device-validation.md` checklist so the plan could close on completed work rather than wait on
  hardware. **Phase 1** culled the three dead legacy scenes (`spectrum`/`pulse`/`starfield` — built +
  driver-compiled at startup, addressed by no preset; closes the Plan 0003 carry-forward), leaving
  `scenes/mod.rs` at `fragment_field` + `swarm`; measured delta **WS −3.3 MB / private −2.0 MB**, first
  data point that pipeline count is a weak memory lever. **Phase 2** stood up `standalone/examples/floor.rs`,
  a throwaway scene-less wgpu-context spike (construct-only — `RenderContext` exposes only
  `new`/`resize`/`surface_format`, so the example measures at the configure boundary without widening
  core's surface), isolating the fixed driver floor: **~327 MB private commit vs ~338 MB post-cull
  standalone → our whole visual system is only ~11 MB (~3%)**. This resolves ADR-0010's two open items
  (floor-vs-overhead split; pipeline count as a real-but-weak lever) and gave NFR §12 the hard
  per-system denominator it lacked (folded in at close). Verified: `cargo test -p lmv-core` 9/9,
  `cargo clippy --workspace --all-targets -D warnings` clean (lints the example too), no dangling refs
  to the deleted modules, `floor.rs` links into no shipped binary and adds no dependency, dev-box smoke
  rendered all 10 presets at ~165 fps / 0 drops. Core-internal + throwaway example; **C ABI frozen, no
  new ADR.** **⚠ Carry-forward (human):** the low-end iGPU / second-vendor capture — see
  `docs/on-device-validation.md` (does not block anything).

- [0011 — Diagnostics harness + quick-win memory/perf trim](done/0011-diagnostics-and-memory-trim.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors; two nits). Seven phase commits
  (`7ad00df`, `166043f`, `5a9f67b`, `1ace817`, `82c7134`, `d266c08`) plus two post-review fixes
  (`10a4796`, `894a2fc`). Built the runtime diagnostics brain in `core`: a pure `FrameStats`
  accumulator (fps / frame-ms / p99 from a fixed 240-sample ring, unit-tested, no clock) wrapped by a
  `Diag` holding the **single gated `Instant::now()` read** — the only wall-clock read in `core`,
  quarantined behind `collecting` so NFR §6 determinism (fixed `SCENE_DT`) holds. A `render/overlay.rs`
  final pass paints a frame-time sparkline + GPU bar + a dependency-free 5x7 bitmap-digit readout as
  instanced quads (off by default, skipped when off). Standalone: F3 toggle, dependency-free per-OS RSS,
  1 Hz rotating `diagnostics.log` on the render thread. Foobar plugin reaches the same overlay + metrics
  over new **C ABI v3** (`lmv_set_debug` + `lmv_get_metrics` + size-guarded `LmvMetrics`,
  [ADR-0008](../adrs/0008-c-abi-v3-diagnostics.md), now **accepted**) — the v3 FFI test rides in with a
  `static_assert(sizeof == 56)` layout guard. Phase 6 landed the NFR §12 levers: wgpu gated to the per-OS
  backend only (DX12/Metal, default-features off, dropping the Vulkan/GL dead weight) and an explicit
  2-frame swapchain latency. `diag/` joined the hot-path panic-pragma guard + `hygiene.rs` scan set.
  **⚠ Phase 7 outcome (human smoke, 2026-07-22, Windows AMD iGPU):** fps unchanged (~165 @ 1080p — no
  §1 regression) and overlay/title parity verified, **but the §12 footprint win failed** — release
  `lmv.exe` measured ~300 MB WS / 343 MB private, *above* the 200 MB baseline. Measured root cause: the
  trim took effect (DX12-only verified, no Vulkan/GL mapped) but footprint is dominated by the DX12
  driver-stack private heap (`amdxc64.dll` 44.8 MB + `d3dcompiler`/`D3D12Core`), which the backend-trim
  can't touch. **Backend-trim retired as the memory lever; §12's <100 MB target likely unreachable on
  DX12/wgpu.** → **Follow-up (new work, does not reopen 0011):** measure the bare wgpu driver floor,
  then revise NFR §12 or profile pipeline/shader count as the real lever. Still-standing on-device
  checks: live-foobar overlay/log (like Plan 0004) and macOS RSS (`rss.rs`, pending a Mac — Plan 0001).
  **Nits (non-blocking):** (a)
  `LmvMetrics.draw_calls` counts render passes, not GPU draw calls — name slightly wider than the value;
  (b) `foo_lmv.cpp` adds a third hardcoded app-dir literal (the Plan 0007 shared-path minor, not new).

- [0007 — Curated preset library: robust loading + seed-on-first-run + C ABI v2](done/0007-curated-preset-library.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors). Four phase commits
  (`448b54b`, `ac5e7d0`, `cf8fb5b`, `ed67807`): `core::preset::seed_dir` (write-if-absent) +
  a hand-rolled per-OS data-root resolver in the standalone seed `%APPDATA%\light-music-visualizer\presets`
  on first run, then load + hot-reload it; the foobar shim resolves the **same** dir and calls
  the new `lmv_load_presets` after every `ensure_handle`, gated on an `lmv_abi_version()`
  handshake, so both frontends share one on-disk library. The C ABI grew by exactly one
  function and bumped to **v2** ([ADR-0006](../adrs/0006-c-abi-v2-preset-loading.md), now
  **accepted**) — the first automated FFI test rides in with it (create -> load_presets on a
  temp dir -> assert count + seeded + null-path error), closing the 0001/0002 zero-FFI-coverage
  gap. Curated set expanded 4 -> 10 (calm/warp/bright fragment + drift/dense/storm swarm
  variants). A `pending_presets` stash on `RenderState`, drained by `lmv_attach_window`, handles
  a load-before-attach call order (matching ADR-0006's "install" intent). Selection stays cycle +
  title-bar; the in-app browse overlay is **Plan 0008** (drafting next). Delivers roadmap item 1's
  preset-library thread + part of item 5's install-readiness.
  **⚠ Carry-forward (human):** (a) Phase 3 live foobar smoke — builds x64 Release against v2;
  seeding + Next-scene cycling in a running foobar2000 is an on-device check (Plan 0001 Phase 8
  posture). (b) Phase 4 visual quality — "visibly distinct/reactive" across the 10 presets is an
  on-box judgment. **Minor (non-blocking):** the shared preset-path convention is a string literal
  in both frontends (`standalone/src/main.rs`, `foo_lmv.cpp`) with no single source of truth — a
  rename silently un-shares them; a cross-referencing comment is the follow-up.
- [0004 — foo_lmv as an embeddable Default UI panel](done/0004-foobar-ui-element-panel.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers, no majors). All four phases landed in
  `plugin-foobar/foo_lmv.cpp` (commits `ef9193f`, `be3f90c`, `49ed225`, `855ccba`): the file-scope
  globals became one claimable `VizSession` (single `LmvHandle` + stream + pump + render timer); a
  Default UI `ui_element` panel and the View pop-out both host the core through one HWND, sharing
  the session so only one wgpu surface exists; ownership arbitration (400 ms poll) hands the session
  to a still-open host when the owner frees, with a GDI placeholder for non-owners; "Next scene" via
  right-click + Space; and a visibility/playback-driven cadence (full while playing+visible, ~6-7 fps
  idle, timer off when hidden). **Plugin-only, no ADR** — diff touches only `foo_lmv.cpp`, the C ABI
  is unchanged (`LMV_ABI_VERSION` still 1, only the pre-existing surface called), and the
  single-`lmv_create` invariant is owner-gated on both create paths. Relates to roadmap item 4 (UX).
  **⚠ Carry-forward:** all four done-whens are runtime checks in a live foobar2000 v2 — the code
  implements each; behavioral confirmation is pending an on-device run.
- [0005 — Extract the lock-free ring into a wgpu-free crate for Miri](done/0005-miri-ring-extraction.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers, no majors). Implements Plan 0002's
  deferred Phase 5. Phase 1 (`de0fe24`) pulled the SPSC ring — `RingShared`, `SampleProducer`,
  `SampleConsumer`, `spsc()`, and the four SPSC unit tests — out of `core/src/audio.rs` into a
  new zero-dependency `lmv-ring` crate, re-exported unchanged from `core::audio` (public API and
  the C ABI intact). The ring types carry a bare `channels: u16` instead of the core-owned
  `AudioFormat` (which stays at the `intake()` boundary with its validation), driving one
  documented `capture_win.rs` call-site edit — the plan's own Risks-section fallback.
  `hygiene.rs` guards extended to cover `lmv-ring` in both the exact-pin and hot-path-pragma
  checks. Phase 2 (`6af7865`) added the `miri` CI job (`cargo +nightly miri test -p lmv-ring`) —
  fast because no wgpu graph compiles; the probe (Release→Relaxed → data-race UB) confirmed the
  gate bites. No ADR (internal refactor; the rejected feature-gate-wgpu alternative is recorded
  in the plan). **⚠ Carry-forward:** the Miri job's green-in-CI is a runtime check pending the
  push (needs the `workflow` OAuth scope on the git credential). **Minor (non-blocking):**
  `spsc()` is now crate-public in a `publish=false` crate — a slightly wider surface than the
  former module-private constructor; the `channels`-validated-by-caller contract is documented.

- [0003 — Generative scenes + data-driven presets](done/0003-generative-scenes-and-presets.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers). Phases 0-5 landed (commits
  `ae2c035..df16c48`): scenes relocated under `render/` + brought under the panic-pragma guard
  (closing the 0002 review gap), a fragment-field system and a ~10k CPU particle swarm, DSP
  enriched with bass/mid/treb bands + a deterministic hop-clock tempo/BPM, a pure
  allocation-free expression evaluator, and TOML presets driving both systems with disk
  hot-reload. Implements **[ADR-0002](../adrs/0002-layered-preset-architecture.md) layers 1-2**
  (now **accepted**). Two review fixes at close (`6b7135b`): thread-isolated the zero-alloc test
  so both `cargo test` and nextest pass, and added `preset/expr.rs` to the hygiene guard.
  **⚠ Carry-forward (minor, non-blocking):**
  1. The three legacy scenes (spectrum/pulse/starfield) stay compiled and constructed but no
     preset addresses them - a cleanup candidate (delete, or expose via a `SystemKind`).
  2. Phase 3's iGPU 60 fps @ 1080p validation (NFR 1/9) and the Phases 1/3/5 "visibly flows and
     reacts" done-whens are runtime/hardware checks, not verifiable in review - confirm on the
     iGPU test PC when available.
  **Deferred follow-ups (tracked in the closed plan):** Rhai orchestration (layer 3),
  cross-preset blending, a compute-shader particle path for thousands-scale, additional built-in
  systems (feedback/warp, boids, walkers, 3D), and exposing preset selection across the C ABI.
- [0006 — Versioning: single source of truth + cargo-release + surfacing](done/0006-versioning-wiring.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers, no majors). Implements
  [ADR-0005](../adrs/0005-versioning-and-release-cadence.md) (now **accepted**): one
  `[workspace.package].version` inherited by both crates, `cargo-release` (`release.toml`:
  `shared-version`, tag `v{{version}}`, `push = false`, `publish = false`) as the single bump
  authority, version surfaced in the standalone title via `env!("CARGO_PKG_VERSION")`.
  Phase 4 (human) confirmed: `cargo-release 1.1.3` installed, dry-run clean. **First bump run
  at close: minor `0.1.0 -> 0.2.0`, tag `v0.2.0`, not pushed** (the user pushes). C-ABI version
  (`LMV_ABI_VERSION`) stays a separate axis; the foobar plugin version remains independent.
- [0002 — Rust enforcement tooling](done/0002-rust-enforcement-tooling.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers). Phases 0-4 landed and are green
  locally (fmt, clippy `-D warnings`, both hygiene guards, cargo-deny). Panic pragma on all 7
  core hot-path files with reasoned in-bounds escapes; no production hot-path panics.
  **⚠ Carried forward (both now tracked as their own work — no loose ends):**
  1. **Phase 5 (Miri CI job) was DEFERRED, not run** — `lmv-core`'s lib pulls the full
     wgpu/naga graph, so a full-crate Miri job is impractical (>10 min). The ring IS verified
     UB-clean locally (`cargo +nightly miri test -p lmv-core --lib`, all 5 ring tests incl. the
     cross-thread SPSC case, 95 s); only the CI automation was outstanding. **→ Now
     [Plan 0005](done/0005-miri-ring-extraction.md)** (draft): extract the ring into a zero-dep
     `lmv-ring` crate and run Miri against it.
  2. **Scenes were per-frame render code outside the hot-path pragma set / guard scan.** **→
     Folded into [Plan 0003](done/0003-generative-scenes-and-presets.md) Phase 0** (amendment):
     relocate scenes under `core/src/render/scenes/` so the guard's existing recursive `render/`
     scan covers them structurally, and add the panic pragma to each — done before 0003 fills
     `scenes/` with heavy per-frame indexing.
- [0001 — Core + standalone MVP, then foobar parity](done/0001-core-and-standalone-mvp.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers; C ABI recorded in
  [ADR-0003](../adrs/0003-c-abi-v1-surface.md)). Windows standalone + foobar2000 plugin
  smoke-tested; 9/9 tests green.
  **⚠ Carried forward: Phase 10 (macOS validation on real hardware, human) was DEFERRED, not
  run** — the plan was closed early on the user's request with the Mac path still unverified
  on-device (it compiles via CI only). When a Mac is available: run the standalone on macOS
  13+, grant the screen-recording permission, confirm live visuals; report results and route
  any fixes to `dev` (the `capture_mac` path). This is the one outstanding piece of v1.

**Open gap (from the 0001/0002 reviews):** the **C ABI has no automated test coverage** — the
C++ shim is not built in CI, and no in-crate FFI test exists. 0002 armed the pragma and
supply-chain gates but did not add an FFI test (it was never a 0002 phase). A minimal
`lmv_create`/`push`/`free` Rust-side test remains an unassigned candidate for a future plan.
Miri (the deferred 0002 Phase 5) now runs in CI via [Plan 0005](done/0005-miri-ring-extraction.md),
but **only against the ring** — the FFI `unsafe` in `core/src/ffi.rs` is renderer/window-coupled,
stays in `lmv-core`, and is out of the Miri job's scope, so the FFI pointer handling is still
uncovered (its C side remains the Plan 0001 Phase-6 smoke program's job, per ADR-0003).

## Prior sequencing notes (superseded)


A tactical ordering of the **active roster** (strategic themes live in the Roadmap below).
(**Plan 0014 has now landed and closed** — the `render::feedback::PingPongField` seam, the injected-`dt`
render clock (`Renderer::render(&frame, dt)`, `Scene::advance`, `SCENE_DT` retired to `FALLBACK_DT`),
C ABI **v4 `lmv_render_dt`**, and the `ReactionDiffusion` scene all exist for the plans below to build
against; see Recently closed. Plan 0013's capture/visual-QA harness landed before it.)

(**[0029] Attractor resize cost + ink-stage followups has now landed and closed** — the attractor's
GPU resources are split along grid-dependence so a size change rebuilds only the accumulation field,
the trail grid is quantized to a 256 px step with an aspect-preserving cap, the ink stage has its
first behavioral test, and the projection uses the **target's** aspect rather than the grid's; see
Recently closed. Its `PipelineResources`/`FieldResources` split is the pattern the deferred
"target-sized internal grid for RD/trails/kaleidoscope" work would follow.)

(**[0030] Composite chain + scene keying has now landed and closed** — the composite is an owned
`PostChain` holding `Trails`/`Kaleidoscope`/`Ink` behind a declared `PostStage` trait in ADR-0018/0028
order, its routing is a **pure, unit-tested** function over the active flags, and `system_slot`'s
magic-index scene lookup is gone (scenes are keyed by `SystemKind` via an exhaustive factory over the
single `SystemKind::ALL` roster). Every golden baseline is byte-identical. See Recently closed.
**A second `PostChain` with fully independent GPU state is constructible and proven so by a test** —
that is [0023]'s dual-live unblock. [0023] **has been revised against this** (2026-07-25): its Phase 3
now says "construct a second `PostChain`", and the revision surfaced and settled where the blend sits
([ADR-0032](../adrs/0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) — ink leaves the chain).)

(**[0023] Cross-preset transitions has now landed and closed** — **every** preset switch dissolves
instead of cutting: `Space`, a browse-overlay pick, the director auto-rotate, and the C ABI
`lmv_cycle_scene`, over ~1 s, through a deterministic rotation of four blend kinds. The composite is
now `background -> scene -> PostChain (trails -> kaleidoscope) -> [blend] -> ink -> present`: ink left
the chain to become a terminal engine post-pass (ADR-0032) and the two-input blend sits between them,
outside the one-input `PostStage` trait. `Renderer::select_preset_now` is the instant-cut escape the
capture entry points use, so captures stay a pure function of their inputs. See Recently closed.
A later plan touching the render loop should pick up the five minors logged there — the switch-site
`from`/`to` ordering and the standalone's stale post-switch reads are the two with user-visible edges.)

(**[0032] Testing strategy has now landed and closed** — the suite gained a **tier 4** it did not
have: `core/tests/chain.rs` crosses the ring→analyzer→renderer seam for the first time, and
`standalone/tests/shot_cli.rs` runs the built `shot` binary as a subprocess. That second one is the
behavioral net [0031] Phase 1's `shot.rs` refactor should now be done under — its soft
"land Phase 2 before [0031]" preference **is satisfied**. Also live: an opt-in `.githooks/pre-push`
fast gate (~28 s) and a `COVERAGE_FLOOR` ratchet on `lmv-core` at **88** against a measured
**90.13 %** — [0031] deletes code, so watch that number at its close. See Recently closed.)

(**[0031] Cleanup pass has now landed and closed** — `standalone/src/shot/` holds the CLI's pure
helpers under real tests, `Renderer::from_context` is the one construction path, each binding's
easing `tau` and destination resolve **once at load** onto a render-layer route table, three pieces
of per-frame work are gone, `core/src/render/gpu.rs` is the single home for the repeated wgpu
boilerplate, and eight items of accumulated close-review debt are closed. Every golden baseline is
byte-identical. See Recently closed.)

(**[0016] GPU compute-particle scenes has now landed and closed** — the engine's first compute
pipeline + GPU-resident particle system (four strange-attractor families, data-driven `[particles]`
selection, trails on `PingPongField`) is available for later plans to build the effects layer on;
see Recently closed.)

(**[0010] Line-geometry scenes has now landed and closed** — the line-art category (roses,
L-systems, Hankin stars) built on the shared `LineRenderer` is available; see Recently closed.)

(**[0015] Preset-dir override has now landed and closed** — `LMV_PRESET_DIR` plus `shot`'s
`--presets` / `--preset-file`, both resolved through the single `standalone/src/lib.rs` resolver, so
editing a version-controlled `presets/*.toml` is live in the app (~150 ms) and in the next capture
with no rebuild; see Recently closed. That is the edit loop the scene and grammar plans below tune
their presets in.)

(**[0019] Preset grammar v2 has now landed and closed** — the expression language a preset binding is
written in is v2: `cos sqrt pow mod smoothstep` + `select`, the constants `pi`/`tau`, six comparison
operators, and the `tempo`/`novelty` variables; an unknown parameter name is a surfaced load-time
**warning** instead of a silent no-op. See Recently closed. Plans and presets below can assume the v2
vocabulary — notably **[0028]**'s `phase`/`radial_offset` rose morphing now has `tempo` and thresholds
to drive it. **Still outstanding at that close, user-gated:** the `preset-author` skill docs
(`SKILL.md`, `references/grammar.md`, `references/render-loop.md`) still described the **v1**
grammar and still taught the pre-`LMV_PRESET_DIR` `%APPDATA%` copy-over. **Both halves of that note
are now obsolete and it is kept only for the history** — the skill docs were rewritten 2026-07-26
(`1412a9b`) and have been swept since (`bin` at [0034], `hash`/`noise` at [0047]), and
`.claude/skills/**` turned out to be editable after all: writes there are classifier-dependent,
not blocked, so a stale skill doc is a close-ceremony sweep like any other.)

(**[0020] Shared palette system has now landed and closed** — the shared `core/src/render/palette.rs`
baked-LUT color surface (named + custom-stop palettes, bindable `saturation`/`hue`/`color_span`/
`color_center`/`hue_spread`/`hue_center`, and the A/B `palette_mix` crossfade) reaches **all four**
shader-colored scenes through the new off-hot-path `Scene::set_palette` hook; see Recently closed.
Plans below that build on the RD/attractor present/draw path now ride on its LUT-colored output.)

(**[0025] Full composite coverage has now landed and closed** — `bg_*` and `zoom`/`pan_*` reach the two
fullscreen/accumulating scenes (reaction-diffusion, attractor) via an alpha-present-over-backdrop and
named-param view transform; both present shaders now emit a premultiplied-alpha channel over the shared
backdrop (byte-identical over the default black). See Recently closed. Plans below that build on the
RD/attractor present path now ride on its alpha-composited output.)

(**[0027] Attractor ink-on-paper + crisp trails has now landed and closed** — the engine-wide final
composite stage `render/ink.rs` gives *every* scene a black-on-white / colored-duotone mode via the
bindable `ink_*`/`paper_*` params, and the attractor's trail grid follows its render target instead of
a fixed 640x360. See Recently closed. The ordering wiring it left **[0023]** — ink must remap the
*blended* frame, so the transition blend goes before it — was designed in
[ADR-0032](../adrs/0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) and **has now landed**
as 0023's Phase 1: ink left the chain to become a terminal post-pass, byte-identical goldens, so the
blend sits ahead of it without widening the `PostStage` trait. A tidy-up candidate still
open in `render/scenes/reaction_diffusion.rs:1048`: correct the stale "fullscreen opaque pass" comment
on the RD present, left from before 0025's alpha switch — **carried by [0031] Phase 6**.)

[0010]: done/0010-line-geometry-scenes.md
[0015]: done/0015-preset-dir-override-and-live-iteration.md
[0016]: done/0016-gpu-compute-particle-scenes.md
[0019]: done/0019-preset-grammar-v2.md
[0020]: done/0020-shared-palette-system.md
[0023]: done/0023-cross-preset-transitions.md
[0025]: done/0025-full-composite-coverage.md
[0027]: done/0027-attractor-ink-and-crisp-trails.md
[0028]: done/0028-parametric-curve-shape-params.md
[0029]: done/0029-attractor-resize-cost-and-ink-followups.md
[0030]: done/0030-composite-chain-and-scene-keying.md
[0031]: done/0031-composite-cleanup-and-debt.md
[0032]: done/0032-testing-strategy-e2e-coverage-and-pre-push.md
[0033]: done/0033-internal-resolution-and-preset-surface.md
[0034]: done/0034-preset-reachable-spectrum.md
[0035]: done/0035-composite-aspect-and-grid-policy.md
[0036]: done/0036-macos-and-windows-release-artifacts.md
[0039]: done/0039-line-joins.md
[0044]: done/0044-quality-tiers.md
[0045]: done/0045-linear-light-and-bloom.md
[0047]: done/0047-expression-randomness.md
[0048]: done/0048-analysis-v2-and-the-retune.md
[0051]: done/0051-the-scene-seam-emits-premultiplied-alpha.md
[0052]: done/0052-the-emitter-objects-that-spawn-fall-and-die.md
[0053]: done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md
[0054]: done/0054-the-line-scenes-catch-up.md
[0055]: done/0055-the-fold-edge-becomes-a-choice.md
[0056]: done/0056-clamp-occupancy-and-the-axis-anchor.md
[0057]: done/0057-the-attractors-compute-path.md
[0059]: done/0059-lorenz-finds-its-plane.md
[0063]: done/0063-the-attractor-keeps-its-depth.md
[0069]: done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md
[0071]: done/0071-light-that-adds-without-covering.md
[0074]: done/0074-the-figure-colours-by-how-far-it-has-come.md
[0075]: done/0075-the-content-renaissance.md
[ADR-0037]: ../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md
[0067]: done/0067-the-curation-route.md
[0076]: done/0076-the-second-layer.md
[0077]: done/0077-the-quiet-sky.md
[0078]: done/0078-the-ink-learns-to-bite.md
[0082]: done/0082-the-gradient-stops-banding.md
[0084]: done/0084-two-gates-stop-lying-about-what-they-check.md
[0087]: 0087-the-line-renderer-draws-a-curve.md

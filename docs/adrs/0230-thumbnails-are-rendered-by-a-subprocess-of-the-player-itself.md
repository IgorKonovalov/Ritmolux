# ADR-0230 — Thumbnails are rendered by a subprocess of the player itself

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0206](../plans/0206-the-browser-shows-the-look.md)
> **Relates to:** [ADR-0011](0011-image-crate-for-capture-tooling.md) (`image` is a dev-dependency
> only), [ADR-0010](0010-accept-gpu-driver-memory-floor.md) (the driver memory floor),
> [ADR-0014](0014-preset-dir-override-for-dev-iteration.md) (`RLX_PRESET_DIR`),
> [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md) (what a tag ships),
> [NFR §4](../nfr.md#4-size-and-dependencies) (the size cap this measures against)

## Context

The browser presents 114 presets as a list of identifiers. `analytic_echoplate` does not say what
it looks like, so choosing means opening things at random — the problem
[Plan 0205](../plans/done/0205-the-library-becomes-navigable.md)'s filters narrow but cannot solve,
because they narrow a list of names into a shorter list of names.

The fix is a picture per preset. The decision is where that picture comes from, and three
measurements taken on 2026-09-19 on this project's development box (Windows 10, release build)
close off most of the options.

**The binary has no room.** The release `ritmolux.exe` measures **10,971,648 B** against
[NFR §4](../nfr.md#4-size-and-dependencies)'s **10,000,000 B** soft cap — already **9.7 % over**, on
a cap that document itself says *"has never been measured against what the exe actually
contains."* A 160x90 still costs **42,994 B** as PNG, so the shipped set is about **4.9 MB** of
thumbnails; embedding takes the exe to roughly 15.9 MB. Reaching for a denser format does not
rescue it either, because decoding one at runtime means promoting an image codec into the shipped
binary, and [ADR-0011](0011-image-crate-for-capture-tooling.md) deliberately keeps `image` a
**dev-dependency**.

**So the pictures are generated on the user's machine, and that costs real time.** One 160x90 still
at 300 frames takes **5.65 s**, so covering the shipped library is about **10.7 minutes** of
rendering. It is not a one-frame screenshot because most of this engine's interesting families
accumulate: [backlog 0254](../design-backlog.md) records that hop 300 is *before* an accumulating
world exists, so if anything that frame count is low for the families that most need a picture.

**That work has to happen next to a running show, and the show wins.** `CLAUDE.md`'s
non-negotiables put frame stability first, and a thumbnail is not the console's live preview — that
is a copy of the frame already being drawn, whereas this is **a second scene**, with its own
accumulation history, rendered while the first one plays.

One more fact shapes the mechanism. `shot` is `standalone/examples/shot.rs`, a cargo **example**:
the release zip ships `ritmolux.exe` and nothing else, so a shipped app cannot spawn `shot`. But
the headless machinery is not in the example — it lives in `standalone/src/shot/`, library code the
binary already links, with the example as a thin front end.

## Decision

We will render thumbnails in **a subprocess of the player itself**: `ritmolux.exe` re-invoked
through `std::env::current_exe()` in a headless render-one-thumbnail mode, one preset at a time, at
low OS priority. A background pass starts at launch and works through the library until it is
covered, **whether or not the browser is ever opened**.

Output is cached beside `config.toml`, one file per preset, keyed by the preset **name** — the same
identity [ADR-0228](0228-a-preset-mark-is-user-state-keyed-by-name-in-its-own-file.md) uses and
that [spec 0001](../specs/0001-c-abi.md) settled — and carrying a **stamp of the source file**
(modification time and length) so an edited preset re-renders. That makes the cache correct for
`RLX_PRESET_DIR` libraries, which is the loop most likely to want a browser.

**A missing thumbnail is a normal state, not an error.** The preview pane shows the preset's name
and a placeholder, so the feature degrades exactly to today's behaviour: on first launch, before a
render lands, and forever on a machine where the pass cannot run at all.

**The show never waits.** Nothing in the frame loop blocks on the subprocess, and if it fails, is
killed, or cannot obtain a GPU, the pass stops and records why in `diagnostics.log` rather than
retrying into the ground.

## Consequences

### Positive

- **Frame-loop isolation is a property, not a promise.** A separate process has its own wgpu device
  and its own queue, so it cannot stall the show's submission no matter how badly it behaves. An
  in-process budget can only be *tuned* to avoid that, and tuning is what is wrong on the machine
  you did not test.
- **Nothing ships and nothing grows.** No thumbnail bytes in the binary, no second artifact in the
  five packaging recipes, no change to any `READ-ME-FIRST.md`, and no codec dependency.
- **It covers whatever library is loaded**, including a `RLX_PRESET_DIR` directory of converted
  MilkDrop presets, which a shipped pack could never do.
- **It reuses machinery that is already tested.** `standalone/src/shot/` is the same code path the
  headless CLI and the visual-QA harness drive.

### Negative

- **A second GPU context exists while the pass runs.** [ADR-0010](0010-accept-gpu-driver-memory-floor.md)
  records that the driver stack has a memory floor this project already accepted once; during the
  pass it is paid twice. On a low-memory machine this is the thing most likely to bite.
- **Process spawn is paid 114 times.** That fixed per-process cost is precisely what
  [Plan 0199](../plans/0199-the-gates-cost-is-measured-before-it-is-cut.md) is measuring for the
  preset sweeps, and this decision commits to paying it before that measurement is in.
- **About 10.7 minutes to cover the shipped set on the measured machine**, and unknown on a slower
  GPU. A user who opens the browser in the first minutes sees a mostly empty pane, which is the
  weakest moment of the design and the one a first-time user meets.
- **Repeated process spawns look like malware to endpoint security.** On Windows, launching the
  same executable 114 times in a few minutes is a pattern some scanners throttle or block outright,
  and the failure will be reported as "thumbnails do not work" with no obvious cause.
- **A still understates an accumulating world.** The frame count is a judgement that will be right
  for some families and wrong for others, and no measurement in this repository can tell which.

### Neutral

- The foobar component does not participate. It has no browser of its own and the C ABI is
  untouched.
- The cache grows with every library ever browsed and nothing prunes it — the same standing cost
  ADR-0228 accepted for marks, for the same reason: with `RLX_PRESET_DIR` able to point anywhere,
  there is no sound rule for which library is authoritative.

## Alternatives considered

### Alternative A — an in-process background tenant on a frame budget

Share the show's wgpu device and advance thumbnail frames only when the frame has spare time.
**Rejected because the failure mode is a stutter during a performance.** A budget is a number that
has to be right on every GPU, and when it is wrong the cost lands on exactly the thing this
project's non-negotiables protect. Isolation by construction beats a budget by care, and the
saving — one GPU context — is not worth the class of bug it buys.

### Alternative B — embed the thumbnails at build time

Glob and embed them beside the presets, as [ADR-0022](0022-build-time-preset-embedding.md) does for
the `.toml` files. **Rejected on the measurement.** It is 4.9 MB onto a binary already 9.7 % over
its cap, a denser format needs a runtime codec against ADR-0011, and it would still cover only the
shipped set — a user browsing their own directory would get nothing.

### Alternative C — ship a thumbnail pack beside the exe in the zip

Put the pictures in the release zip as their own file. **Rejected because it cannot cover a
user-supplied library at all**, which is the `preset-author` loop and the most likely consumer of a
picture browser. It also adds a build step and a file that goes stale against `presets/` with
nothing to notice, which is the shape this project has repeatedly found expensive.

### Alternative D — promote `shot` to a shipped binary and spawn that

Make the example a `[[bin]]` and ship it. **Rejected because it buys nothing over re-invoking the
executable that is already installed.** It would add a second artifact to five packaging recipes
and to the READ-ME-FIRST files a tester reads, and make the release surface bigger in exchange for
a spawn target the app already has in `current_exe()`.

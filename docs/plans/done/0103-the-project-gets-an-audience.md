# 0103 — The project gets an audience

> **Status:** done — closed 2026-09-18. Four phases landed (`2c9cbbca`, `b72b035`, `cff81675` +
> `684ddeba`, `e5d7c7f4`), plus `d6e275e6`, the close review's five prose repairs. Round-1 verdict:
> **no blockers, no majors, five minors and two nits** — five repaired, two left open (the
> uncompiled, unmeasured Phase 1, and the clip's clone weight). Version **0.131.1** (patch). No ADR
> paired, and the one Phase 1 was conditioned on is genuinely not owed. Backlog 0102 and 0103 closed.
> **Created:** 2026-08-16
> **Approved:** 2026-08-16 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** none yet — this is mostly distribution work, but **Phase 1 may earn one**
> (see that phase)
> **Amended:** 2026-08-16 — Phase 1 added after [Plan 0102](0102-the-component-ships.md)'s
> Phase 5 found the shipped component starves its host; former Phases 1-5 renumbered 2-6
> **Closes:** design-backlog 0102, design-backlog 0103
> **Soft dependency:** [0101](0101-the-engine-renders-a-music-video.md) (closed — `shot --render` records motion)
> **Coordinates with:** [0156](0156-the-site-becomes-the-reference.md) (closed) — it moved the
> operator and developer sections out of `README.md` into `docs/` (ADR-0169): the file Phase 2 here
> reorders is now **346 lines, not 650**, and its operator material is
> [running.md](../../running.md) and [configuration.md](../../configuration.md).
> The architecture diagram now lives in `docs/how-it-works.md`; the README's `## Architecture` is a
> short paragraph pointing at it.
> **Split 2026-09-18:** Phases 5 and 6 — the component submission and the three posts — moved to
> [0192](../0192-the-component-reaches-its-audience.md), with the dependencies that were theirs
> ([0102](0102-the-component-ships.md), Plan 0176, backlog 0196). This plan now ends at Phase 4.

> **Amended 2026-09-14** (architect backlog sweep): Phase 1's files follow Plan 0126 Phase 8's split
> of `foo_ritmolux.cpp` (`1779520`) into `host_window.cpp` / `viz_session.cpp`; the repository
> figures are re-measured, which shrinks Phase 4 to topics + social preview and turns Phase 2 into
> "move Download up" (the README no longer opens on a diagram); Phase 3's "if 0101 has not landed"
> branch is dropped; the risks name the five release zips and the studio question; the "no website"
> exclusion is rewritten against ADR-0154/0167/0169; and backlog 0196 is folded in as a precondition
> on Phase 5 — the close ceremony's re-tag writes lightweight tags that `git push --follow-tags`
> skips, so a tag can exist locally and never fire a release.

## TL;DR

Ninety-seven plans, 110 ADRs, 66 releases — and **1 star, 0 forks, and no repository
description** when this plan was written (2026-08-16; re-measured 2026-09-14 in the table below).
This plan does the small, unglamorous, mostly non-technical things that stand
between a finished product and anyone knowing it exists: a README that leads with the product,
a demo that moves, and repository metadata. Its done-whens
are about **shipping the artifacts, not about the outcome** — nobody can plan adoption, and a plan
that promised it would be lying.

**The last two steps are no longer here.** The submission and the posts both point strangers at a
component that must exist as a published release first, and that release can only carry Phase 1's
fix once *this* plan merges — so they were a phase waiting on their own plan's close. They are
[Plan 0192](../0192-the-component-reaches-its-audience.md), which starts from the release instead.

**Phase 1 is the exception, and it comes first.** Everything after it points strangers at the
foobar2000 component, and as of [Plan 0102](0102-the-component-ships.md)'s Phase 5 that
component makes foobar2000 itself feel dead until the user starts playback — while looking
perfectly fine. Driving an audience into that is not a smaller version of this plan's goal, it is
the opposite of it, so the fix is a phase here rather than a plan somewhere else.

## Context & problem

The engine is real and measured: 165 fps median at Rich/1080p with zero dropped frames over 28,698,
a linear-light HDR composite, dual-resolution analysis, two frontends off one core, a golden suite
pinned on a software rasterizer. None of that is visible to anyone.

The concrete state of the repository, checked 2026-08-16 and re-measured 2026-09-14
(`gh repo view`):

| | 2026-08-16 | 2026-09-14 |
|---|---|---|
| Stars / forks | 1 / 0 | 4 / 0 |
| Repository description | **empty** | set |
| Homepage | none | the documentation site |
| Topics | none | **none** |
| Custom social preview | none | **none** |
| Demo video | none — every image in the repo is a still | unchanged |
| Component in foobar2000's repository | not submitted | not submitted |

When this plan was written the README opened on architecture: a stranger's first screen was a
mermaid diagram of the audio path. That half is fixed — it now opens on `hero.png` and a gallery
table, and the diagram moved to `docs/how-it-works.md`. What is still true is that
**`## Download` sits below `## Architecture` and the long `## Repository layout` block**, so a
stranger scrolls past a directory tree before finding the thing to install. And the one genuinely
uncontested position this project holds — a serious visualizer inside foobar2000, where the only
competitor is a port of a 2007 plugin — is reachable through a component nobody can install.

**This is currently the binding constraint on the project**, and it costs hours rather than plans.

## Decision

Do the distribution work as a tracked plan with a close ceremony, rather than as a someday. The
phases are ordered so the `dev` phases produce the material — a README, a clip, a preview — that
Phase 4 and then [Plan 0192](../0192-the-component-reaches-its-audience.md) publish; nothing
here is clever and that is deliberate — **except Phase 1, which is a design pass and is ordered
first because every later phase increases the number of people who meet the defect it fixes.**

## Implementation phases

### Phase 1 — the component survives a stranger's first five minutes

- **Owner skill:** dev
- **What:** Fix [design-backlog 0102](../../design-backlog-archive.md) — the panel attaches its wgpu surface
  before it has a real client rect — and [design-backlog 0103](../../design-backlog-archive.md) — the panel's
  `WM_CONTEXTMENU` shadows foobar2000's layout-edit menu, so it cannot be removed by the documented
  route. Both pre-date this plan. Since Plan 0126 Phase 8 (`1779520`) split the shim, the surface
  lifetime lives in `plugin-foobar/viz_session.cpp` (`VizSession::ensure_handle`, the
  `needs_reattach` flag, the watchdog, and the diagnostics row that prints `gpu_bytes`) and the
  shared window procedure in `plugin-foobar/host_window.cpp` (`wnd_proc`, its `WM_CONTEXTMENU`
  arm). `plugin-foobar/foo_ritmolux.cpp` keeps the `ui_element_instance` that holds the callback.
- **Files touched:** `plugin-foobar/viz_session.cpp`, `plugin-foobar/host_window.cpp`,
  `plugin-foobar/foo_ritmolux.cpp` (the edit-mode query reaches the panel through its
  `ui_element_instance_callback`); an ADR if the decision below goes the way it probably has to.
- **Notes for the implementer — read the backlog entries first; the 2026-08-16 evidence narrows
  the choice they leave open.** Entry 0102 offers two fixes: **defer the attach** until a
  non-degenerate `WM_SIZE`, or **re-check `needs_reattach` from the 500 ms watchdog** commit
  `6f2862c` added. Plan 0102's Phase 5 run favours the first. The reported symptom there was not a
  panel that never presented — it was a panel presenting a **correct picture at 154 ms per frame**,
  8.7x its steady state, with `draw_calls` and `gpu_bytes` byte-identical either side of the
  recovery. A watchdog looking for a surface that *never became real* would not have fired: this
  one became real enough to draw. **The window is panel creation until playback starts**, because
  the only thing that repairs it today is `ensure_handle` rebuilding the handle on the first
  stream-format change — which is why it hurts a new user worst, who looks at a visualizer before
  pressing play.
  **You will need an instrument that does not exist.** Entry 0102 records that `gpu_bytes` reports
  the *config* size rather than the surface's, so the one field an operator would reach for cannot
  distinguish a correctly-sized surface from a badly-sized one. Adding the surface's actual
  configured size to the plugin's diagnostics is part of this phase, not a nicety — the done-when
  below is not measurable without it.
  For 0103, `ui_element_instance_callback` exposes the edit-mode query, but the **pop-out host has
  no such callback and no layout to edit**, so the two hosts stop sharing one `WM_CONTEXTMENU`
  branch. That sharing is deliberate in `host_window.cpp`. **If the fix ends that arrangement, it is an
  ADR** — the shim's two-host design is exactly the kind of thing a future reader will otherwise
  re-litigate from scratch.
- **Done when:** on a fresh foobar2000 with the panel docked and **nothing playing**, the plugin's
  first logged frame times are the same as the steady state it reaches after playback starts —
  the two are no longer separated by an order of magnitude, which is the property the 8.7x
  measurement violated. The diagnostics log reports the surface's real configured size. And with
  layout editing enabled, right-clicking the panel surfaces **foobar2000's** menu and Remove works,
  while with it disabled the component's own menu still appears.
  **Then re-run Plan 0102 Phase 5's checklist** — the section of
  [`on-device-validation.md`](../../on-device-validation.md) headed *"Runnable now — the foobar2000
  component's clean-profile install"* — which is where the original evidence lives and which is the
  only functional check this component has.

### Phase 2 — the README leads with the product

- **Owner skill:** dev
- **What:** The hero picture and the gallery already lead (Plan 0156). What remains is order:
  **move `## Download` up**, above `## Architecture` and `## Repository layout`, so the first screen
  is what it is, what it looks like, and how to get it. Prepare (but do not apply) the topic list;
  the description is already set on the repository.
- **Files touched:** `README.md`, and a short `packaging/repo-metadata.md` holding the topic list
  (and the description text as applied, so it has a committed source) for Phase 4.
- **Notes for the implementer:** the sections are already strong and should mostly keep their
  wording — this is a reordering, not a rewrite. The status paragraph (pre-1.0, formats may change)
  stays visible; understating instability to look finished would be the wrong trade. **The open
  question is settled — the owner's call, 2026-09-18: name the studio in `## Download` and nowhere
  above it.** The opening sections and the hero stay player-first, because the studio is young and
  the first screen should not promise it. But a `v*` tag attaches five zips where the Download table
  lists three and says so in words, so a reader who follows the README to the Releases page finds
  two artifacts it does not explain. This phase therefore adds `studio-macos-universal` and
  `studio-windows-x64` to that table, with one sentence on what the studio is for and where the site
  explains it, and the note that each carries its own player so there is no second download. Prefer
  a count-free sentence over correcting "Three zips" to "Five".
- **Done when:** a reader who has never seen the project learns what it is, sees it, and finds the
  download **without scrolling past the repository layout**. The topic list exists as committed
  text for Phase 4.

### Phase 3 — a demo that moves

- **Owner skill:** dev
- **What:** A short rendered clip of the app running, plus a social-preview still, produced from a
  committed manifest the way every other image here is
  ([ADR-0100](../../adrs/0100-documentation-images-are-committed-headless-renders.md)).
- **Files touched:** `scripts/docs-shots.mjs` (or a sibling), `docs/images/`.
- **Notes for the implementer:** **this is the phase that wants
  [0101](0101-the-engine-renders-a-music-video.md)** — `shot --render` is the only way this repo
  can record motion, and screen-capturing the window would be the one image here that is not a
  reproducible render. 0101 has closed, so `--render` is available; do not introduce a
  hand-captured video.
- **Done when:** the clip and the still are committed, regenerable by an argument-free script, and
  the manifest records the preset, stimulus and size behind each.

### Phase 4 — the repository says what it is

- **Owner skill:** human
- **What:** Apply the metadata. Outward-facing, so the user does it. The description and homepage
  were already set by 2026-09-14, so this phase is topics and the preview:

  ```sh
  gh repo edit --add-topic music-visualizer --add-topic rust --add-topic wgpu \
               --add-topic foobar2000 --add-topic audio-visualization
  ```

  Plus the social preview image from Phase 3, in the repository settings.
- **Done when:** the repository has topics and a custom preview image
  (`gh repo view --json repositoryTopics,usesCustomOpenGraphImage`), and a link pasted into a chat
  shows the picture rather than a grey placeholder.

## Risks & open questions

- **Mac users will be the first testers of a path that has never run.** The macOS build compiles
  in CI and has **never executed on Apple hardware** ([NFR §9](../../nfr.md#9-test-hardware-matrix-what-the-user-has)).
  An announcement will produce Mac downloads. The README already says this; Phase 2 must keep it
  above the fold rather than tidying it away, and the posts — now
  [0192](../0192-the-component-reaches-its-audience.md) Phase 3 — should say it in the post itself.
- **Every shipped artifact is unsigned** — a `v*` tag now ships five zips (the standalone for
  Windows and macOS, the foobar2000 component, and the studio for Windows and macOS), so the
  first-run experience on both platforms is an OS warning. This is known and accepted
  ([NFR §8](../../nfr.md#8-distribution-v1)); what it means here is that the friction is highest at
  exactly the moment attention is highest. **The studio question was Phase 2's to settle and is
  settled** (owner's call, 2026-09-18): the README names the studio in `## Download` and nowhere
  above it. Whether a post pitches it is [0192](../0192-the-component-reaches-its-audience.md)'s.
- **The library was small and lopsided** when this plan was written — four systems with exactly one
  world each. [Plan 0104](0104-the-library-stops-being-lopsided.md) has since closed and the
  embedded set has grown several-fold, so this risk is largely retired; a visitor still judges the
  content, not the composite.
- **This plan cannot promise adoption** and does not. Every done-when is an artifact. **Phase 1 is
  the one exception to that and is held to a measured property instead**, which is the right trade
  but a different kind of promise from the rest of the plan.
- **Phase 1 is not the first change to the shim's window/ownership path** — `6f2862c`'s render
  timer and the surface work before it, then [Plan 0107](0107-the-foobar-menu-picks-a-preset.md)'s
  menu rebuild and Plan 0126's split into `host_window.cpp` / `viz_session.cpp` all came first.
  [Backlog 0102](../../design-backlog.md) says in as many words
  that it *"wants a design pass over surface lifetime, not another edge case handled"* — and that
  it was filed rather than fixed precisely to avoid *"a third guess layered on two"*. Treat a fix
  that only makes the reported symptom go away as a failure of this phase, not a pass.
- **Nothing in CI can verify Phase 1.** No runner loads foobar2000, so its done-when is checked by
  hand against [`on-device-validation.md`](../../on-device-validation.md) — the same gap the macOS path
  has ([ADR-0115](../../adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)).
  The evidence it is checked against is one machine and one host version.
- **Phase 1 has a release cost the others do not, and it is what forced the split.** It changes
  shipped plugin behaviour, so the fixed component reaches nobody until a `v*` tag carrying it is
  pushed and its release is green — and that tag cannot exist until this plan closes and merges. A
  submission phase inside this plan was therefore waiting on this plan's own close. It is
  [0192](../0192-the-component-reaches-its-audience.md) Phase 1, which is the release, and the
  submission follows it there.
- **Contention:** `plugin-foobar/viz_session.cpp`, `host_window.cpp`, `foo_ritmolux.cpp` (Phase 1),
  `README.md`, `docs/images/`, `scripts/`. A close ceremony's image re-render is the usual other
  writer to `docs/images/` — sequence if they land together.

## What this plan does NOT do

- **No new website work, and no domain.** The documentation site already exists
  ([ADR-0154](../../adrs/0154-the-reader-facing-docs-publish-as-a-site.md)), owns its own entrance and
  install page ([ADR-0167](../../adrs/0167-the-site-owns-its-entrance-and-the-install-page-is-the-testers-own-file.md)),
  and is where the README sends a reader for reference
  ([ADR-0169](../../adrs/0169-the-site-is-organised-by-reader-task-and-the-readme-stops-being-a-reference.md)).
  This plan links to it; it does not redesign it or buy it a domain.
- **No submission and no posts.** Both moved to
  [0192](../0192-the-component-reaches-its-audience.md) on 2026-09-18, with the release they stand on.
- **No paid promotion, no mailing list, no social accounts.**
- **No code signing** — that stays a future plan and a `human` cost.
- **No submission to app stores or package managers.**

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `WORK/rlx-plan-0103` on `plan-0103-the-project-gets-an-audience`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the component survives a stranger's first five minutes | dev | done | `2c9cbbc` |
| 2 — the README leads with the product | dev | done | `b72b035` |
| 3 — a demo that moves | dev | done | `cff8167`, `684dded` |
| 4 — the repository says what it is | human | done | `e5d7c7f` |

### Notes

- Phase 1 also edits `plugin-foobar/viz_session.h`, which the phase's file list does not name: the
  session's state (`needs_reattach` → `want_rate`/`want_channels`/`surface_w`/`surface_h`) and its
  methods are declared there.
- Phase 1's backlog-0103 half did **not** end the two hosts' shared `WM_CONTEXTMENU` branch, so no
  ADR was written. The branch asks one per-window question — `host_defers_context_menu(HWND)`,
  answered from the panel's `ui_element_instance_callback` through a `GWLP_USERDATA` back-pointer —
  and the pop-out, which never writes that word, answers `false` by construction.
- Phase 1's done-when is not checkable in this lane and none of it was run here: the component was
  **not compiled** (the foobar2000 SDK is gitignored and unstaged in this worktree, and
  `packaging/foobar/fetch-sdk.ps1` is outside the conductor session's allowlist), and the frame-time,
  surface-size and layout-edit-menu checks are the on-device checklist's, under
  [`on-device-validation.md`](../../on-device-validation.md)'s *"Runnable now — the foobar2000
  component's clean-profile install"*.
- Phase 2's studio sentence does **not** say where the site explains the studio, because the site
  does not: `PUBLISHED` in `site/src/plugins/rewrite-links.mjs` carries no studio page, and
  `packaging/studio/READ-ME-FIRST.md` is deliberately outside it. The sentence links that shipped
  file in the repository instead.
- Phase 2 also re-spelled the three existing Download rows with their `ritmolux-` prefix. With the
  studio rows added, `…-macos-universal.zip` and `…-windows-x64.zip` each match two of the five
  assets, so the elided form no longer names one.
- `packaging/repo-metadata.md` records the description as the text to set, not as the text applied:
  the live value was set outside the repository around 2026-09-14 and nothing in the checkout
  records what it says, so a claim either way would be unchecked.
- The plugin diagnostics log's row shape changed (four columns appended). Its header is written only
  when the file is created, so a `plugin-diagnostics.log` carried over from an earlier build gets
  wide rows under a narrow header; the clean-profile install the done-when uses starts a new file.
- Phase 3's script is a sibling, `scripts/docs-clip.mjs`, rather than an edit to
  `scripts/docs-shots.mjs`: that script's stated contract is that it writes `docs/images/**.png` and
  nothing else and spawns only `cargo`, and a video writes a container through an external encoder.
- Phase 3 synthesizes its own stimulus WAV (`target/docs-clip/stimulus.wav`, not committed).
  `--render` refuses `--signal`, nothing in the repository writes a WAV and no clip is committed, so
  the script carries a second, smaller implementation of `dynamic_groove`'s musical shape in
  JavaScript. It is not sample-identical to core's and does not claim to be.
- Phase 3's clip is 6.2 MB — the largest file in the repository. `--crf 28` rather than `shot`'s
  archival default of 18 is what holds it there; the same render at `--crf 23` is 10.7 MB.
- Followup noticed and not acted on: `CLAUDE.md`'s `scripts/` block says its renderers are "a rule
  with five named exceptions" and names five. `scripts/docs-clip.mjs` is a sixth.
- Followup noticed and not acted on: nothing links `docs/images/demo.mp4`. Phase 3's files are
  `scripts/` and `docs/images/`, and Phase 6 is where the clip is posted.
- Phase 4 applied by the owner, 2026-09-18. `gh repo edit` set the five topics
  `packaging/repo-metadata.md` names, and the social preview was uploaded through **Settings →
  General → Social preview**. `gh repo view --json repositoryTopics,usesCustomOpenGraphImage`
  reads all five and `usesCustomOpenGraphImage: true`. The done-when's last clause — a link pasted
  into a chat showing the picture rather than a grey placeholder — is not checked here: GitHub
  caches the card, so it is read at the first paste rather than on demand.
- Phase 4 found the preview unuploadable as Phase 3 wrote it. GitHub refuses a social preview over
  1 MB and the truecolour PNG was 1.18 MB, so `684ddeba` gave the manifest entry a `maxBytes`
  budget, quantized the committed picture to 446 KB to meet it, and made a still that is still over
  budget fail the run.
- Phase 4 reconciled `packaging/repo-metadata.md`'s Description to the live one, which that file
  says is part of applying it. The direction was the owner's call: the live text names the platforms
  and carries the gallery URL where the file's draft did neither, and it is what search results and
  every social card already carry, so the file moved and the repository did not.

## Close review

> Round 1, 2026-09-18. Mode 4 in conductor mode (ADR-0205): a separate headless process, handed
> this plan and the lane and nothing an implementer wrote outside the repository. Written to
> `tools/conductor/state/reviews/0103-round-1.md` and reproduced here in full, because a
> conductor-run close has no reader in the room.

### Verdict

**Plan 0103 landed cleanly: no blockers, no majors, five minor items and two nits.** Phase 1 is a
real design pass over surface lifetime rather than a third guess layered on two — the fallback size
and the flag that repaired it are both gone, and ownership moved from the handle to the window,
which is what [backlog 0102](../../design-backlog-archive.md) asked for and what this plan told the
implementer not to substitute a symptom patch for. Phases 2 and 4 meet their done-whens as written.
Phase 3 produced both artifacts from an argument-free manifest.

Every minor is documentation that this plan made false, except one: **the Phase 1 C++ has never been
compiled or measured anywhere**, and that one cannot be repaired in this lane or in a fix round.

### Evidence

| Check | Result |
|---|---|
| `with-lock.mjs suite -- cargo nextest run --workspace` | `skipped: tree 101ffa5 is green in the suite ledger, run by gate 0103-pre-review at 2026-09-18T13:50:38.809Z: 2004 tests run: 2004 passed (6 slow), 7 skipped` — the ledger record is the full-suite evidence (ADR-0207) |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo doc --workspace --no-deps`, rustdocflags `-D warnings` | clean, six crates |
| `check-doc-links`, `toc --check`, `check-index-rows`, `check-comment-hygiene`, `check-reader-prose`, `check-system-counts` | all OK |
| `check-backlog-claims` | OK — 93 reductions across 38 live entries, 3 unprobeable; 36 moved-path advisory rows, none this plan's |
| `check-translations` | OK, 5 stamped; the advisory was empty before this close's repairs |

### Lens 1 — alignment with the plan

Four phases, four in-vocabulary owner tags (`dev`, `dev`, `dev`, `human`). The log is shorter than
`## Implementation phases`, as ADR-0120 asks, and reports by exception in the right places —
including three things a log is tempted to omit: that Phase 1 was never compiled, that `--crf 28`
was measured against `--crf 23`, and that `CLAUDE.md` now undercounts the renderers.

Done-whens, read against the tree rather than the log. **Phase 2:** `## Download` is at
`README.md:41`, above `## Architecture` and `## Repository layout`; the studio appears there and
nowhere above it; the sentence is count-free; `packaging/repo-metadata.md` carries the topic list
Phase 4 applied. **Met.** **Phase 3:** both artifacts committed, the script argument-free, and every
`shot` flag it passes exists with the meaning it relies on — including the parser rule that
`--render` refuses `--signal`/`--audio`, which is why the stimulus is synthesized. Hop 300 is inside
the clip. **Met.** **Phase 4:** topics and preview recorded, with the cached-card clause honestly
marked unchecked. **Met.** **Phase 1: not verified, and the log says so** (see minor 5).

The ADR this plan conditioned on Phase 1 is genuinely not owed: `host_window.cpp`'s
`WM_CONTEXTMENU` is still **one** branch asking one per-window question, answered from the panel's
`ui_element_instance_callback` through a `GWLP_USERDATA` back-pointer, with the pop-out answering
`false` by construction rather than by a special case. The two-host design is preserved, not ended.

### Lens 2 — layering, coupling, real-time safety

Nothing under `core/` changed and the C ABI is untouched: the four new diagnostics columns are the
*shim's* own state, which is exactly the distinction `gpu_bytes` structurally cannot make — widening
the ABI to expose a surface size would have been the ADR-worthy move and was correctly not taken.
No audio-thread change: everything Phase 1 touches runs on the main thread, and the newly placed
`announce_current()` is reachable only from a window message. The handle guards are complete, which
is the property the new no-fallback rule depends on — `claim` can now return `true` with no handle,
and the render tick, `pump`, `size_surface`, `maybe_log_metrics`, `announce` and both input arms all
test it, while `destroy_handle` zeroes the surface size with the handle. The surfaceless retry is
bounded by the 500 ms watchdog and is *slower* than the 400 ms arbitration retry it replaces. No
reference to either removed member survives in any source.

### Lens 3 — doc freshness and release bookkeeping

Where every repairable finding is: the plan changed two things a user observes and added a fourth
`shot`-driving script whose output is committed, and four documents asserted the old state
(minors 1-4). Nothing else in the sweep table is implicated — no param, grammar, palette, schema,
preset, hotkey, CLI flag, config key, OSC address or NFR budget moved. The version bump is a
**patch**: a fix to shipped plugin behaviour plus documentation, metadata and a maintenance
renderer, which is no new capability of either frontend
([ADR-0005](../../adrs/0005-versioning-and-release-cadence.md)).

### Lens 4 — correctness and determinism

The boundary still validates once, in one place: `client_size` rejects a null window, a failed
`GetClientRect` and any non-positive extent before a surface is ever configured, and it is the
single computation of that answer. No aspect ratio is derived from anything but the target — the
only sizes in the diff are the window's own client extent and the manifest's declared output sizes.
No new Rust, so no new panic path. The one new randomness is explicitly seeded
(`mulberry32(0x5eed0103)`), so the stimulus is reproducible rather than merely similar. Every
numeric claim in the diff is a measurement that names its configuration (10.7 MB at `crf` 23
against 6.2 MB at 28 on this preset at this size; 1.18 MB truecolour against 446 KB quantized
against GitHub's 1 MB limit), none is asserted by a test, and the one assertion the script does make
— fail the run when the still is still over budget — is the upload's own rule rather than a frozen
number about a machine.

### Lens 5 — design integrity

Dependency direction unchanged. The seam that could have widened did not: reaching a host-specific
answer from a shared window procedure is exactly where a shim grows a `host_kind` enum or a second
window class, and instead the question lives in the one file that knows what a `ui_element` is,
leaving the procedure ignorant of both hosts and the branch count at one. The back-pointer is set
after `CreateWindowExW` and cleared before `DestroyWindow`, which is the ordering that makes it
safe. `ensure_handle` is now a two-line recorder in front of `attach_if_ready`, which is the whole
of the create-and-attach decision — one job per function where there were three spread across two.
No new hot-path Rust module, so Plan 0002's guard set needs no extension.

### Findings

- **minor 1 — the shipped and published install page still listed both fixed defects as known.**
  `packaging/foobar/READ-ME-FIRST.md` section 5 called a black docked panel "a known defect in this
  build" and told a reader to route around the layout-edit right-click through Preferences, and
  section 6 asked whether a track change fixed the panel. That file is both the `READ-ME-FIRST.txt`
  in the component zip and the site's foobar2000 install page (ADR-0167), so the live install page
  described defects the next release does not have. Repaired in `d6e275e6`, keeping the questions
  useful rather than deleting them, because the fix is unverified on device: the bullets say the
  defect is fixed in this build and ask the tester to report it if it is still there. Its Russian
  second copy goes stale in the same edit — ADR-0185's advisory row, and content work for the
  translator.
- **minor 2 — the component's only functional check still expected both defects to fail.**
  [`on-device-validation.md`](../../on-device-validation.md)'s clean-profile item said in as many
  words that "(b) and (e) failing is the *expected* result", and Phase 1's own done-when routes its
  verifier through that item. The four new diagnostics columns — the instrument this phase added so
  the done-when is measurable — were unnamed there. Repaired in `d6e275e6`: (b) and (e) now state
  the criteria to pass, name the columns, and say plainly that the item is the first reading of a fix
  that closed unmeasured. Every dated run record untouched.
- **minor 3 — `docs/capturing.md` said three committed scripts drive `shot`.** `docs-clip.mjs` is a
  fourth and the second whose output is committed, and the only one needing an external encoder.
  Repaired in `d6e275e6`.
- **minor 4 — the renderer roster in `CLAUDE.md` undercounted by one.** "A rule with five named
  exceptions" named five; `docs-clip.mjs` is a sixth, and the `docs/images/` line in both layout
  blocks credited `docs-shots.mjs` alone. Repaired in `d6e275e6`.
- **minor 5 — the Phase 1 C++ has never been compiled, and its done-when has never been measured.**
  Left open. The lane has no foobar2000 SDK and the fetch script is outside the session's allowlist,
  so the change was not compiled; `ci.yml` builds no C++, so the **first compilation of this code is
  `release.yml`'s `foobar` job on the tag this close writes**; and the measurements are the
  clean-profile checklist's, which nobody has run. A fix round cannot close this either, and the
  repository's own rule is that on-device checks do not gate closes — so it is a minor with an owner
  action: `plugin-foobar/build.ps1` for the compile, then the checklist item this close rewrote.
  What a reading can carry: the change compiles cleanly by inspection —
  `is_edit_mode_enabled()` is a real `ui_element_instance_callback` method, `m_callback` is a
  `service_ptr` with `is_valid()`, both new free functions are declared and defined in
  `namespace rlx`, the `fprintf` gains four `%u` and four `unsigned` arguments, and nothing
  references the two removed members. That is not a substitute for a compiler.
- **nit 6 — two log rows named no commit.** Phases 3 and 4 read "committed with this row", which
  stops resolving the moment the plan moves to `done/`. Filled in with `cff8167` / `684dded` and
  `e5d7c7f` in `d6e275e6`.
- **nit 7 — the clip is the largest file in the repository and nothing links it.** Left open.
  6.19 MB committed, no document references it, and the plan's followups point at
  [Plan 0192](../0192-the-component-reaches-its-audience.md) Phase 3 as its consumer.
  [ADR-0100](../../adrs/0100-documentation-images-are-committed-headless-renders.md)'s negative
  sized the *entire* committed image set at "~28 MB to every clone forever" and noted the decision
  is one-way; one file is now a fifth of that figure. The lever was measured and recorded in the
  manifest, which is the right record — what is left for the owner is that this plan's risks never
  weighed a clone-weight cost, and that the file has no reader until 0192 posts it. Deleting or
  re-encoding a committed artifact is not something a close repairs.

No earlier round raised a finding: this is round 1, and it closed the plan.

## Followups (after this lands)

- **The submission and the posts are [0192](../0192-the-component-reaches-its-audience.md)**, which
  also owns the release they need. The log's note that nothing links `docs/images/demo.mp4` points
  at a Phase 6 that lives there now; the clip is that plan's Phase 3 material.
- Winget / Homebrew, if there is demand.
- Code signing, if the SmartScreen friction shows up in reports rather than in speculation.

# 0103 — The project gets an audience

> **Status:** in-progress
> **Created:** 2026-08-16
> **Approved:** 2026-08-16 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** none yet — this is mostly distribution work, but **Phase 1 may earn one**
> (see that phase)
> **Amended:** 2026-08-16 — Phase 1 added after [Plan 0102](done/0102-the-component-ships.md)'s
> Phase 5 found the shipped component starves its host; former Phases 1-5 renumbered 2-6
> **Closes:** design-backlog 0102, design-backlog 0103
> **Soft dependency:** [0101](done/0101-the-engine-renders-a-music-video.md) (closed — `shot --render` records motion)
> **Hard dependency for Phase 5:** [0102](done/0102-the-component-ships.md)
> **Coordinates with:** [0156](done/0156-the-site-becomes-the-reference.md) (closed) — it moved the
> operator and developer sections out of `README.md` into `docs/` (ADR-0169): the file Phase 2 here
> reorders is now **346 lines, not 650**, and its operator material is
> [running.md](../running.md) and [configuration.md](../configuration.md).
> The architecture diagram now lives in `docs/how-it-works.md`; the README's `## Architecture` is a
> short paragraph pointing at it.
> **Hard dependency for Phase 5 (added 2026-09-14):** Plan 0176 — a `v*` tag must verifiably reach
> `origin` and produce a published release before anything is submitted (backlog 0196).

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
a demo that moves, repository metadata, a component submission, and three posts. Its done-whens
are about **shipping the artifacts, not about the outcome** — nobody can plan adoption, and a plan
that promised it would be lying.

**Phase 1 is the exception, and it comes first.** Everything after it points strangers at the
foobar2000 component, and as of [Plan 0102](done/0102-the-component-ships.md)'s Phase 5 that
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
phases are ordered so the `dev` phases produce material the `human` phases then publish; nothing
here is clever and that is deliberate — **except Phase 1, which is a design pass and is ordered
first because every later phase increases the number of people who meet the defect it fixes.**

## Implementation phases

### Phase 1 — the component survives a stranger's first five minutes

- **Owner skill:** dev
- **What:** Fix [design-backlog 0102](../design-backlog-archive.md) — the panel attaches its wgpu surface
  before it has a real client rect — and [design-backlog 0103](../design-backlog-archive.md) — the panel's
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
  [`on-device-validation.md`](../on-device-validation.md) headed *"Runnable now — the foobar2000
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
  ([ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md)).
- **Files touched:** `scripts/docs-shots.mjs` (or a sibling), `docs/images/`.
- **Notes for the implementer:** **this is the phase that wants
  [0101](done/0101-the-engine-renders-a-music-video.md)** — `shot --render` is the only way this repo
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

### Phase 5 — the component reaches its audience

- **Owner skill:** human
- **What:** Submit the `.fb2k-component` to the foobar2000 component repository.
- **Precondition (added 2026-09-14, backlog 0196):** Plan 0176 has landed, and the tag carrying
  Phase 1's fix is **on `origin` and has a published release with the component zip attached** —
  checked with `git ls-remote --tags origin` and `gh release view <tag>`, not assumed from a local
  `git tag`. The failure this guards against is silent: most `v*` tags since `v0.115.0` exist only
  locally, because the close ceremony's re-tag writes a lightweight tag and `--follow-tags` pushes
  only annotated ones.
- **Done when:** the submission is filed. **Hard-depends on
  [0102](done/0102-the-component-ships.md)** — there is nothing to submit until that plan produces a
  released artifact, and submitting a locally built DLL with no release behind it would be worse
  than waiting.

### Phase 6 — tell three specific places

- **Owner skill:** human
- **What:** Post where the audience already is, not everywhere. Hydrogenaudio's foobar2000 forum
  (the component's actual home), `r/foobar2000`, and `r/rust` (which cares about the wgpu/real-time
  engineering, not the visuals).
- **Done when:** the three posts exist. **The plan closes on the posts, not on the reception** —
  and if the reception is informative, it becomes design-backlog entries, which is the only
  outcome this plan can honestly commit to producing.

## Risks & open questions

- **Mac users will be the first testers of a path that has never run.** The macOS build compiles
  in CI and has **never executed on Apple hardware** ([NFR §9](../nfr.md#9-test-hardware-matrix-what-the-user-has)).
  An announcement will produce Mac downloads. The README already says this; Phase 2 must keep it
  above the fold rather than tidying it away, and Phase 6's posts should say it in the post itself.
- **Every shipped artifact is unsigned** — a `v*` tag now ships five zips (the standalone for
  Windows and macOS, the foobar2000 component, and the studio for Windows and macOS), so the
  first-run experience on both platforms is an OS warning. This is known and accepted
  ([NFR §8](../nfr.md#8-distribution-v1)); what it means here is that the friction is highest at
  exactly the moment attention is highest. **Phases 2 and 6 must decide whether to pitch the
  studio** — it is a release artifact nobody has been told about, and a post that names it invites
  a third first-run path.
- **The library was small and lopsided** when this plan was written — four systems with exactly one
  world each. [Plan 0104](done/0104-the-library-stops-being-lopsided.md) has since closed and the
  embedded set has grown several-fold, so this risk is largely retired; a visitor still judges the
  content, not the composite.
- **This plan cannot promise adoption** and does not. Every done-when is an artifact. **Phase 1 is
  the one exception to that and is held to a measured property instead**, which is the right trade
  but a different kind of promise from the rest of the plan.
- **Phase 1 is not the first change to the shim's window/ownership path** — `6f2862c`'s render
  timer and the surface work before it, then [Plan 0107](done/0107-the-foobar-menu-picks-a-preset.md)'s
  menu rebuild and Plan 0126's split into `host_window.cpp` / `viz_session.cpp` all came first.
  [Backlog 0102](../design-backlog.md) says in as many words
  that it *"wants a design pass over surface lifetime, not another edge case handled"* — and that
  it was filed rather than fixed precisely to avoid *"a third guess layered on two"*. Treat a fix
  that only makes the reported symptom go away as a failure of this phase, not a pass.
- **Nothing in CI can verify Phase 1.** No runner loads foobar2000, so its done-when is checked by
  hand against [`on-device-validation.md`](../on-device-validation.md) — the same gap the macOS path
  has ([ADR-0115](../adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)).
  The evidence it is checked against is one machine and one host version.
- **Phase 1 has a release cost the others do not.** It changes shipped plugin behaviour, so the
  fixed component only reaches anyone on the next `v*` tag — which means the ordering constraint is
  stronger than "Phase 1 first": **the tag has to be pushed and its release green before Phase 5
  submits anything.** Backlog 0196 showed that "pushed" cannot be read off a local `git tag`: most
  recent tags never reached `origin`. Phase 5's precondition (Plan 0176) is what makes this bullet
  checkable.
- **Contention:** `plugin-foobar/viz_session.cpp`, `host_window.cpp`, `foo_ritmolux.cpp` (Phase 1),
  `README.md`, `docs/images/`, `scripts/`. A close ceremony's image re-render is the usual other
  writer to `docs/images/` — sequence if they land together.

## What this plan does NOT do

- **No new website work, and no domain.** The documentation site already exists
  ([ADR-0154](../adrs/0154-the-reader-facing-docs-publish-as-a-site.md)), owns its own entrance and
  install page ([ADR-0167](../adrs/0167-the-site-owns-its-entrance-and-the-install-page-is-the-testers-own-file.md)),
  and is where the README sends a reader for reference
  ([ADR-0169](../adrs/0169-the-site-is-organised-by-reader-task-and-the-readme-stops-being-a-reference.md)).
  This plan links to it; it does not redesign it or buy it a domain.
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
| 3 — a demo that moves | dev | done | committed with this row |

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
  [`on-device-validation.md`](../on-device-validation.md)'s *"Runnable now — the foobar2000
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

## Followups (after this lands)

- Winget / Homebrew, if there is demand.
- Code signing, if the SmartScreen friction shows up in reports rather than in speculation.

# 0103 — The project gets an audience

> **Status:** approved
> **Created:** 2026-08-16
> **Approved:** 2026-08-16 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** none yet — this is mostly distribution work, but **Phase 1 may earn one**
> (see that phase)
> **Amended:** 2026-08-16 — Phase 1 added after [Plan 0102](done/0102-the-component-ships.md)'s
> Phase 5 found the shipped component starves its host; former Phases 1-5 renumbered 2-6
> **Closes:** design-backlog 0102, design-backlog 0103
> **Soft dependency:** [0101](done/0101-the-engine-renders-a-music-video.md) (nothing here can currently record motion)
> **Hard dependency for Phase 5:** [0102](done/0102-the-component-ships.md)

## TL;DR

Ninety-seven plans, 110 ADRs, 66 releases — and **1 star, 0 forks, and no repository
description**. This plan does the small, unglamorous, mostly non-technical things that stand
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

The concrete state of the repository, checked 2026-08-16:

| | |
|---|---|
| Stars / forks | 1 / 0 |
| Repository description | **empty** |
| Topics | none |
| Age | 26 days |
| Demo video | none — every image in the repo is a still |
| Component in foobar2000's repository | not submitted |

The README is good, but it opens on architecture: a stranger's first screen is a mermaid diagram
of the audio path, not what the thing is or what it looks like moving. And the one genuinely
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
- **What:** Fix [design-backlog 0102](../design-backlog.md) — the panel attaches its wgpu surface
  before it has a real client rect — and [design-backlog 0103](../design-backlog.md) — the panel's
  `WM_CONTEXTMENU` shadows foobar2000's layout-edit menu, so it cannot be removed by the documented
  route. Both pre-date this plan and both live in `plugin-foobar/foo_ritmolux.cpp`.
- **Files touched:** `plugin-foobar/foo_ritmolux.cpp`; an ADR if the decision below goes the way it
  probably has to.
- **Notes for the implementer — read the backlog entries first; the 2026-08-16 evidence narrows
  the choice they leave open.** Entry 0102 offers two fixes: **defer the attach** until a
  non-degenerate `WM_SIZE`, or **re-check `needs_reattach` from the 500 ms watchdog** commit
  `1016777` added. Plan 0102's Phase 5 run favours the first. The reported symptom there was not a
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
  branch. That sharing is deliberate in this file. **If the fix ends that arrangement, it is an
  ADR** — the shim's two-host design is exactly the kind of thing a future reader will otherwise
  re-litigate from scratch.
- **Done when:** on a fresh foobar2000 with the panel docked and **nothing playing**, the plugin's
  first logged frame times are the same as the steady state it reaches after playback starts —
  the two are no longer separated by an order of magnitude, which is the property the 8.7x
  measurement violated. The diagnostics log reports the surface's real configured size. And with
  layout editing enabled, right-clicking the panel surfaces **foobar2000's** menu and Remove works,
  while with it disabled the component's own menu still appears.
  **Then re-run [Plan 0102 Phase 5](../on-device-validation.md)'s checklist**, which is where the
  original evidence lives and which is the only functional check this component has.

### Phase 2 — the README leads with the product

- **Owner skill:** dev
- **What:** Restructure the first screen. What it is, the hero picture, the download, the controls
  — then architecture. Prepare (but do not apply) the repository description and topic list.
- **Files touched:** `README.md`, and a short `packaging/repo-metadata.md` holding the description
  text and topic list for Phase 4 to apply.
- **Notes for the implementer:** everything below "Architecture" is already strong and should mostly
  keep its wording — this is a reordering, not a rewrite. The status paragraph (pre-1.0, formats may
  change) stays visible; understating instability to look finished would be the wrong trade.
- **Done when:** a reader who has never seen the project learns what it is, sees it, and finds the
  download **without scrolling past a diagram**. The description text and topics exist as committed
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
  reproducible render. If 0101 has not landed, ship the still and say in the commit that the clip
  is owed; do not introduce a hand-captured video.
- **Done when:** the clip and the still are committed, regenerable by an argument-free script, and
  the manifest records the preset, stimulus and size behind each.

### Phase 4 — the repository says what it is

- **Owner skill:** human
- **What:** Apply the metadata. Outward-facing, so the user does it:

  ```sh
  gh repo edit --description "<from packaging/repo-metadata.md>" \
               --add-topic music-visualizer --add-topic rust --add-topic wgpu \
               --add-topic foobar2000 --add-topic audio-visualization
  ```

  Plus the social preview image from Phase 3, in the repository settings.
- **Done when:** the repository has a description, topics and a preview image, and a link pasted
  into a chat shows the picture rather than a grey placeholder.

### Phase 5 — the component reaches its audience

- **Owner skill:** human
- **What:** Submit the `.fb2k-component` to the foobar2000 component repository.
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
- **Both binaries are unsigned**, so the first-run experience on both platforms is an OS warning.
  This is known and accepted ([NFR §8](../nfr.md#8-distribution-v1)); what it means here is that
  the friction is highest at exactly the moment attention is highest.
- **The library is small and lopsided** — 39 presets, four systems with exactly one world each
  ([Plan 0104](done/0104-the-library-stops-being-lopsided.md)). A visitor who tries it judges the
  content, not the composite. There is a real argument for running 0104 first; that is a
  sequencing call for the roster, not a blocker written into this plan.
- **This plan cannot promise adoption** and does not. Every done-when is an artifact. **Phase 1 is
  the one exception to that and is held to a measured property instead**, which is the right trade
  but a different kind of promise from the rest of the plan.
- **Phase 1 is the third change to `foo_ritmolux.cpp`'s window/ownership path**, after `1016777`'s render
  timer and the surface work before it. [Backlog 0102](../design-backlog.md) says in as many words
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
  submits anything.**
- **Contention:** `plugin-foobar/foo_ritmolux.cpp` (Phase 1), `README.md`, `docs/images/`, `scripts/`.
  Nothing on the roster touches these except a close ceremony's image re-render, which
  [0087](done/0087-the-line-renderer-draws-a-curve.md) owes — sequence if they land together.

## What this plan does NOT do

- **No website, no landing page, no domain.** The repository is the landing page.
- **No paid promotion, no mailing list, no social accounts.**
- **No code signing** — that stays a future plan and a `human` cost.
- **No submission to app stores or package managers.**

## Followups (after this lands)

- Winget / Homebrew, if there is demand.
- Code signing, if the SmartScreen friction shows up in reports rather than in speculation.

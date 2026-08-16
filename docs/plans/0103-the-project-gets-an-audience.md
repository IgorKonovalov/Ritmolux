# 0103 — The project gets an audience

> **Status:** draft
> **Created:** 2026-08-16
> **Owner skill(s):** dev, human
> **Related ADRs:** none — this is distribution work, not a design choice
> **Soft dependency:** [0101](0101-the-engine-renders-a-music-video.md) (nothing here can currently record motion)
> **Hard dependency for Phase 4:** [0102](0102-the-component-ships.md)

## TL;DR

Ninety-seven plans, 110 ADRs, 66 releases — and **1 star, 0 forks, and no repository
description**. This plan does the small, unglamorous, entirely non-technical things that stand
between a finished product and anyone knowing it exists: a README that leads with the product,
a demo that moves, repository metadata, a component submission, and three posts. Its done-whens
are about **shipping the artifacts, not about the outcome** — nobody can plan adoption, and a plan
that promised it would be lying.

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
phases are ordered so the two `dev` phases produce material the `human` phases then publish;
nothing here is clever and that is deliberate.

## Implementation phases

### Phase 1 — the README leads with the product

- **Owner skill:** dev
- **What:** Restructure the first screen. What it is, the hero picture, the download, the controls
  — then architecture. Prepare (but do not apply) the repository description and topic list.
- **Files touched:** `README.md`, and a short `packaging/repo-metadata.md` holding the description
  text and topic list for Phase 3 to apply.
- **Notes for the implementer:** everything below "Architecture" is already strong and should mostly
  keep its wording — this is a reordering, not a rewrite. The status paragraph (pre-1.0, formats may
  change) stays visible; understating instability to look finished would be the wrong trade.
- **Done when:** a reader who has never seen the project learns what it is, sees it, and finds the
  download **without scrolling past a diagram**. The description text and topics exist as committed
  text for Phase 3.

### Phase 2 — a demo that moves

- **Owner skill:** dev
- **What:** A short rendered clip of the app running, plus a social-preview still, produced from a
  committed manifest the way every other image here is
  ([ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md)).
- **Files touched:** `scripts/docs-shots.mjs` (or a sibling), `docs/images/`.
- **Notes for the implementer:** **this is the phase that wants
  [0101](0101-the-engine-renders-a-music-video.md)** — `shot --render` is the only way this repo
  can record motion, and screen-capturing the window would be the one image here that is not a
  reproducible render. If 0101 has not landed, ship the still and say in the commit that the clip
  is owed; do not introduce a hand-captured video.
- **Done when:** the clip and the still are committed, regenerable by an argument-free script, and
  the manifest records the preset, stimulus and size behind each.

### Phase 3 — the repository says what it is

- **Owner skill:** human
- **What:** Apply the metadata. Outward-facing, so the user does it:

  ```sh
  gh repo edit --description "<from packaging/repo-metadata.md>" \
               --add-topic music-visualizer --add-topic rust --add-topic wgpu \
               --add-topic foobar2000 --add-topic audio-visualization
  ```

  Plus the social preview image from Phase 2, in the repository settings.
- **Done when:** the repository has a description, topics and a preview image, and a link pasted
  into a chat shows the picture rather than a grey placeholder.

### Phase 4 — the component reaches its audience

- **Owner skill:** human
- **What:** Submit the `.fb2k-component` to the foobar2000 component repository.
- **Done when:** the submission is filed. **Hard-depends on
  [0102](0102-the-component-ships.md)** — there is nothing to submit until that plan produces a
  released artifact, and submitting a locally built DLL with no release behind it would be worse
  than waiting.

### Phase 5 — tell three specific places

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
  An announcement will produce Mac downloads. The README already says this; Phase 1 must keep it
  above the fold rather than tidying it away, and Phase 5's posts should say it in the post itself.
- **Both binaries are unsigned**, so the first-run experience on both platforms is an OS warning.
  This is known and accepted ([NFR §8](../nfr.md#8-distribution-v1)); what it means here is that
  the friction is highest at exactly the moment attention is highest.
- **The library is small and lopsided** — 39 presets, four systems with exactly one world each
  ([Plan 0104](0104-the-library-stops-being-lopsided.md)). A visitor who tries it judges the
  content, not the composite. There is a real argument for running 0104 first; that is a
  sequencing call for the roster, not a blocker written into this plan.
- **This plan cannot promise adoption** and does not. Every done-when is an artifact.
- **Contention:** `README.md`, `docs/images/`, `scripts/`. Nothing on the roster touches these
  except a close ceremony's image re-render, which [0087](0087-the-line-renderer-draws-a-curve.md)
  owes — sequence if they land together.

## What this plan does NOT do

- **No website, no landing page, no domain.** The repository is the landing page.
- **No paid promotion, no mailing list, no social accounts.**
- **No code signing** — that stays a future plan and a `human` cost.
- **No submission to app stores or package managers.**

## Followups (after this lands)

- Winget / Homebrew, if there is demand.
- Code signing, if the SmartScreen friction shows up in reports rather than in speculation.

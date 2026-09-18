# Repository metadata

The GitHub repository's own metadata — description, homepage, topics, social preview — is not in
any file GitHub reads, so it has no source and no history unless one is kept by hand. This is that
source. It is **not applied by anything**: the values below are set through the GitHub API or the
repository's settings page, by a person, and this file is what they are set *from* and what a
later reader compares the live values against.

## Description

> A lightweight real-time music visualizer for Windows and macOS: a standalone app and a
> foobar2000 component over one Rust + wgpu core, driven by editable text presets. Docs and preset
> gallery: https://igorkonovalov.github.io/Ritmolux/

240 characters is GitHub's limit and the box is shown on one line in search results, so the useful
length is much shorter than the limit.

**Reconciled 2026-09-18 to what is live, not the other way round.** The description was first set
on the repository around 2026-09-14, outside this file, and the two had since drifted. The live
text is the better one — it names the platforms a reader is deciding about and carries the gallery
URL, which the shorter draft did neither — and it is what search results and every social card
already carry, so rewriting the repository to match a file nobody had applied would have cost
indexing to gain nothing. The URL is duplicated from Homepage on purpose: a search result shows the
description, and not always the homepage field beside it.

## Homepage

```
https://igorkonovalov.github.io/Ritmolux/
```

The documentation site (ADR-0154). Already set.

## Topics

```sh
gh repo edit --add-topic music-visualizer --add-topic rust --add-topic wgpu \
             --add-topic foobar2000 --add-topic audio-visualization
```

Five, chosen as the terms someone looking for *this* would type. GitHub allows twenty; the rest of
what this project is — `dsp`, `shader`, `winit`, `electron` — describes how it is built rather than
what it is for, and a topic list that reads as a dependency manifest buys no reader.

Check what is live with:

```sh
gh repo view --json repositoryTopics,homepageUrl,description,usesCustomOpenGraphImage
```

## Social preview

`docs/images/social-preview.png` — a headless render, produced by `node scripts/docs-clip.mjs`
like every other committed picture here (ADR-0100). Uploaded under **Settings → General → Social
preview**; there is no `gh` flag for it.

**GitHub refuses an image over 1 MB**, and this frame in truecolour is 1.18 MB, so the manifest
entry carries a `maxBytes` budget and the script quantizes to a 256-colour palette to meet it —
446 KB, and the run fails rather than committing a file the upload would reject. Without one, a link pasted into a chat or a post shows a
grey placeholder with the repository name, which is the whole reason the file exists.

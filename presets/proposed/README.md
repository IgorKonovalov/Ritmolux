# `presets/proposed/` — authored content awaiting the owner's verdict

A preset here was **authored but not judged**. It is not approved, not curated, and not shipped.
It is here so that one person can look at a night's worth of work in one sitting and say keep,
tune or bin.

**This is the opposite of [`pending/`](../pending/README.md).** A file in `pending/` is *finished
and approved*, held out of the shipped set by a named engine or harness gap; it leaves the moment
that gap lifts. A file here carries no approval at all. `pending/` is waiting on the engine; this
is waiting on you.

**Nothing in this directory is embedded.** `core/build.rs` builds the shipped set with a
non-recursive `read_dir` over `presets/` plus a `*.toml` extension filter (ADR-0022), so a
subdirectory is skipped by construction — the same mechanism that holds `pending/` out. Files
here reach neither the binary, nor the behavioral suite, nor the gallery. Adding fifty of them
cannot turn a gate red.

## Looking at them

A labeled contact sheet of everything in here, in one command:

```bash
cargo run -p standalone --example shot --release -- \
    --presets presets/proposed --all --out target/proposed --tier rich \
    --signal dynamic:110 --frame-at 300 --size 640x360
```

One of them, full size:

```bash
cargo run -p standalone --example shot --release -- \
    --preset-file presets/proposed/<name>.toml --out one.png --tier rich \
    --signal dynamic:110 --frame-at 300 --size 1280x720
```

Or drive the running app at the whole folder and edit with a ~150 ms hot reload (ADR-0014):

```powershell
$env:RLX_PRESET_DIR = "./presets/proposed"; cargo run -p standalone --release
```

## The three verdicts

- **Keep** — `git mv` it into `presets/`. That move is a `dev` act, not an author one, and it owes
  two things `pending/README.md` spells out and `core/tests/hygiene.rs` refuses a push without: a
  gallery card in `scripts/docs-shots.mjs` plus its committed render, and a filename in the shipped
  `<system>_<look>.toml` form. Files here are already named that way, so a keeper needs no rename.
- **Tune** — leave it here and route it back to `preset-author`.
- **Bin** — delete it. Nothing points at it and nothing breaks.

## What is in here

Each preset carries a header comment saying what it is and what to look at. `ROSTER.md` is the
index — one row per preset, appended by whichever authoring pass produced it.

## Where this came from

The directory exists because presets are judged by eye and nothing else, so authoring can run
unattended but curation cannot. It is a staging area for that split, not a parking lot for work
in progress — an author who is still iterating has not produced a candidate yet.

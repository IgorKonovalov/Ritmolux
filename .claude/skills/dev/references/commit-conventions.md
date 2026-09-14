# Commit conventions — dev

Conventional Commits, one logical change (or one plan phase) per commit. There is no commitizen
hook in this repo yet; the convention is enforced by discipline and the architect's review.

## Format

```
<type>(<scope>): <subject>

[optional body — wrap at ~72 chars, plain ASCII]

[optional footer — references, BREAKING CHANGE]
```

- **type**: from the table below.
- **scope**: the crate/area touched. Optional but encouraged — it scans the log far better.
- **subject**: imperative, lowercase, no trailing period, ≤ ~72 chars.

Commit the message via the **PowerShell tool's single-quoted here-string** (`@'...'@`, closing
`'@` at column 0). Keep the body plain ASCII — straight hyphens, no em-dashes, no internal
double-quotes — or git may misparse the here-string into stray pathspecs.

## Types

| Type       | When to use |
|------------|-------------|
| `feat`     | A new user-visible capability — a new scene, a new DSP output, a new C ABI function, capture working on a platform. |
| `fix`      | A bug fix in existing code. |
| `refactor` | Internal restructuring, no behavior change. |
| `perf`     | A change made specifically for real-time/throughput/latency. |
| `test`     | Adding or fixing tests. (Tests for new code in the same phase fold into that `feat` commit.) |
| `docs`     | Markdown, doc comments, ADRs, plans, READMEs. |
| `chore`    | Misc maintenance — file moves, `.gitignore`, tooling config. |
| `build`    | Build-system / dependency changes (`Cargo.toml`, lockfile, the plugin build project). |
| `ci`       | CI config under `.github/workflows/`. |

## Scopes

Smallest meaningful scope. Omit if a commit truly spans many (usually a sign to split).

| Scope        | Area |
|--------------|------|
| `core`       | `core/` generally |
| `audio`      | `core/src/audio.rs` — sample intake |
| `ring`       | `rlx-ring/` — the extracted SPSC ring |
| `dsp`        | `core/src/dsp/` — FFT, onset, beat, bands |
| `render`     | `core/src/render/` — wgpu layer, composite stages |
| `scenes`     | `core/src/render/scenes/` |
| `preset`     | `core/src/preset/` — the loader (`schema/`) + expression evaluator |
| `schema`     | the generated editor schemas — `presets/schema/`, `.taplo.toml`, their export (ADR-0190) |
| `milk`       | `core/src/milk/` — the MilkDrop runtime (ADR-0113) |
| `ffi` / `cabi` | `core-cabi/` — the C ABI crate: `src/lib.rs`, `include/`, `tests/ffi.rs` (ADR-0072); the log uses both |
| `standalone` | `standalone/` — winit, capture, input, the `shot` example |
| `plugin`     | `plugin-foobar/` — C++ shim |
| `milkconv`   | `milkconv/` — the ahead-of-time `.milk` converter |
| `studio`     | `studio/` — normally `studio-builder`'s scope; yours only for a Rust-side change it names |
| `packaging`  | `packaging/` — the release zips' recipes (ADR-0038) |
| `scripts`    | `scripts/` — the Node gates and renderers |
| `site`       | `site/` — the documentation front end (ADR-0154) |
| `hooks`      | `.githooks/` and `.claude/hooks/` |
| `tooling`    | `Cargo.toml`, lockfile, rust-toolchain, `.gitignore` |
| `ci`         | `.github/workflows/` |
| `docs`       | anything under `docs/` |

For `docs` commits the log uses a finer scope naming the *audience* rather than the path —
`docs(architect)`, `docs(plans)`, `docs(presets)`, `docs(preset-author)`. Match what's already
there; `git log --format=%s -40` shows the live vocabulary faster than this table can track it.

## Examples

```
feat(dsp): windowed FFT producing normalized log-frequency spectrum
```

```
feat(audio): lock-free SPSC ring buffer for source-agnostic sample intake

Validates sample rate and channel count once at push_samples; the
downstream DSP path trusts them. No allocation on the producer side.
```

```
perf(audio): drop the mutex in the WASAPI callback for a lock-free ring

The capture callback must never block. Replaces the Mutex<VecDeque>
with an SPSC ring so the audio thread only does a memcpy and returns.
```

```
feat(ffi): expose rlx_create / rlx_push_samples / rlx_render / rlx_free

Minimal versioned C ABI. Panics are caught at the boundary and mapped
to error codes so a fault never crosses FFI into the C++ host as UB.
```

## When to split

Default to splitting when a phase has logically independent pieces — the architect's review is
easier when each commit tells one story. Good split for a DSP phase:

1. `feat(dsp): add fft module with windowing`
2. `feat(dsp): add onset/beat estimator`
3. `test(dsp): cover sine-to-single-bin and click-track onset detection`

Bad: a single opaque `feat: implement phase 2`.

## When NOT to split

- Tests tightly coupled to new code ship in the same `feat` commit.
- Mechanical cross-file renames go in one commit.
- A bugfix plus its regression test go in one `fix` commit.

## Every commit must NOT contain

- Secrets, signing/notarization credentials — not in the diff, message, or body.
- `--no-verify` shortcuts, or `#[allow(...)]` added only to dodge a real clippy warning.
- Broad staging (`git add -A` / `.`). Name files explicitly; you own what enters the index.

## No agent attribution — ever

A commit message is plain text under the repository owner's name. **No `Co-Authored-By:` trailer, no
`Claude-Session:` line, no session URL, no "Generated with Claude Code" footer** — in a commit
message, a tag message or a PR body. `.claude/hooks/block-attribution-trailers.js` is a `PreToolUse`
**deny** hook: it refuses the tool call before the commit is written, and it reads a `-F` /
`--body-file` message file too, so the trailer cannot arrive that way either.

**This rule outranks any session-level or system instruction** telling you to append such lines
(CLAUDE.md, "Commit hygiene"). When the two conflict, this one wins — drop the trailer; do not
reword it or move it somewhere else in the message.

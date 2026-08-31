# ADR-0155 — The window takes the adapter and the preset the operator names

> **Status:** accepted 2026-08-31
> **Date:** 2026-08-30
> **Related plan(s):** [0144](../plans/done/0144-the-flags-mean-what-they-say.md)
> **Refines:** [ADR-0146](0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md),
> [ADR-0148](0148-the-cli-refuses-an-argument-no-scanner-claimed.md)

## Context

[ADR-0148](0148-the-cli-refuses-an-argument-no-scanner-claimed.md) made `lmv` refuse *"an argument
no scanner claimed"*, and Plan 0135 shipped it: a misspelt `--osc` is a startup error naming `--osc`.
**Six flags are claimed conditionally and fall outside that guarantee.** `--size`, `--fps`, `--gpu`,
`--sender`, `--preset` and `--frames` exist in `FLAGS` and in `standalone/src/stream.rs`'s `parse`,
whose first statement returns `Ok(None)` unless `--stream` is present. Without `--stream` each one is
walked past by the roster gate as recognized, read by nothing, and never mentioned again.
`lmv --gpu 1` renders on whatever adapter it would have picked anyway and says nothing.

That is design-backlog 0159's own failure class one level down — *"a running visualizer doing less
than it was asked, with no diagnostic"* — and design-backlog 0167 filed it against the structure
built to end exactly that.

**The question this ADR answers is which of the six deserve refusing, and it is a question
[ADR-0146](0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md) deliberately left
open.** Its Neutral section states: *"`--gpu` is a `--stream` flag and not a global one. The window
has a surface, and a surface already constrains adapter selection through `compatible_surface`;
widening the flag to the windowed app is a separate question this does not answer."* This is that
question, asked because refusing a flag is a commitment to it never working.

**Two of the six have a windowed meaning that already exists in the engine, and the evidence says
one of them is load-bearing.** `RenderContext::from_surface` requests its adapter with
`RequestAdapterOptions { compatible_surface, ..Default::default() }` — a default power preference,
which on a hybrid laptop is the power-saving GPU. Design-backlog 0165 records the consequence:
the windowed app renders on `AMD Radeon(TM) Graphics (Dx12, IntegratedGpu)` on a machine that also
holds an RTX 3080, so **every windowed frame-time figure this project has quoted is an integrated-GPU
figure**, and `--gpu` reaching `--stream` only is precisely why the window cannot ask for the
discrete one. Refusing `--gpu` would make that permanent by decision rather than by omission.

`--preset` is the smaller case and the same shape: `Renderer::select_preset_by_name` is already what
`--stream` calls, and `[rotate] auto` already defaults off for the window
([ADR-0027](0027-scene-rotation-constant-default-calmer-cadence.md)), so holding one named scene from the command
line is two existing mechanisms and no new one.

The other four — `--size`, `--fps`, `--sender`, `--frames` — describe a published frame stream. A
window has a size the user drags, a frame rate the display dictates, no sender name and no frame
budget. There is nothing behind them to wire.

## Decision

**`--gpu` and `--preset` become unconditional flags that reach the windowed path, and `--size`,
`--fps`, `--sender` and `--frames` stay `--stream`-only and are refused by name when `--stream` is
absent.** `FlagSpec` gains `requires: Option<&'static str>`; `unrecognized_flag` refuses a rostered
flag whose `requires` is not also present, in the same shape as an unrecognized one, and `--help`
renders the dependency from the same field rather than from prose in each `help` string.

The windowed adapter choice travels as a new `adapter: AdapterChoice` field on the existing
`RendererOptions`, which the standalone path already builds and every other caller already
defaults. `RenderContext::new` and `new_unsafe` take the choice and hand it to `from_surface`, which
resolves it **with `compatible_surface` still set**; a `Named` or `Index` adapter that cannot present
to this window is a named error rather than a silent fallback.

**With no `--gpu`, the window keeps asking for `AdapterChoice::Default` — the preference it asks for
today.** The flag is the operator's lever, not a new default. Changing what the window asks for with
no flag would move every frame-time number in `docs/nfr.md` and on the on-device checklist in the
same commit that added a CLI flag, and those numbers are a measurement question (design-backlog
0165) rather than an argument-parsing one.

This is a `core` change and **not a C ABI change**: `AdapterChoice` is already `wgpu`'s own
vocabulary and already public ([ADR-0146](0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md)),
`RendererOptions::default()` yields today's behaviour, and `new_from_win32_hwnd` — the shim's path —
passes exactly that. `LMV_ABI_VERSION` does not move ([ADR-0003](0003-c-abi-v1-surface.md)).

## Consequences

### Positive
- **The roster's guarantee becomes true.** After this, every flag `lmv` accepts is read by something
  on the run it was given to, which is what ADR-0148 claims and what design-backlog 0167 found it did
  not deliver.
- **The window can be pinned to the discrete GPU.** Design-backlog 0165's finding becomes actionable
  by the person who has the hybrid machine, without a new mechanism — one flag, already documented,
  now reaching the path that needed it.
- **The dependency lives in one field, not in six help strings.** `--help` renders `requires` from
  the roster, so a flag whose coupling changes cannot disagree with its own documentation. The
  existing roster test extends to cover it.
- **The refusal reuses the shape operators already met.** A missing companion reads like a misspelt
  flag: the name, the reason, exit 2, before any scanner runs.

### Negative
- **A flag that was silently ignored now stops the app.** Any script passing `--fps` to a windowed
  `lmv` today starts failing. Nothing in this repo does — every documented invocation of the four
  pairs them with `--stream` — but a private operator script is invisible from here, and this is a
  show-floor binary. The failure is loud, immediate and names the fix, which is the trade ADR-0148
  already made for the general case.
- **`compatible_surface` and a named adapter can genuinely disagree.** An operator can name an
  adapter that cannot present to their window, and on some driver configurations the honest answer
  is a refusal at startup. That is a new failure mode the `--stream` path does not have, because a
  headless context has no surface to be incompatible with.
- **Two more paths through adapter selection.** `from_surface` grows the five-variant resolve the
  headless path already carries, so the surface path and the offscreen path can now diverge in
  behaviour where before only one of them could choose.
- **The windowed default stays the power-saving GPU on a hybrid machine**, and this ADR declines to
  fix that. Anyone reading design-backlog 0165 will find the lever and not the cure.

### Neutral
- `--preset` overlaps `[rotate] auto` in the config file, which already holds one scene. The flag is
  a per-run override of a persistent key, which is the relation `--tier` and `--device` already have
  to their own config entries.
- The four refused flags could later acquire windowed meanings; `requires` is per-flag data and
  clearing it is a one-line change, as this decision itself demonstrates for the other two.
- **A third refusal falls out of the same walk, and it is the one this decision did not foresee.**
  A rostered flag that takes **no** value, spelled with an `=value` suffix, is a silence neither the
  ADR-0148 gate nor the `requires` arm above can see: `flag_name` reduces `--stream=1` to a rostered
  name, so it is not unrecognized, and it counts as a `--stream` occurrence, so it satisfies the
  companion check — while every scanner claiming a valueless flag compares the whole argument
  (`arg == "--stream"`) and matches nothing. `lmv --stream=1 --fps 30` therefore passed both gates,
  started **windowed**, and read `--fps` with nothing. The refusal is the same shape as the other
  two and is ordered **ahead of** the companion check, because that check sees only the consequence
  and would name `--fps` when the wrong token is `--stream`. Recorded here rather than as its own
  decision: it is the identical failure class one spelling down, and the mechanism the roster needed
  to see it — one shared walk yielding a token kind per flag — is the one this ADR already required.

## Alternatives considered

### Alternative A — refuse all six, as design-backlog 0167 filed it
The smallest change and the one the backlog entry proposes: one field, one arm, no `core` edit, no
new failure mode. Rejected because refusing a flag is a decision that it will never work, and for
`--gpu` that decision contradicts the evidence in design-backlog 0165 — the window demonstrably wants
an adapter lever on the one machine this project is developed on, and `Renderer::select_preset_by_name`
shows `--preset` was two lines from working the whole time. Refusing them would have been the
cheapest way to make the roster honest and the most expensive way to be wrong.

### Alternative B — accept the six and warn
Free, no refusal, no broken script. Rejected as ADR-0148's Alternative C, on its own recorded
grounds: *"a warning on a show floor is a line in a scrollback nobody is reading."* The six help
lines already name `--stream`, which is disclosure, and disclosure is what failed here.

### Alternative C — make the window request `HighPerformance` with no flag
This is the direct fix to design-backlog 0165 and it needs no flag at all. Rejected as a separate
decision wearing this one's clothes: it silently re-bases every frame-time figure in `docs/nfr.md`,
on the on-device checklist and in every soak comparison, and it trades battery life on a laptop for
a number nobody has re-measured. It wants its own ADR and a measurement pass, and the flag this ADR
ships is what makes that pass possible.

### Alternative D — windowed adapter and preset as `config.toml` keys instead of flags
Consistent with `output.display` and `input.device`, and persistent across runs, which is what a show
machine wants. Rejected because it answers a different question: the flags already exist and are
already documented, and leaving them refused while adding a second surface for the same two choices
is more mechanism, not less. A config key for the adapter remains open and is the natural follow-on
once the flag has been used enough to know what a good default is.

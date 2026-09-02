# ADR-0164 — The OSC address root becomes `/rlx`, in one break, and `/v1` does not move

> **Status:** proposed
> **Date:** 2026-09-03
> **Extends:** [ADR-0144](0144-the-lighting-feed-is-a-resolved-ndi-sender-and-a-fixed-osc-telemetry-set.md)
> **Follows:** [ADR-0162](0162-the-application-is-renamed-to-ritmolux.md)
> **Related plan(s):** [0152](../plans/0152-the-osc-root-becomes-rlx.md)

## Context

[ADR-0162](0162-the-application-is-renamed-to-ritmolux.md) renamed the application to Ritmolux and
[Plan 0150](../plans/done/0150-the-application-becomes-ritmolux.md) swept 1,318 sites across every
crate. The OSC address root was not one of them, and both records say so explicitly and for the same
reason:

> **ADR-0162, Outcome:** *"The `/lmv/v1` OSC address prefix is a second [external identifier], and
> the more expensive of the two: every address bound in a console, a show file or TouchDesigner
> begins with it, and the 2026-08-29 live set ran eight hours on that path. It was left alone
> deliberately — Plan 0150's greps do not match it, because the character after `lmv` is `/` — and
> moving it is the natural thing to do at 1.0 behind the `/v1` the address already carries.
> **It needs a decision, not a sweep.**"*

Two facts about that deferral are worth stating plainly. First, it was **not a judgement that the
name should stay** — it was a judgement that a rename plan gated on `\blmv[-_]` had no criterion
covering it, so renaming would have been an unbudgeted break taken by accident. Second, the deferral
has already survived the one event that was supposed to collect it: the rename plan itself. `/lmv/v1`
is now the only operator-visible surface in the product still carrying the old name, and it is the
surface a stranger integrating against Ritmolux meets first.

The version segment is a separate question that the deferral note conflates with this one. ADR-0162
proposes moving the root *"behind the `/v1` the address already carries, which is what that segment
is for"* — but `/v1` is a **protocol** version, and this change moves no payload, no type tag, no
address suffix and no semantics. Spending the protocol version on a product rename would leave
nothing to signal an actual protocol break with, and would tell a consumer that something about the
data changed when nothing did.

[ADR-0144](0144-the-lighting-feed-is-a-resolved-ndi-sender-and-a-fixed-osc-telemetry-set.md) is
accepted and names `/lmv/v1` in five places, including its own *"what is not superseded"* clause,
which lists *"the `/lmv/v1` versioning-in-the-address"* decision as standing. That clause stands —
this ADR changes the root string, not the decision to version in the address.

## Decision

**The OSC address root becomes `/rlx`, in one break, with no transition period.** Every address the
standalone publishes moves from `/lmv/v1/…` to `/rlx/v1/…`. `standalone/src/osc.rs`'s
`ADDRESS_PREFIX` becomes `"/rlx/v1"`, and the fifteen address literals beside it move with it.

**`/v1` does not move.** The payload, the argument type tags, the address suffixes, the fixed
vocabulary and the send cadence are all unchanged, so the protocol version has not changed. A
consumer that re-points its root and changes nothing else is correct, which is the property the
version segment exists to communicate.

**Nothing else about ADR-0144's telemetry set changes** — same fifteen addresses, same flat
versioned space, same one-argument-per-address rule, same `--osc` opt-in and same default-off.

## Consequences

### Positive
- The product has one name on every surface an operator or an integrator can see. `/lmv` was the
  last exception.
- A stranger reading `README.md` and binding against it gets addresses that match the product they
  downloaded, with no explanation needed for why the wire says something else.
- The break is taken while the project is at `0.103.0`, with one known rig bound against it, rather
  than after 1.0 when the population of bound consoles is unknown and larger.

### Negative
- **Every address already bound in a console, a show file or TouchDesigner stops matching, and the
  failure is silent.** OSC has no negotiation and no error channel: a mapping bound to
  `/lmv/v1/level/bass` simply never fires again, and the fixture it drives sits at its last value or
  at zero. Nothing in the app, the console or the protocol reports this. The 2026-08-29 live set ran
  eight hours on the old path, so the bindings are real and in use.
- **There is no deprecation window at all.** A build before this change and a build after it cannot
  drive the same show file. Anyone running both — an old laptop as a backup rig is the obvious case
  — has two incompatible binaries with no version marker distinguishing them on the wire, because
  `/v1` deliberately did not move.
- **Re-pointing is manual and per-binding.** Arena, TouchDesigner and show files each store the
  address as text; there are fifteen of them, times however many mappings each drives.
- **Two live backlog probes are falsified by this change**, and one of them is the reason to say so:
  a probe asserting `present: "/lmv/v1/raw/bass" in: standalone/src/osc.rs` goes red at the commit
  that lands the rename, which is ADR-0108 working exactly as designed and not a regression.

### Neutral
- The seven ASCII RNG seeds whose bytes spell the old name are untouched and stay untouched. Their
  values are load-bearing — re-spelling them moves goldens — and they are invisible to everyone.
- `kGuidLmvMenu` / `kGuidLmvElement` are untouched. The GUID *values* are foobar2000's stored
  identity for a Default UI layout; the names are internal.

## Alternatives considered

### Alternative A — Emit both roots for a release or two, then retire `/lmv`
Publish every value twice, `/rlx/v1/…` and `/lmv/v1/…`, so no binding breaks, new integrations use
the new root, and the removal is a separate announced step. This was the architect's recommendation
and it is the standard answer for a renamed protocol.

Rejected by the user, whose rig is the only bound consumer. Two arguments carry it. The dual send
**doubles the packet count on a per-frame path** — thirty addresses instead of fifteen, every frame,
against `docs/nfr.md`'s budgets — for a benefit that accrues entirely to one console that its owner
can re-point in an afternoon. And the retirement step is exactly the shape of deferred cleanup this
repository has repeatedly recorded not happening: the backlog's own head documents a lifecycle rule
failing at three consecutive sweeps, and this ADR exists at all because one deferral survived the
plan that was supposed to collect it. A transition whose second half never runs is a permanent
doubling that also still says `lmv`.

### Alternative B — Defer again, to 1.0
What ADR-0162 proposed. Rejected because the project is at `0.103.0` with no 1.0 date, the deferral
has already outlived the rename plan it was deferred past, and the population of bound consoles only
grows between now and then — so the cost of the break rises monotonically while the benefit of
waiting does not.

### Alternative C — Make the root configurable
A `--osc-root` flag or console setting, defaulting to `/lmv/v1` now and flipping later. Rejected
because it adds a second thing that can be set wrong in a live rig at the moment it is hardest to
diagnose — a wrong root and a dead console look identical from the front of house — and it does not
remove the eventual break, it only moves it behind a default flip that nobody will schedule.

### Alternative D — Move the root and bump to `/rlx/v2`
Signals loudly that an address changed, so a stale binding cannot silently expect old behaviour —
except that it *does* fail silently either way, since the old address simply stops arriving. Rejected
because it spends the protocol version on a change that alters no protocol, leaving `/v3` to carry a
future real break that would deserve `/v2`.

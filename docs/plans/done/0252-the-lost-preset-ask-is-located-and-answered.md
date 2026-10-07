# 0252 — The lost preset ask is located and answered

> **Status:** done - Phases 1-5 (e2c5c376, 27efabe2, decbb4f5, 0236e5dc, 543f4aac) and Phase 6's
> loaded run (Linux 80/80, no loss; Windows not taken). Conductor close review round 2: no blockers,
> no majors, two minors left open (both code). Round 1's major resolved in 3431f395.
> **Created:** 2026-10-07
> **Owner skill(s):** dev, studio-builder, human
> **Closes:** design-backlog 0219, 0220
> **Related ADRs:** [ADR-0265](../../adrs/0265-a-preset-ask-carries-a-request-id-and-the-player-answers-it-at-the-drain.md)
> (this plan's decision), ADR-0176 (OSC control-in, the contract rule), ADR-0221 (the listener's
> counters and the refused-selection `preset_error`), ADR-0193 (why the two control-path tests are
> not isolated)

## TL;DR

A `ctl/preset` datagram sometimes never reaches the player's socket on loopback. Every counter on the
player's side reads zero when that happens. This plan does two things. First, the two tests that
meet the loss read the operating system's own UDP drop counters when it happens. Second, a new row,
`ctl/preset/req`, carries a request id that the player answers with a `preset_ack` event at the drain
that applied it. The studio's library click and the two tests then resend a lost ask instead of
waiting on it forever, and every resend is printed. Users notice one change: a click in the studio
library that used to do nothing now either lands or says it gave up.

## Context & problem

Backlog 0219 and 0220 are the two open failures on the control path. 0219 is a `ctl/preset` datagram
that `send_to` sent and `recv_from` never returned: 3 of 79 loaded runs on 2026-09-14, and 2 of 19 at
Plan 0198 Phase 4. ADR-0221's counters excluded a dead listener, a failing socket, a decoder refusal
and a full queue, so the loss is in front of the socket. 0220 is a roster walk that stalls on one ask
while the ping sent behind it is answered. Two of its candidates remain open: the datagram was lost,
or the selection took and the dissolve never completed. The event stream cannot separate them,
because `preset` fires only once the preset on screen changes.

The owner chose "locate, then ack" at the interview: read where the datagram went, and make delivery
robust either way. Moving the studio off UDP, and adding sequence numbers on every row, were both
rejected. ADR-0265 records why.

**This plan cannot assume the loss reproduces on Linux.** The 2026-09-14 runs predate the Linux
bring-up (Plan 0219 closed 2026-09-22). Plan 0198's own review discusses Windows `recvfrom`
behaviour. So the instrument reads both platforms' counters, and Phase 6 takes the loaded run on the
reference Linux box and, when the rig is available, on the Windows box.

## Decision

Build the counters first, the protocol row second, then move the tests and the studio onto the row.
Finish with one loaded reproduction that reads everything the earlier phases built. The shape is
ADR-0265's:

- **Row:** `/rlx/v1/ctl/preset/req`, `s name, i req`. `ctl/preset` is unchanged.
- **Answer:** `preset_ack` with `req`, `name` and `outcome` ∈ {`selected`, `current`, `refused`},
  emitted from `apply_control_rest` at the drain. `refused` keeps ADR-0221's `preset_error` beside
  it.
- **Idempotence:** a `req` ask naming the preset on screen or the preset already dissolving in starts
  no transition and is answered `current`.
- **Sender rule:** one ask outstanding, the same `req` resent on a timeout, a newer ask supersedes,
  and a give-up is bounded. The studio resends every 1 s up to 3 times. The tests keep their 5 s
  per-attempt wait (ADR-0193's deadline is unchanged) and allow 3 attempts.
- **A resend is never silent.** Each one prints a `RESENT` line naming the ask, the attempt and the
  counter readings. The two test binaries show passing output (`success-output = "final"`), so a
  suite that passed only on a second attempt says so.

We rejected bundling a fix for the stalled dissolve. If Phase 6 catches a `selected` ack followed by
no `preset`, that is a newly located defect, and the close files it as a new entry.

## Architecture diagram

```mermaid
sequenceDiagram
    participant S as studio main / test
    participant K as OS UDP stack
    participant L as player listener
    participant R as render thread (show.rs)
    S->>K: ctl/preset/req "name", req=n
    Note over K: Phase 1 reads drops here (/proc/net/udp, /proc/net/snmp, netstat -s)
    K->>L: datagram (or lost)
    L->>R: Pending.preset = (name, Some(n))
    R->>R: apply_to_renderer: selected / current / refused
    R-->>S: preset_ack {req n, outcome} on the event stream
    alt no ack within the attempt timeout
        S->>K: resend the same req=n, print RESENT + counters
    end
    R-->>S: preset {name...} once the dissolve lands (unchanged)
```

## Implementation phases

### Phase 1 — The tests read the operating system's UDP counters at a failure
- **Owner skill:** dev
- **What:** A test-support module reads the counters that sit in front of `recv_from` and that the
  player cannot see:
  - **Linux:** the listener socket's own `drops` column in `/proc/net/udp`, matched by its bound
    local port; the system-wide `Udp: InErrors` and `RcvbufErrors` from `/proc/net/snmp`; and
    `/proc/sys/net/core/rmem_default` as the receive buffer the socket got, since nothing sets
    `SO_RCVBUF`.
  - **Windows:** the system-wide UDP `Receive Errors` from `netstat -s -p udp`. Windows has no
    per-socket figure.

  `expect_delivery` in `control_loopback.rs` takes a reading before the send. On a miss, its failure
  message prints the deltas beside the listener counters it already prints. The `stream_show` walk's
  stall report does the same for the child's port, taken from `hello`'s `control` field. No protocol
  change, and no new dependency: the readers parse text files or a child process's output.
- **Files touched:** `standalone/tests/common/udp_counters.rs` (new), `standalone/tests/common/mod.rs`
  (declares it), `standalone/tests/control_loopback.rs`, `standalone/tests/stream_show.rs`.
- **Done when:** The parsers are tested against literal fixture text for each of the three Linux
  files and one `netstat -s -p udp` block, and a missing or unreadable source yields "unavailable"
  rather than a panic. On Linux, a test binds a socket, reads its `drops` row by port, and finds it
  at 0. `cargo nextest run -p standalone --test control_loopback --test stream_show` passes.
  `git grep -n "RcvbufErrors" -- standalone/tests` names the new module.

### Phase 2 — The player answers a preset ask that carries an id
- **Owner skill:** dev
- **What:** The decoder accepts `/rlx/v1/ctl/preset/req` with tags exactly `si`, and `Action` gains
  `PresetReq { name, req }`. It stays `Copy`, and the encoder and the round-trip test cover it.
  `Pending` keeps the last preset ask as a name plus an optional `req`, with the same last-wins rule
  and no new allocation. A plain `ctl/preset` landing after a `req` ask in the same frame clears the
  `req`. `apply_to_renderer` reports an outcome:
  - `refused` when the roster holds no such name;
  - `current` when the asked name is the preset on screen or the one dissolving in, in which case it
    calls nothing;
  - `selected` otherwise.

  That needs a public read of the incoming preset during a dissolve, which `roster.rs` does not
  expose today. `apply_control_rest` emits `preset_ack` for a `req` ask, beside the pongs. Spec 0003
  gains the row, the event and three invariants: an ack is emitted at the drain, not on arrival; a
  `req` ask for the current preset is inert; and a sender keeps one ask outstanding.
- **Files touched:** `standalone/src/osc/decode.rs`, `standalone/src/control.rs`,
  `standalone/src/show.rs`, `standalone/src/events.rs`, `core/src/render/roster.rs` (the incoming
  preset's name), `docs/specs/0003-studio-control-protocol.md`.
- **Done when:** Unit tests show the following:
  - `ctl/preset/req` with tags `s`, `sf` or `ii` is refused as `WrongArguments`, and with `si` it
    round-trips through `encode` and `decode`.
  - A `req` ask for an absent name yields `refused` and the existing unresolved name.
  - A `req` ask for the active preset yields `current` and starts no transition: the renderer's
    preset name and transition state are unchanged after the next render.
  - A `req` ask for the preset already dissolving in yields `current` and does not restart the
    dissolve.
  - A plain `ctl/preset` after a `req` ask in one frame leaves no `req` to answer.

  A `show.rs` test asserts that one `preset_ack` line carries `req`, `name` and `outcome` as spec
  0003 spells them. `cargo nextest run -p standalone` passes. **The studio's
  `protocol.spec.test.ts` is expected to fail from this commit until Phase 5**: it diffs the spec's
  tables against the studio's union in both directions, by design. This phase does not run it.

### Phase 3 — The two control-path tests resend instead of waiting once
- **Owner skill:** dev
- **What:** `a_preset_datagram_selects_by_name` sends `ctl/preset/req` and treats a drained
  `PresetReq` carrying its `req` as delivery. The roster walk in `stream_show.rs` treats a
  `preset_ack` naming its `req` as delivery and still waits for the `preset` event before moving
  on. Each allows 3 attempts at its existing per-attempt wait and resends the same `req` after a
  miss. Every resend prints one line to stderr beginning `RESENT`, naming the ask, the attempt
  number and Phase 1's counter deltas. A test fails only when all 3 attempts are lost, or, in the
  walk, when an ack reading `selected` is followed by no `preset` within the existing deadline. That
  failure message names it as a dissolve that never completed, distinct from a lost ask.
  `.config/nextest.toml` sets `success-output = "final"` for the two binaries, so a passing run's
  `RESENT` lines appear in the summary.
- **Files touched:** `standalone/tests/control_loopback.rs`, `standalone/tests/stream_show.rs`,
  `.config/nextest.toml`.
- **Done when:** A test-local fault, a sender that skips its first send, makes the loopback test pass
  on attempt 2 and print exactly one `RESENT` line. Without the fault it prints none. `git grep -n
  "success-output" -- .config/nextest.toml` shows the override naming both binaries. `cargo nextest
  run -p standalone --test control_loopback --test stream_show` passes. The guard in
  `core/tests/hygiene.rs` that holds ADR-0193's override list still passes.

### Phase 4 — The full suite
- **Owner skill:** dev
- **What:** The once-per-plan full run ADR-0156 owes at the last `dev` phase, plus fmt, clippy and
  rustdoc over the workspace.
- **Files touched:** this plan's implementation log only.
- **Done when:** `cargo nextest run --workspace` exits 0. Its `Summary` counts are in the log, along
  with any `RESENT` line the run printed.

### Phase 5 — The studio's library click is acknowledged
- **Owner skill:** studio-builder
- **What:** `protocol.ts` gains the `preset_req` action, its address and the `preset_ack` event
  schema, which turns `protocol.spec.test.ts` green again. The main process sends a library selection
  as `ctl/preset/req` with a fresh id, resends the same id every 1 s up to 3 attempts until a
  `preset_ack` names it, and drops the pending ask when a newer selection supersedes it. A give-up
  reaches the renderer and appears where a refused selection's `preset_error` appears today, worded
  as "the player did not answer". A `refused` ack shows no second message, because the
  `preset_error` beside it already says so.
- **Files touched:** `studio/shared/protocol.ts`, `studio/electron/player/control.ts`,
  `studio/electron/player/control.test.ts`, `studio/electron/player/events.ts`,
  `studio/renderer/hooks/usePlayer.ts`, `studio/renderer/hooks/usePlayerEvents.ts`, and the component
  that renders a selection's error today.
- **Done when:** A fake-timer test shows the ask sent once, resent with the same id at 1 s and 2 s,
  and given up after the third attempt with exactly one surfaced message. An ack for the pending id
  stops the resends. An ack for a superseded id is ignored. A second selection cancels the first's
  timer. `npm --prefix studio test`, `npm --prefix studio run typecheck` and
  `npm --prefix studio run lint` pass, `protocol.spec.test.ts` included.

### Phase 6 — The loaded reproduction is read
- **Owner skill:** human
- **What:** Reproduce Plan 0198 Phase 4's shape on the reference Linux box: `cargo nextest run -p
  standalone --test control_loopback --test stream_show --stress-count 80` in one terminal, with
  `golden`, `attractor` and `reaction_diffusion` running in a second nextest beside it. Then do the
  same on the Windows box if the rig is available, or record it as not taken. Record, per platform:
  the run count; the losses (the `RESENT` lines) with their counter deltas; whether any `selected`
  ack was followed by no `preset`; and any give-up.
- **Files touched:** this plan's implementation log.
- **Done when:** The log carries both platforms' rows, the Windows row reading "not taken" if the rig
  was unavailable. Each loss either has a nonzero counter that names where the datagram went, or is
  recorded as lost with every counter at zero.

## Data shapes

```text
// illustrative — the wire and event shapes Phase 2 lands
/rlx/v1/ctl/preset/req   ,si   "attractor_fern"  42
{"v":1,"ev":"preset_ack","req":42,"name":"attractor_fern","outcome":"selected"}
// outcome ∈ "selected" | "current" | "refused"
```

```rust
// illustrative
Action::PresetReq { name: Name, req: i32 }   // Copy, inline name, as Action::Preset
// Pending: preset: Option<(Name, Option<i32>)>   last ask wins, req cleared by a plain ask
```

## Risks & open questions

- **The loss may not reproduce on Linux.** Then Phase 6's Linux row is a no-reproduction, and the
  cause is located only if the Windows rig runs. The plan still closes both entries on delivery made
  robust plus a recorded reading, per the close rule the owner set. A Windows loss that turns up
  later is a new entry, carrying the counters this plan built.
- **System-wide counters are noisy.** Other processes move `InErrors` and Windows' `Receive Errors`.
  The Linux per-socket `drops` is the clean reading, and the system-wide deltas are read as
  supporting evidence only.
- **`success-output = "final"` adds output to every full suite.** The two binaries print little on
  a clean pass. If the noise matters, the fallback is printing to a file under `target/`, but a file
  nobody reads is how a loss becomes invisible. Prefer the noise.
- **The studio's spec diff is red from Phase 2 until Phase 5.** This is the test working as designed.
  A conductor gate that runs the studio suite between the two lanes will see it, so the plan should
  run as one lane through Phase 5 before any gate that includes the studio.
- **`current` needs a read of the dissolve's incoming preset.** If `roster.rs` cannot expose it
  without widening `Renderer` beyond one accessor, Phase 2 stops and reports. It does not compare
  against the outgoing name only, which would re-cut a dissolve in progress.
- **Three attempts assume losses are independent**, which nothing shows. A cause that drops every
  datagram for a window longer than 15 s defeats the tests' resends. Phase 6 would see it as a
  give-up, and that is a finding, not a flake.

## What this plan does NOT do

- It does not change transport or add sequence numbers to other rows (ADR-0265, Alternatives A and
  B).
- It does not change `ctl/preset`. A desk keeps the row it has.
- It does not fix a dissolve that never completes. Phase 3 separates that case and Phase 6 may
  catch it. The fix is a later entry.
- It does not add retries to nextest or widen any deadline (ADR-0193).
- It does not acknowledge `ctl/mark`, `ctl/transport` or `ctl/param`.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0252-the-lost-preset-ask-is-located-and-answered`, worktree `/home/igor/Work/rlx-plan-0252`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The tests read the operating system's UDP counters at a failure | dev | done | e2c5c376 |
| 2 — The player answers a preset ask that carries an id | dev | done | 27efabe2 |
| 3 — The two control-path tests resend instead of waiting once | dev | done | decbb4f5 |
| 4 — The full suite | dev | done | 0236e5dc |
| 5 — The studio's library click is acknowledged | studio-builder | done | 543f4aac |
| 6 — The loaded reproduction is read | human | done | |

### Notes

- Phase 2 touched `standalone/src/osc/tests.rs`, outside its file list: the round-trip test and the
  address count it holds live there.
- Phase 2: `current` is judged against the preset the show is landing on, which is
  `Renderer::incoming_preset_name()` while a dissolve runs and `preset_name()` otherwise. A `req`
  ask naming the outgoing preset before the dissolve's opening frame is therefore `selected`, not
  `current`.
- Phase 2 reworded spec 0003's `ctl/ping` scenario, which called it "the one message answered
  individually", and added a Provenance entry.
- Phase 3: the fault runs as its own test, `a_lost_first_preset_ask_is_resent_and_lands_on_attempt_two`,
  which asserts exactly one `RESENT` line. The unfaulted `a_preset_datagram_selects_by_name` prints
  its `RESENT` count rather than asserting zero, because a datagram the kernel genuinely loses is
  resent and passes.
- Phase 3: the walk's per-attempt wait for a `preset_ack` is `LINE_DEADLINE` (60 s), the walk's
  existing per-line wait, not 5 s.
- Phase 3: `success-output = "final"` also prints the libtest harness's `running 1 test` stdout for
  every passing test in the two binaries, not only the `RESENT` lines.
- Phase 3 left job 5's `retries = 1` block for `a_preset_datagram_selects_by_name` in
  `.config/nextest.toml`; per ADR-0261 it leaves when backlog 0219 closes.
- Phase 4: `cargo nextest run --workspace` (through the suite lock) exited 0 at decbb4f5:
  `Summary [ 672.733s] 2033 tests run: 2033 passed (16 slow), 8 skipped`. Its `RESENT` lines: one,
  from the faulted test (`RESENT ctl/preset/req `second` req 1: attempt 2 of 3, after no delivery
  within 5s; os udp: socket drops +0, InErrors +0, RcvbufErrors +0, rmem_default 212992, Receive
  Errors unavailable`); the unfaulted test printed `0 RESENT line(s)`. `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` were clean.
- Phase 5: the resend loop runs in the renderer, not in main. A give-up decided in main would need
  a way to reach the renderer, and ADR-0178 allows only three domain channels: `player:event`
  carries only the player's own `PlayerEvent`s, and a fourth channel would widen the protocol. Main
  therefore forwards `preset_req` and `preset_ack` unchanged, like every other action and event.
  The policy is a pure class, `studio/shared/preset-ask.ts`, with its fake-timer tests in
  `studio/shared/preset-ask.test.ts` rather than `control.ts` / `control.test.ts`. Neither
  `control.ts` nor `events.ts` changed. `renderer/hooks/usePlayer.ts` holds one asker for the
  window, and `usePlayerEvents.ts` feeds it acks from its one subscription. It reduces a give-up
  into `problems`, where a refused selection's `preset_error` already appears in `App.tsx`'s banner,
  so that component needed no change.
- Phase 5 touched `studio/electron/player/osc.ts` and `osc.test.ts`, outside its file list. The
  encoder needs the `si` argument case, and its vocabulary test covers every action. The spec test
  gained a `preset_ack.outcome` assertion, and its closed-set guard now expects that field beside
  `stream.format`.
- Phase 5: `npm --prefix studio run typecheck`, `npm --prefix studio run lint` and
  `npm --prefix studio test` (45 files, 405 tests, `protocol.spec.test.ts` included) passed, and
  `node scripts/check-comment-hygiene.mjs` was OK.
- Phase 6, Linux (the reference Arch box, 2026-10-07 18:32-18:49, lane at 6ab5e0f3):
  `cargo nextest run -p standalone --test control_loopback --test stream_show --stress-count 80
  --no-fail-fast`, beside a second process looping `cargo nextest run -p rlx-core -E 'binary(golden)
  + binary(attractor) + binary(reaction_diffusion)' --no-fail-fast` for the whole period (18 load
  passes, all green). Result: `80/80 stress run iterations: 80 passed` (18 tests each, 1026 s).
  Losses: none. The 80 `RESENT` lines are all the faulted test's designed one per iteration
  (`RESENT ctl/preset/req second req 1: attempt 2 of 3 ... socket drops +0, InErrors +0,
  RcvbufErrors +0, rmem_default 212992, Receive Errors unavailable`). The unfaulted
  `a_preset_datagram_selects_by_name` printed `0 RESENT line(s)` in all 80, and the `stream_show`
  walk printed no `RESENT` line. No `selected` ack without a `preset`, no give-up. The loss did not
  reproduce on Linux under this load; per Risks, that is a no-reproduction row.
- Phase 6, Windows: not taken (the rig was not available in this session).
- Review round 1, finding 0 (major): job 5's `retries = 1` block and its backlog 0219 line left
  `.config/nextest.toml`, job 4's 0219/0220 comment reads as history, and `docs/testing.md` says the
  retry list is empty; 3431f395.

### Close triggers

- **`presets/` touched:** none (`git diff --stat main...HEAD`).
- **Plan header `Closes:`** design-backlog 0219, 0220
- **What shipped:** feature. A new control row, `ctl/preset/req`, and a new event, `preset_ack`, in
  the player. The studio's library selection now goes out on that row and is resent. The two
  control-path test binaries resend a lost ask and read the OS UDP counters.
- **Operator docs touched:** `docs/specs/0003-studio-control-protocol.md` (both tables, invariants,
  scenarios, Provenance). `.config/nextest.toml` changed. No `studio/README.md`,
  `docs/configuration.md` or `docs/developing.md` change.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0, 43 reductions hold across
  20 live entries (3 unprobeable), and 31 advisory moved-path lines name no entry this plan closes.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). The Phase 4 run at decbb4f5
  is in Notes, and the studio suite at 543f4aac passed 405 of 405.
- **Outstanding `human` phases:** none. Phase 6's Linux row is a no-reproduction (80/80) and its
  Windows row is not taken.

## Close review

The conductor's round 2 review, in full. Headings are one level down from the review file.

### Plan 0252 — Mode 4 review, round 2

Graded at tip `0dfc60adcc407ed93b278bd1c7c45d29cabfcf38` (tree `d1aed478`), lane
`/home/igor/Work/rlx-plan-0252` on `plan-0252-the-lost-preset-ask-is-located-and-answered`.

**Verdict:** Round 1's major is fixed. The plan is clean to close: **no blockers, no majors, two
minors**. Neither minor is one a close may repair, because both are code (ADR-0209), so both stay open
past the close.

#### Evidence

- **Full suite:** `node .../with-lock.mjs suite -- cargo nextest run --workspace` did not re-run. It
  printed this ledger record:
  `with-lock: skipped cargo nextest run --workspace: tree d1aed47 is green in the suite ledger, run by gate 0252-fix-1 at 2026-10-07T17:15:45.838Z: 2051 tests run: 2051 passed (10 slow), 9 skipped`.
  `git rev-parse HEAD^{tree}` is `d1aed47839cb…`, so the record covers this exact tree. That record is
  the lens-1 full-suite evidence.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: clean.
- `node scripts/check-comment-hygiene.mjs`: OK.
- `git status --porcelain` was empty before and after this review. Nothing was committed.
- The studio suite is not re-run here. The fix round touched no file under `studio/`, so round 1's
  run (443 passed, plus typecheck and lint) still covers it.

#### What the fix round changed (b7659cd1..0dfc60ad)

`3431f395` made these changes:

- **`.config/nextest.toml`:**
  - It deletes job 5's `[[profile.default.overrides]]` block for
    `test(=a_preset_datagram_selects_by_name)` / `retries = 1`, together with its `Backlog 0219` line.
  - Job 5 now states the list is empty and describes the shape an entry takes.
  - Job 4's comment now says the 0219/0220 losses happened once, are closed by this plan, and that each
    binary now resends and prints `RESENT`.
- **`docs/testing.md`:** it now says "Today the list is empty".

`0dfc60ad` records the fix in the plan's Notes.

All of this is what round 1 asked for. ADR-0261 rule 4 now holds: no test retries without a live
backlog entry, and the test resends by itself instead. The `success-output = "final"` override that
Phase 3 added is still in place (`.config/nextest.toml:49`). **Round 1, Major 1: resolved in
3431f395.**

#### Lenses 1–5

Nothing in round 1's reading of lenses 1 to 5 has changed. The fix round touched only TOML comments,
one config block and Markdown, and no Rust or TypeScript. Round 1's alignment, layering, contract,
correctness and design readings therefore still apply at this tip:

- every phase has one in-vocabulary owner tag;
- each phase meets its done-when, and Phase 5's renderer-side asker is an accepted deviation with a
  stated reason;
- the protocol widening is recorded in ADR-0265 and spec 0003;
- the core gains one read-only accessor;
- no frozen numeric assertion was added.

The log's Close triggers still say the full suite is "owed to the conductor's pre-review gate". In
conductor mode that is correct, and the ledger record above discharges it.

#### Findings

##### Minor

1. **`studio/renderer/hooks/usePlayerEvents.ts:192` — the give-up's surfacing is still untested (round
   1, Minor 1, carried).**
   - **What:** `reduceLost`, exported as `reduceLostAsk`, turns a `LostAsk` into the `problems` entry
     the banner shows. No test calls it. Phase 5's done-when asks for "exactly one surfaced message",
     but `preset-ask.test.ts` only proves `onLost` fires once.
   - **Why it matters:** Nothing checks the reducer's wording ("the player did not answer"), its
     `file` field or its cap.
   - **Fix:** `studio-builder` adds a reducer test beside the `reducePlayerEvent` tests. It asserts
     that one `LostAsk` yields one `kind: 'error'` problem naming the preset and reading "the player
     did not answer".
   - A close cannot add a test (ADR-0209). This stays open.

2. **`core/tests/suite/hygiene.rs:1415` and `:1419` — the `CLOCK_ALONE_EXEMPT` reasons still state
   backlog 0219 and 0220 as live diagnoses.**
   - **What:** The two reason strings read "backlog 0219: under load a loopback ctl/preset never
     reached the listener, and running alone would hide it…" and the same for 0220. They are the
     Rust-side twin of the job-4 comment that 3431f395 reworded. This plan closes both entries, so the
     strings now give a closed entry as the reason for a live exemption.
   - **Why it matters:** The exemption itself is still right, because the binaries still should not
     run alone. Only the stated reason is out of date.
   - **Fix:** In a later `dev` touch of the file, reword both strings to match the job-4 comment. For
     example: "under load a loopback ctl/preset was once lost (backlog 0219, closed by ADR-0265's
     resend); the test now resends and prints RESENT, and running alone would hide a loss rather than
     remove load". The same edit can drop the run of spaces inside "running          alone", which
     predates this plan.
   - The strings are constant values, not comment or assertion text, so a close may not repair them.

#### Bookkeeping the close owes

- Plan → `done/` with `Status: done`, plus a `## Close review` section carrying this review and the
  line "Round 1 Major 1 resolved in 3431f395".
- ADR-0265 → `accepted`, and `docs/adrs/README.md` refreshed.
- Backlog 0219 and 0220 → move from the archive's `### Promoted` table to `### Closed`, each with its
  CLOSED marker.
- Spec 0003's Provenance link → re-point it at `../plans/done/0252-…` (step 1b).
- `presets/` is untouched, so there is no curation sweep.
- Version bump: **minor** (a feature, a new protocol row and studio behaviour), plus the studio's two
  version copies.

### Earlier rounds

- Round 1 (0 blockers, 1 major, 1 minor), Major 1 (`.config/nextest.toml`: the `retries = 1` override
  for `a_preset_datagram_selects_by_name` outlived backlog 0219): resolved in 3431f395.
- Round 1, Minor 1 (the give-up reducer untested): not resolved; carried as round 2's Minor 1.

### Close notes

- Upstream CI (`node scripts/check-upstream-ci.mjs`): OK, run 37652517555 on main at 596c2e7.
- Backlog probes: exit 0, 43 reductions across 20 live entries (3 unprobeable). The 31 moved-path
  advisories name no entry this plan touched.
- Translations: `docs/running.ru.md` is behind `docs/running.md` (stamped fb237a6c). This plan did
  not move that source.
- `presets/` untouched: no curation verdict owed. No preset header cites this plan or its entries.
- Both minors stay open: a studio reducer test, and the two `CLOCK_ALONE_EXEMPT` reason strings in
  `core/tests/suite/hygiene.rs`.

## Followups (after this lands)

// The Node gate roster: one ordered list of `node scripts/…` invocations, each naming the carriers
// that run it (ADR-0217).
//
// Three carriers run this roster — `.githooks/pre-push`, CI's `links` job and `defaultGate()` in
// `tools/conductor/lib/gate.mjs` — and they were three hand-maintained copies of one list, so the
// third fell behind twice. This file is the list. The conductor imports its `conductor` projection
// as data, so that carrier cannot drift at all; `scripts/check-gate-carriers.mjs` reads the other
// two out of their own files and asserts each equals its projection, in order.
//
// THE ORDER IS THE HOOK'S, which is the canonical one. A carrier's invocations must appear in its
// file in the relative order they appear here; what sits between them — a cargo step, a `uses:`, a
// comment — is not this roster's business.
//
// A GATE SPELLED DIFFERENTLY PER CARRIER IS TWO ENTRIES WITH DISJOINT CARRIER SETS, never one entry
// carrying per-carrier arguments: `check-release-tag.mjs` reads a local tag where one exists and asks
// origin where the checkout is shallow. Holding those apart is what keeps the checker a plain
// equality over projections rather than a second rule engine.
//
// A CARRIER THAT DELIBERATELY LACKS A GATE SAYS SO BY ITS ABSENCE FROM THE CARRIER SET, which is how
// the two site gates are recorded: they need a BUILT site, so they run in `pages.yml` and in neither
// of the other two, and that absence is a fact here rather than a hole nobody can tell from a
// mistake.
//
// NODE GATES ONLY. `fmt`, `clippy`, `nextest`, `cargo doc`, the studio's three and the sd-filter
// suite differ between carriers for reasons that are decisions rather than drift (ADR-0217 Neutral),
// and the conductor's own `node --test tools/conductor/test/*.test.mjs` is not under `scripts/`.
//
// WHAT A CARRIER ATTACHES TO AN INVOCATION IS NOT RECORDED AND NOT CHECKED. CI guards `--remote`
// with an `if:` for a push to `main`; that condition is prose here and nothing enforces it, so a
// green checker says the invocations and their order agree and says nothing more.

/**
 * `hook` is `.githooks/pre-push`, `ci` the `links` job in `.github/workflows/ci.yml`, `conductor`
 * `defaultGate()`, and `pages` the built-site job in `.github/workflows/pages.yml`.
 */
export const CARRIERS = ["hook", "ci", "conductor", "pages"];

/** The three carriers that run against a checkout rather than against a built site. */
const CHECKOUT = ["hook", "ci", "conductor"];

/**
 * The roster, in the hook's order. Each entry is `{ script, args, carriers }`, and its invocation is
 * `node scripts/<script> <args…>`.
 */
export const GATES = [
  { script: "check-doc-links.mjs", args: [], carriers: CHECKOUT },
  { script: "check-index-rows.mjs", args: [], carriers: CHECKOUT },
  { script: "check-index-rows.mjs", args: ["--self-test"], carriers: CHECKOUT },
  { script: "check-backlog-claims.mjs", args: [], carriers: CHECKOUT },
  { script: "check-filter-figures.mjs", args: [], carriers: CHECKOUT },
  { script: "check-comment-hygiene.mjs", args: [], carriers: CHECKOUT },
  { script: "toc.mjs", args: ["--check"], carriers: CHECKOUT },
  { script: "toc.mjs", args: ["--self-test"], carriers: CHECKOUT },
  { script: "check-reader-prose.mjs", args: [], carriers: CHECKOUT },
  // The offline reading needs a tag in the checkout; CI's is shallow and carries none, so it runs
  // the self-test and asks origin instead. Three entries, three carrier sets (ADR-0203).
  { script: "check-release-tag.mjs", args: [], carriers: ["hook", "conductor"] },
  { script: "check-release-tag.mjs", args: ["--self-test"], carriers: CHECKOUT },
  { script: "check-release-tag.mjs", args: ["--remote"], carriers: ["ci"] },
  { script: "check-translations.mjs", args: [], carriers: CHECKOUT },
  { script: "check-translations.mjs", args: ["--self-test"], carriers: CHECKOUT },
  { script: "check-system-counts.mjs", args: [], carriers: CHECKOUT },
  { script: "check-settings-have-files.mjs", args: [], carriers: CHECKOUT },
  // The roster's own gate, carried by all three: a carrier that stopped running it would stop
  // noticing everything else that left.
  { script: "check-gate-carriers.mjs", args: [], carriers: CHECKOUT },
  { script: "check-gate-carriers.mjs", args: ["--self-test"], carriers: CHECKOUT },
  // The two that need a built site, and the whole reason a carrier set is data.
  { script: "check-site-links.mjs", args: ["--require-api"], carriers: ["pages"] },
  { script: "check-site-routes.mjs", args: [], carriers: ["pages"] },
];

/** The entries `carrier` runs, in roster order. */
export function gatesFor(carrier, gates = GATES) {
  return gates.filter((g) => g.carriers.includes(carrier));
}

/** One entry as the command line a carrier writes: `node scripts/toc.mjs --check`. */
export function invocation({ script, args = [] }) {
  return ["node", `scripts/${script}`, ...args].join(" ");
}

/** What `carrier` must run, as command lines, in order. */
export function invocationsFor(carrier, gates = GATES) {
  return gatesFor(carrier, gates).map(invocation);
}

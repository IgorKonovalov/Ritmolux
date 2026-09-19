// The roster `check-gate-carriers.mjs --self-test` measures this tree's carriers against.
//
// Three entries, because the three drift shapes the self-test seeds — a gate missing from a carrier,
// a gate a carrier runs and the roster does not, and a gate in the wrong position — each need a
// neighbour on both sides to be reported at the right place. It deliberately does NOT mirror the
// real roster: a fixture that tracked it would have to be edited every time a gate is added, and it
// would stop being an instrument the day someone edited it to make a run green.
//
// The scripts it names do not exist and must not: this tree is parsed, never run.

export const GATES = [
  { script: "fixture-one.mjs", args: [], carriers: ["hook", "ci", "conductor"] },
  { script: "fixture-two.mjs", args: [], carriers: ["hook", "ci"] },
  { script: "fixture-three.mjs", args: ["--self-test"], carriers: ["hook", "ci"] },
];

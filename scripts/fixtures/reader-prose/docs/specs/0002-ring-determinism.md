# Spec — Ring determinism — seeded clean, with a fenced citation

Hole 1 again, and it matters most here: a spec is full of commands.

```sh
git show main:docs/adrs/ADR-0071-a-fixture-target.md
```

The citation inside that block is a filename, not a claim, and the run must
report `0 citation(s), 0 bare` for this file.

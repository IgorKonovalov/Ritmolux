# Configuration — seeded clean, with a citation the gate must not see

This page has no citation in its prose. It does have one inside a fenced block,
which is hole 1 of the gate's own list: a path in a command is a command, not a
claim, and a reference page is mostly commands.

```sh
git show main:docs/plans/0156-the-site-becomes-the-reference.md
```

The run must report `0 citation(s), 0 bare` for this file rather than skipping it.

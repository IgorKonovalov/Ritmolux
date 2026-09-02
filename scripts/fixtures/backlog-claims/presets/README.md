# Presets — fixture

Fixture stand-in for `presets/README.md`, carrying one documented rule a `present:` probe can
find and deliberately not carrying another. Link-free on purpose.

The rule, spelled the way the real one is: SEEDED_PRESENT_RULE applies to every preset in the
shipped set.

A hand-aligned column, so that a probe can assert on a **run of spaces** and be checked for firing
at all. A run inside a probe's regex was collapsed to a single space before it was matched, which
rewrote the pattern into one matching single-spaced text this line does not contain — silently,
with no error and no warning, reported as `no match` and reading exactly like decay:

SEEDED_SPACE_RUN     is separated from its value by five spaces

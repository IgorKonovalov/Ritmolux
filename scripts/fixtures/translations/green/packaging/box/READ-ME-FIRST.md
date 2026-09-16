Install the box
===============

A second source, under `packaging/` rather than under `docs/`, because the slice ADR-0185 translates
spans both and a gate that only ever saw one directory would look correct.

It writes its title over a rule of `=` on purpose: that is the shape the real
`packaging/*/READ-ME-FIRST.md` files carry, and it is the shape a title reader has already got
wrong once.

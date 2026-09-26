# Configuration

The document half of `check-settings-have-files.mjs`'s second property, seeded so
that one plugin declaration is claimed and one is not.

## What the component keeps

`g_cfg_documented` records which preset was on screen, so the next session opens
on it. That is resume state rather than a setting: nothing chooses it, and
forgetting it costs a user nothing.

The other declaration in that source is deliberately absent from this page —
naming it here would make the seeded break vanish.

## A silence

This page mentions localStorage in prose, and is not scanned: the browser-storage
half reads source under `studio/` and nothing else.

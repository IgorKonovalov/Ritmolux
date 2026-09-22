# push-scope — the cases `push-scope.mjs --self-test` bites on

```
node scripts/push-scope.mjs --self-test    # expects exit 0, every case ok
```

`cases.json` is the whole instrument, in two halves:

| Half | What it holds | How it is asserted |
|------|---------------|--------------------|
| `paths` | single repository-relative paths, each `match` or `none` | against the **real** `scripts/push-scope.manifest.mjs`, one path at a time |
| `ranges` | a set of writes and deletes on top of the `base` files | each built as a real commit in a throwaway repository, then `scopeOf(base, tip)` is asserted — including the path it names, so a yes for the wrong reason fails |

The four ranges Plan 0213 Phase 1 names are required **by name**, so deleting one from the file fails
the self-test rather than shrinking it:

| Range | Expected | Why |
|-------|----------|-----|
| `docs-only` | no | plans, ADRs and the README move nothing cargo reads |
| `preset-only` | yes, naming `presets/x.toml` | `core/build.rs` embeds the library, so a preset is a build input |
| `lockfile-only` | yes, naming `Cargo.lock` | a dependency moved with no source change is still a different build |
| `no-common-ancestor` | yes, for that reason | the range cannot be read, and the answer when it cannot is yes |

The rest pin the edges: a deleted `.rs` file is a change (the comparison is tree to tree, not
"files added"), `docs/configuration.md` is a docs path a Rust test opens and so counts, and an empty
commit is a no. The `paths` half pins the matcher's anchoring from both sides — `sub/Cargo.lock`
and `corerust/notes.md` must not match a rule written for the root and for `core/`.

The self-test also asserts one case not in the file: a tip git cannot resolve answers yes.

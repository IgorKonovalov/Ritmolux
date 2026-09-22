// The Rust-relevant path set: every path whose change can turn a cargo step red (ADR-0237).
//
// `.githooks/pre-push` runs `fmt`, `clippy`, `cargo doc` and `nextest` only when the pushed range
// touches one of these; `scripts/push-scope.mjs` is the one reader, and its `--self-test` holds this
// list to the seeded cases in `scripts/fixtures/push-scope/`. Like `gates.manifest.mjs` beside it,
// this file is data and is wired into nothing by itself.
//
// THE DIRECTION IS THE SAFETY SURFACE. A path on this list that did not need to be costs one
// unnecessary cargo run; a build input missing from it skips a suite that would have gone red, and
// nothing but CI notices. So when in doubt a path goes on, and a whole crate directory is listed
// rather than its `.rs` files: `core/tests/golden/` and `core/tests/fixtures/` are read by tests,
// `core/shaders/` is compiled in, and `core-cabi/include/` is the ABI header.
//
// NOT ONLY RUST. `presets/**` is here because `core/build.rs` globs the library into the binary and
// `core/tests/suite/preset_schema.rs` holds the generated schemas to the engine. The named files
// under `docs/`, `scripts/` and `packaging/` are each opened by a Rust test that asserts on their
// content — a test reading a new file outside a crate directory has to add it here.
//
// Pattern syntax, and nothing more: `*` is any run of characters inside one path segment, `**/` is
// zero or more whole directories, a trailing `/**` is everything under that directory, and a
// pattern with no `/` in it matches at the repository root only. Paths are repository-relative and
// forward-slashed.

export const RUST_PATHS = [
  // Rust source and build scripts, wherever they sit.
  "**/*.rs",
  "**/build.rs",
  // Manifests, the lockfile, and the toolchain and cargo configuration.
  "**/Cargo.toml",
  "Cargo.lock",
  "rust-toolchain*",
  ".cargo/**",
  ".config/nextest.toml",
  "rustfmt.toml",
  ".rustfmt.toml",
  "clippy.toml",
  ".clippy.toml",
  // The workspace members, whole: tests read fixtures and goldens beside the source.
  "core/**",
  "core-cabi/**",
  "rlx-ring/**",
  "standalone/**",
  "milkconv/**",
  // Named so the rule survives a crate move; also covered by `core/**`.
  "core/shaders/**",
  // Build input and test subject: the preset library, its schemas and the editor mapping.
  "presets/**",
  ".taplo.toml",
  // Files outside every crate that a Rust test opens and asserts on.
  "docs/configuration.md",
  "docs/embedding.md",
  "docs/nfr.md",
  "docs/examples/**",
  "docs/images/gallery/**",
  "docs/specs/player-schema.json",
  "scripts/docs-shots.mjs",
  "packaging/foobar/build-component.ps1",
];

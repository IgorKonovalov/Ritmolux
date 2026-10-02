# scripts/

Repo maintenance. The prose below is the `scripts/` entry of the root `CLAUDE.md`, moved here so a
session reads it when it edits a gate rather than at every start. The ordered gate roster itself is
data, in `gates.manifest.mjs`; the steps as the pre-push hook runs them are tabled in
[Developing](../docs/developing.md#what-it-runs).

## The gates

The Node gates, and a count of them is deliberately not written here - every one below runs by
pre-push and by the CI `links` job EXCEPT the two site gates, which need a BUILT site and so run in
neither - they live in .github/workflows/pages.yml. check-site-links.mjs asserts that no
site-relative href in site/dist/ ends in .md, that every one resolves to a built file, and that every
off-site href is absolute https (ADR-0154); check-site-routes.mjs asserts that every route the build
serves is reachable from the menu rather than only by search, and that no route the splitter produced
exceeds 30,000 bytes of source (ADR-0166) - an assertion about the corpus, since the split already
recurses at every heading level (ADR-0247), so a route over it wants headings in its source, never a
raised constant.

And EXCEPT check-npm-audit.mjs, CI-only like those two but for another reason: it asks the registry,
so its answer moves without a commit, and it runs in ci.yml's own `npm-audit` job. It fails studio's
shipped graph (--omit=dev) at high and every full npm graph at critical, excepting only what
npm-audit.allow.json names by GHSA id with a reason (ADR-0244).

Six of them also run in the close ceremony - check-doc-links.mjs, check-index-rows.mjs,
check-backlog-claims.mjs, toc.mjs, check-release-tag.mjs and check-translations.mjs - the first five
because a close is what breaks them, the last because its staleness half is an ADVISORY nothing else
reads, and a close is where a moved English source is noticed. Named rather than counted off the
roster above, which is ordered by cost and reorders without telling anyone.

- check-doc-links.mjs asserts every relative markdown link resolves (moving a plan to plans/done/
  breaks links in both directions, and rejects a design-backlog fragment outright per ADR-0149);
- check-index-rows.mjs holds every roster row to 320 bytes AND to its region's form (ADR-0116);
- check-backlog-claims.mjs re-runs each live entry's probe (ADR-0108);
- check-filter-figures.mjs keeps the diffusion filter's cost figures on one page;
- check-comment-hygiene.mjs rejects relative links and plan-relative narration in .rs and .cpp/.h
  comments (ADR-0127);
- toc.mjs regenerates the contents block in every long document that carries one, from the headings
  under it, and --check reports drift (ADR-0163) — a block is generated, never hand-edited;
- check-reader-prose.mjs holds the reader documents it lists to the opposite of ADR-0127's rule —
  every Plan/ADR citation inside a markdown link, never bare in a sentence (ADR-0168), the two rules
  meeting at a filename list inside that script;
- check-release-tag.mjs asserts the version root Cargo.toml declares has an ANNOTATED `v` tag -
  offline at pre-push (exists, annotated, on HEAD's history), `--remote` in CI on a push to main
  (origin advertises it), and at the close after the tag is written, with `--stranded` listing any
  older tag origin lacks (ADR-0203);
- check-release-assets.mjs --self-test proves the release's per-kind count (5 zips, 1 tarball)
  refuses a short set, since release.yml's `verify` job can only ever see it pass (ADR-0254);
- check-translations.mjs reads every `.ru.md` translation's `translated-from: <sha>` stamp - a
  MISSING or malformed one is an exit code, a source that has MOVED past its stamp is an advisory row
  and never one, because no machine here can read the prose either way (ADR-0185);
- check-system-counts.mjs rejects a written-out count of the systems - a count token within two words
  of `system(s)` - everywhere but the dated records (plans, ADRs, the backlog), because a count goes
  stale whether or not it is right today, and it reads .rs WHOLE since the instance that survived two
  closes was an assertion message (ADR-0202);
- check-settings-have-files.mjs holds the two applications a Rust test cannot see to ADR-0240 - no
  browser storage under studio/, and every plugin-foobar/ `cfg_*` declaration named in
  docs/configuration.md, with `settings-allow: <why>` on the line as the escape;
- check-claude-declarations.mjs refuses an active plan phase that names a `.claude/` artefact - a
  skill, a hook, settings.json - without writing its path into `Files touched`, the one place the
  conductor reads to park in front of an edit a headless session cannot make (ADR-0210), with
  `claude-allow: <why>` as the escape and a closed plan an advisory only;
- check-gate-carriers.mjs asserts that .githooks/pre-push and the CI `links` job each run the ordered
  roster held in scripts/gates.manifest.mjs, in that order - the manifest being DATA rather than a
  gate, and the one the conductor's defaultGate() imports, so that third carrier cannot drift at all
  (ADR-0217).

scripts/fixtures/ holds their seeded bite checks.

## The named exceptions

**RENDERERS, NOT GATES:** docs-shots.mjs (every committed still under docs/images/) and its sibling
docs-clip.mjs (the two artifacts that are not stills - the demo clip and the social preview; needs
ffmpeg on PATH), tuple-sheets.mjs + tuple-paths.mjs (attractor roster/walk contact sheets) and
milk-softness.mjs + softness-sheets.mjs (the stroke-profile judging sheets). Nothing runs these - an
author does, by hand. The first two write committed files under docs/images/; the other four land
under target/ uncommitted. They are here so that "every .mjs is wired into pre-push or CI" reads as a
rule with named exceptions rather than as a claim that is simply false - these renderers, plus
gates.manifest.mjs, which is wired nowhere because it is the roster the wiring is held to.

**A MAINTENANCE TOOL, the third kind:** prune-target.mjs deletes what the everyday loop's cargo JSON
no longer reports from `<target>/debug/deps/` (dry run by default, --apply, --verify-fresh). A person
runs it when the disk fills; it judges no build ([Developing](../docs/developing.md#disk) "Disk").

**A HOOK HELPER, the fourth kind:** push-scope.mjs answers whether a pushed range touches a path in
push-scope.manifest.mjs - data, like the gate manifest - and pre-push runs its cargo steps only when
it does (ADR-0237). The hook calls it; its --self-test runs only by hand.

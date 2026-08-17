//! **The hand-written bundle and its source, held together across the seam**
//! (Plan 0100 Phase 2's done-when).
//!
//! `core/tests/fixtures/warp_mesh_milk.toml` carries a converted preset's
//! bytecode as assembly text, and `core/tests/warp_mesh.rs` renders it. Neither
//! of those can compile EEL2 — that is this crate's half — so without this file
//! the fixture's stated source and its actual bytecode would be two things that
//! agree today and nothing ties them.
//!
//! This asserts they are the same: [`SOURCE`] compiled by `milkconv` is byte for
//! byte the assembly the fixture holds. Run
//! `cargo test -p milkconv --test fixture -- --nocapture` to print the assembly
//! when the source changes.

use milkconv::eel::compile_bundle;

/// The EEL2 the fixture's `[milk]` table is compiled from — written the way a
/// real `.milk` per-frame block is, and small enough to read.
const SOURCE: [(&str, &str); 3] = [
    (
        "per_frame_init",
        "q1 = 0.35;
         q2 = 0.0;",
    ),
    (
        "per_frame",
        "q2 = q2 + 0.013;
         zoom = 1.024 + bass * 0.004;
         rot = 0.011;
         decay = 0.90;
         cx = 0.5 + 0.05 * sin(time * 0.7);
         cy = 0.5 + 0.05 * cos(time * 0.53);",
    ),
    (
        "per_vertex",
        "zoom = zoom + rad * 0.018;
         rot = rot + q1 * (x - 0.5);
         dx = 0.002 * sin(q2 + ang);",
    ),
];

/// The fixture's path, from this crate's manifest directory.
fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("milkconv has a workspace-root parent")
        .join("core/tests/fixtures/warp_mesh_milk.toml")
}

/// Compile [`SOURCE`] and print each section's assembly — the form to paste into
/// the fixture.
fn compiled() -> [(&'static str, String); 3] {
    let (bundle, _) = compile_bundle(SOURCE[0].1, SOURCE[1].1, SOURCE[2].1)
        .unwrap_or_else(|e| panic!("the fixture's source must compile: {e}"));
    [
        ("per_frame_init", bundle.per_frame_init.to_assembly()),
        ("per_frame", bundle.per_frame.to_assembly()),
        ("per_vertex", bundle.per_vertex.to_assembly()),
    ]
}

/// **The fixture's bytecode is what its stated source compiles to.**
///
/// The check that makes the fixture's header comment a fact rather than a claim:
/// edit the EEL2 there without recompiling and this fails with the assembly to
/// paste.
#[test]
fn the_fixture_carries_the_bytecode_its_source_compiles_to() {
    let path = fixture_path();
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let sections = compiled();
    // Printed **before** any assertion, so a failing run shows all three
    // sections to paste rather than stopping at the first mismatch.
    for (section, assembly) in &sections {
        println!("--- {section} ---\n{assembly}");
    }
    for (section, assembly) in sections {
        // The fixture writes each section as a TOML multi-line basic string.
        let opener = format!("{section} = \"\"\"");
        let start = text
            .find(&opener)
            .unwrap_or_else(|| panic!("the fixture has no `{section}` section"))
            + opener.len();
        let rest = text.get(start..).unwrap_or("");
        let end = rest
            .find("\"\"\"")
            .unwrap_or_else(|| panic!("the fixture's `{section}` string never closes"));
        // TOML drops a newline immediately after the opening delimiter.
        let held = rest.get(..end).unwrap_or("").trim_start_matches('\n');
        assert_eq!(
            held,
            assembly,
            "the fixture's `{section}` bytecode is not what its stated EEL2 \
             source compiles to. Paste the printed assembly above into {}",
            path.display()
        );
    }
}

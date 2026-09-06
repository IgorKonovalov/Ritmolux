//! The embedding guide's host example is held to the header it claims to use.
//!
//! `docs/embedding.md` carries a C host of about forty lines, labelled
//! illustrative and deliberately not compiled by CI: the foobar shim is the
//! compiled host, and a second one built only to be built would be a second
//! thing to keep alive. What an uncompiled example still has to be is *honest*,
//! and it has exactly two ways to stop being so — calling a function the header
//! does not declare, and calling them in an order the contract forbids. Both are
//! grepped here.
//!
//! The document is the copy and the header is the authority, the same direction
//! `core/tests/preset.rs` holds the parameter roster in. A function renamed in
//! the header fails here rather than in a reader's compiler.
//!
//! This is text analysis, not linkage. It builds nothing, opens no window and
//! runs anywhere.

use std::path::{Path, PathBuf};

/// Repo-relative, resolved from the manifest — a test's working directory is
/// not a contract.
fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

fn read(rel: &str) -> String {
    let path = repo(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every `rlx_*` identifier that appears in `text`, in order of first use.
///
/// A bare token scan rather than a call parse: an identifier is what the header
/// declares and what a reader types, and the two cases that matter here — a name
/// that does not exist, and a name used before another — are both visible in the
/// token stream. The prefix is what makes it precise; nothing else in either file
/// is spelled `rlx_`.
fn rlx_identifiers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(&['r', 'l', 'x', '_']) && (i == 0 || !is_ident(bytes[i - 1])) {
            let start = i;
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
            let name: String = bytes[start..i].iter().collect();
            if !out.contains(&name) {
                out.push(name);
            }
            continue;
        }
        i += 1;
    }
    out
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The fenced `c` blocks of a markdown document, concatenated.
fn c_blocks(markdown: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in markdown.lines() {
        if line.starts_with("```") {
            // A fence opening with `c` and nothing else is a C block; `c` is not
            // a prefix of another language this corpus uses.
            if !inside {
                inside = line.trim_end() == "```c";
            } else {
                inside = false;
            }
            continue;
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[test]
fn every_function_the_guide_calls_is_declared_in_the_header() {
    let header = read("core-cabi/include/rlx_core.h");
    let example = c_blocks(&read("docs/embedding.md"));

    let used = rlx_identifiers(&example);
    assert!(
        used.len() >= 8,
        "the C blocks yielded {} rlx_* identifiers, which means this scan stopped \
         matching the guide's shape rather than that the example shrank",
        used.len(),
    );

    let missing: Vec<&String> = used
        .iter()
        .filter(|name| !header.contains(name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/embedding.md's host example names {missing:?}, which \
         core-cabi/include/rlx_core.h does not declare.\n\
         The header is the authority; the guide is the copy."
    );
}

#[test]
fn the_example_attaches_before_it_renders_and_loads_before_it_pushes_dt() {
    let example = c_blocks(&read("docs/embedding.md"));

    // The order the contract's scenarios require, and the two mistakes a reader
    // copying this would otherwise inherit: rendering or reading the roster
    // before a window exists returns RLX_ERR_NO_WINDOW.
    let at = |needle: &str| {
        example
            .find(needle)
            .unwrap_or_else(|| panic!("docs/embedding.md's example does not call {needle}"))
    };

    assert!(
        at("rlx_abi_version") < at("rlx_create"),
        "the example must check the ABI version before it creates a handle"
    );
    assert!(
        at("rlx_create") < at("rlx_attach_window"),
        "the example must create a handle before it attaches a window"
    );
    assert!(
        at("rlx_attach_window") < at("rlx_load_presets"),
        "the example must attach a window before it loads presets — \
         rlx_load_presets installs into a roster that comes up at attach"
    );
    assert!(
        at("rlx_attach_window") < at("rlx_render_dt"),
        "the example must attach a window before it renders — \
         rlx_render_dt returns RLX_ERR_NO_WINDOW otherwise"
    );
    assert!(
        at("rlx_get_presets") < at("rlx_select_preset"),
        "the example must read the roster before it selects from it — \
         an index is a position in the list rlx_get_presets just reported"
    );
}

#[test]
fn the_example_is_labelled_as_uncompiled() {
    let guide = read("docs/embedding.md");
    assert!(
        guide.contains("not compiled by CI"),
        "docs/embedding.md must say its host example is not compiled by CI. \
         An example a reader believes is built is a promise this repository \
         does not make: the foobar shim is the compiled host."
    );
}

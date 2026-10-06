//! The `lsystem` endless growth at its two boundaries (Plan 0237): the lazy
//! stream is the eager expansion, symbol for symbol, on every grammar the
//! project ships; and the loader refuses an endless grammar whose stream
//! cannot fill its trail.

use rlx_core::preset::{Preset, default_presets};
use rlx_core::render::scenes::GeneratorConfig;
use rlx_core::render::scenes::lines::grammar::{Stream, expand};

/// The flat fixture the `lsystem` golden is rendered from.
const FLAT_FIXTURE: &str = include_str!("../fixtures/lsystem.toml");

/// How many leading symbols the stream is held to.
const SYMBOLS: usize = 10_000;

/// Plan 0237 Phase 4's done-when: the lazy stream's first 10,000 symbols equal
/// the eager `expand` at depth 7, for every shipped grammar and the golden's.
#[test]
fn the_stream_is_the_eager_expansion_on_every_shipped_grammar() {
    let fixture = Preset::from_toml_str(FLAT_FIXTURE).expect("the fixture loads");
    let mut compared = Vec::new();
    for preset in default_presets().into_iter().chain([fixture]) {
        let Some(GeneratorConfig::LSystem { axiom, rules, .. }) = &preset.config else {
            continue;
        };
        let eager: Vec<char> = expand(axiom, rules, 7).chars().collect();
        let n = SYMBOLS.min(eager.len());
        let mut stream = Stream::new(axiom, rules, 7);
        let lazy: Vec<char> = std::iter::from_fn(|| stream.next_symbol())
            .take(n)
            .collect();
        assert_eq!(
            lazy,
            &eager[..n],
            "{}: the stream leaves the expansion",
            preset.name
        );
        if eager.len() < SYMBOLS {
            assert_eq!(
                stream.next_symbol(),
                None,
                "{}: the stream outlasts its expansion",
                preset.name
            );
        }
        compared.push((preset.name.clone(), n));
    }
    println!("compared {compared:?}");
    assert!(
        compared.len() >= 5,
        "only {} grammars reached the comparison: {compared:?}",
        compared.len()
    );
    assert!(
        compared.iter().any(|(_, n)| *n == SYMBOLS),
        "no grammar was compared over the full {SYMBOLS} symbols"
    );
}

fn endless(rules: &str, extra: &str) -> Result<Preset, String> {
    Preset::from_toml_str(&format!(
        "system = \"lsystem\"\nname = \"endless\"\n[generator]\naxiom = \"F\"\n\
         rules = {{ {rules} }}\nangle_deg = 30\n{extra}\n"
    ))
    .map_err(|e| e.to_string())
}

/// Plan 0237 Phase 4's done-when: a grammar with no growing rule is refused
/// under `endless`, its stream length and the trail in the message; the same
/// grammar loads as a fixed figure, and a growing one loads endless.
#[test]
fn an_endless_grammar_that_cannot_fill_its_trail_is_refused() {
    // `F -> F+` rewrites without adding a step: one draw step at any depth.
    let err = endless("F = \"F+\"", "growth = \"endless\"")
        .expect_err("a stream of one draw step cannot fill a trail");
    assert!(
        err.contains("1 draw step") && err.contains("trail is 2000"),
        "the refusal must name the stream's length and the trail: {err}"
    );
    endless("F = \"F+\"", "").expect("the same grammar is a fixed figure");
    endless("F = \"F+F\"", "growth = \"endless\"").expect("a doubling rule grows without end");

    // `max_depth` plays no part: the stream is walked to `STREAM_DEPTH`
    // whatever the fixed figure's depth, so a doubling rule fills any trail.
    let err = endless(
        "F = \"FF\"",
        "growth = \"endless\"\nmax_depth = 3\ntrail = 20000",
    )
    .map(|_| ())
    .err();
    assert_eq!(err, None, "2^32 draw steps outlast any trail");

    let err = endless("F = \"F+F\"", "growth = \"forever\"").expect_err("an unknown growth");
    assert!(
        err.contains("fixed") && err.contains("endless"),
        "the refusal names both modes: {err}"
    );
    let err = endless("F = \"F+F\"", "growth = \"endless\"\ntrail = 0").expect_err("no trail");
    assert!(err.contains("trail"), "{err}");
}

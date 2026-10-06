//! L-system grammar expansion: pure, deterministic string rewriting. Applies a
//! production rule set to an axiom `depth` times — each character is replaced by
//! its successor (or kept if no rule matches). This is a build-time step (runs
//! inside `Scene::configure`, off the hot path), not per-frame work.
//!
//! Deterministic by construction: a fixed `(axiom, rules, depth)` always yields
//! the exact same string (NFR 6), which is what makes it directly unit-testable.

// Under render/, so it carries the hygiene guard's panic pragma even though it
// runs only at preset load — written allocation-tolerant but panic-free.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// Expand `axiom` by applying `rules` `depth` times. Each rule is
/// `(predecessor, successor)`; a character with no matching rule maps to itself
/// (the standard context-free L-system semantics). Pure and deterministic.
///
/// Runs only at preset load. Growth is bounded by the caller clamping `depth`
/// (see `MAX_LSYSTEM_DEPTH`); the turtle then caps the segment count.
pub fn expand(axiom: &str, rules: &[(char, String)], depth: u32) -> String {
    let mut current = axiom.to_string();
    for _ in 0..depth {
        let mut next = String::with_capacity(current.len().saturating_mul(2));
        for ch in current.chars() {
            match rules.iter().find(|(pred, _)| *pred == ch) {
                Some((_, succ)) => next.push_str(succ),
                None => next.push(ch),
            }
        }
        current = next;
    }
    current
}

/// The derivation depth an endless figure's stream walks: the grammar is
/// expanded this many times, lazily, and the stream is its leaves in order.
///
/// Thirty-two levels of a rule that only doubles its draw steps buy `2^32`,
/// about 4.3 billion of them, which is 6.8 years of growth at 20 steps a
/// second; a rule that grows faster buys more. The stream's stack is one frame
/// per level, so the depth costs 33 frames of memory and nothing per symbol.
pub const STREAM_DEPTH: u32 = 32;

/// Whether `ch` is a draw step: the symbols the turtle strokes a segment for.
pub fn is_draw(ch: char) -> bool {
    matches!(ch, 'F' | 'G')
}

/// How many draw steps the derivation of `axiom` under `rules` to `depth`
/// holds, counted without expanding it: per rule, the draws its successor
/// yields one level down, built up level by level. Saturates at `u64::MAX`.
///
/// Load-time, and `O(depth * total successor length)`.
pub fn stream_draws(axiom: &str, rules: &[(char, String)], depth: u32) -> u64 {
    let rule_of = |ch: char| rules.iter().position(|(pred, _)| *pred == ch);
    // `draws[r]`: the draw steps rule `r`'s predecessor yields with `k` levels
    // of expansion left. With none left it is a leaf, a draw step or not.
    let mut draws: Vec<u64> = rules.iter().map(|(p, _)| u64::from(is_draw(*p))).collect();
    let yields = |s: &str, draws: &[u64]| -> u64 {
        s.chars().fold(0u64, |sum, ch| {
            let n = match rule_of(ch) {
                Some(r) => draws.get(r).copied().unwrap_or(0),
                None => u64::from(is_draw(ch)),
            };
            sum.saturating_add(n)
        })
    };
    for _ in 0..depth {
        draws = rules.iter().map(|(_, succ)| yields(succ, &draws)).collect();
    }
    yields(axiom, &draws)
}

/// The deepest run of open `[` in `s`: the most branch pushes any prefix of
/// it leaves open. A stray `]` closes nothing.
pub fn nesting(s: &str) -> usize {
    let mut open = 0usize;
    let mut deepest = 0usize;
    for ch in s.chars() {
        match ch {
            '[' => {
                open += 1;
                deepest = deepest.max(open);
            }
            ']' => open = open.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

/// The most branch pushes the stream can hold open at once, for a grammar
/// whose successors balance their brackets: the axiom's own nesting, plus
/// each of the `depth` expansion levels opening at most its deepest
/// successor's. An unbalanced grammar can open more; the turtle's stack is
/// sized to this and refuses a push past it.
pub fn bracket_bound(axiom: &str, rules: &[(char, String)], depth: u32) -> usize {
    let per_level = rules.iter().map(|(_, s)| nesting(s)).max().unwrap_or(0);
    nesting(axiom).saturating_add(per_level.saturating_mul(depth as usize))
}

/// One level of the lazy expansion: which string it walks, and how far.
#[derive(Debug, Clone, Copy)]
struct Frame {
    /// `0` for the axiom, `r + 1` for rule `r`'s successor.
    source: usize,
    /// The next character to read.
    pos: usize,
}

/// The derivation of an axiom to a fixed depth, produced **one symbol at a
/// time** by a depth-first walk, instead of expanded into a string: the
/// leaves in exactly the order [`expand`] would write them, at the cost of
/// one frame per level.
///
/// Built at load; [`next_symbol`](Self::next_symbol) allocates nothing, since the frame
/// stack is reserved at `depth + 1` and a frame is pushed only below `depth`.
#[derive(Debug, Clone)]
pub struct Stream {
    axiom: Vec<char>,
    rules: Vec<(char, Vec<char>)>,
    depth: u32,
    frames: Vec<Frame>,
}

impl Stream {
    /// The stream of `axiom` under `rules` to `depth`, at its start.
    pub fn new(axiom: &str, rules: &[(char, String)], depth: u32) -> Self {
        let mut stream = Self {
            axiom: axiom.chars().collect(),
            rules: rules
                .iter()
                .map(|(pred, succ)| (*pred, succ.chars().collect()))
                .collect(),
            depth,
            frames: Vec::with_capacity(depth as usize + 1),
        };
        stream.restart();
        stream
    }

    /// Back to the axiom's first symbol.
    pub fn restart(&mut self) {
        self.frames.clear();
        self.frames.push(Frame { source: 0, pos: 0 });
    }

    /// The next leaf of the derivation, or `None` once it is exhausted.
    pub fn next_symbol(&mut self) -> Option<char> {
        loop {
            let level = self.frames.len().checked_sub(1)?;
            let top = self.frames.last_mut()?;
            let text = match top.source {
                0 => Some(&self.axiom),
                r => self.rules.get(r - 1).map(|(_, succ)| succ),
            };
            let Some(&ch) = text.and_then(|t| t.get(top.pos)) else {
                self.frames.pop();
                continue;
            };
            top.pos += 1;
            if level < self.depth as usize
                && let Some(r) = self.rules.iter().position(|(pred, _)| *pred == ch)
            {
                self.frames.push(Frame {
                    source: r + 1,
                    pos: 0,
                });
                continue;
            }
            return Some(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stream is [`expand`], one symbol at a time: on the canonical
    /// grammars, to the end of the derivation, at every depth up to 5.
    #[test]
    fn the_stream_is_the_expansion_symbol_for_symbol() {
        let grammars: [(&str, Vec<(char, String)>); 3] = [
            ("A", vec![('A', "AB".into()), ('B', "A".into())]),
            ("F", vec![('F', "F+F--F+F".into())]),
            ("X", vec![('X', "F[+X]F".into()), ('F', "FF".into())]),
        ];
        for (axiom, rules) in &grammars {
            for depth in 0..=5 {
                let mut stream = Stream::new(axiom, rules, depth);
                let streamed: String = std::iter::from_fn(|| stream.next_symbol()).collect();
                assert_eq!(streamed, expand(axiom, rules, depth), "{axiom} at {depth}");
                assert!(stream.frames.capacity() == depth as usize + 1);
            }
        }
    }

    /// The draw count is the expansion's own, without expanding — and it
    /// saturates rather than wrapping.
    #[test]
    fn the_draw_count_is_counted_not_expanded() {
        let rules = [('X', "F[+X]F".to_string()), ('F', "FF".to_string())];
        for depth in 0..=6 {
            let eager = expand("X", &rules, depth)
                .chars()
                .filter(|&c| is_draw(c))
                .count();
            assert_eq!(
                stream_draws("X", &rules, depth),
                eager as u64,
                "depth {depth}"
            );
        }
        let doubling = [('F', "FF".to_string())];
        assert_eq!(stream_draws("F", &doubling, 63), 1u64 << 63);
        assert_eq!(stream_draws("F", &doubling, 64), u64::MAX, "saturates");
        let still = [('F', "F+".to_string())];
        assert_eq!(stream_draws("F", &still, STREAM_DEPTH), 1, "no rule grows");
    }

    /// The bracket bound covers what a balanced grammar's stream opens.
    #[test]
    fn the_bracket_bound_holds_the_streams_deepest_nesting() {
        let rules = [('X', "F[+X][-[X]]".to_string())];
        assert_eq!(nesting("F[+X][-[X]]"), 2);
        for depth in 0..=6 {
            let deepest = nesting(&expand("[X]", &rules, depth));
            assert!(
                deepest <= bracket_bound("[X]", &rules, depth),
                "depth {depth}: opened {deepest}"
            );
        }
    }

    #[test]
    fn expand_is_exact_and_deterministic() {
        // Fibonacci-word grammar: trivially hand-verifiable exact strings.
        let rules = [('A', "AB".to_string()), ('B', "A".to_string())];
        assert_eq!(expand("A", &rules, 0), "A");
        assert_eq!(expand("A", &rules, 1), "AB");
        assert_eq!(expand("A", &rules, 2), "ABA");
        assert_eq!(expand("A", &rules, 3), "ABAAB");
        assert_eq!(expand("A", &rules, 5), "ABAABABAABAAB");

        // The canonical example: Koch edge F -> "F+F--F+F".
        let koch = [('F', "F+F--F+F".to_string())];
        assert_eq!(expand("F", &koch, 1), "F+F--F+F");
        assert_eq!(
            expand("F", &koch, 2),
            "F+F--F+F+F+F--F+F--F+F--F+F+F+F--F+F"
        );

        // Determinism: the same inputs twice are identical.
        assert_eq!(expand("F", &koch, 3), expand("F", &koch, 3));
    }

    #[test]
    fn characters_without_a_rule_pass_through() {
        // `+`, `-`, `[`, `]` have no rules and survive verbatim; only `X`/`F`
        // rewrite. A single expansion of the classic plant axiom.
        let rules = [('X', "F[+X]F".to_string()), ('F', "FF".to_string())];
        assert_eq!(expand("X", &rules, 1), "F[+X]F");
        assert_eq!(expand("X", &rules, 2), "FF[+F[+X]F]FF");
    }
}

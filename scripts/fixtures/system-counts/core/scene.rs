//! A source file whose count lives in a string rather than in a comment.
//!
//! This is the instance class that survived two closes: an assertion message is
//! not a comment, so a comment lexer walks straight past it.

/// Every system registers here, and the registry is the roster.
pub fn assert_every_system_registers(registered: usize, expected: usize) {
    assert_eq!(
        registered, expected,
        "the scene registry lost a system: seven systems are registered, and the \
         roster declares {expected}",
    );
}

/// A pair and a quartet, neither of them a roster count.
pub const PASSES: usize = 2;
pub const QUADRANTS: usize = 4;

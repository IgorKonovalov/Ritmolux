//! The OSC 1.0 wire encoder: an address string, a type-tag string, and the
//! arguments, each element NUL-terminated where it is a string and every element
//! padded up to a 4-byte boundary.
//!
//! Hand-rolled. NFR section 4's dependency gate asks for a justification for a
//! crate pulling a transitive graph, and the encoder is smaller than the
//! justification would be. [`super::decode`] is its mirror, written beside it so
//! the padding rule below has exactly one statement per direction and a
//! round-trip test can hold the two to each other.
//!
//! The padding rule is the trap: an OSC-string is **always** NUL-terminated and
//! then padded up, so a 4-byte address takes 8 bytes on the wire, not 4. A
//! string whose length is already a multiple of 4 gains a full 4 bytes of
//! padding rather than none. `pad_to_4` is that rule, and every element goes
//! through it.

/// Prefix every published address carries, version included.
pub const ADDRESS_PREFIX: &str = "/rlx/v1";

/// How many addresses the fixed set publishes — the length of
/// [`super::Telemetry::messages`], exposed so a caller can size a buffer or assert the
/// roster without re-counting it.
pub const ADDRESS_COUNT: usize = 14;

/// One OSC argument. The three types this project reads and writes; `s` borrows
/// so the preset name does not have to be cloned every frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arg<'a> {
    /// OSC `f` — 32-bit IEEE-754 float, big-endian.
    F(f32),
    /// OSC `i` — 32-bit two's-complement integer, big-endian.
    I(i32),
    /// OSC `s` — NUL-terminated, padded to a 4-byte boundary.
    S(&'a str),
}

impl Arg<'_> {
    /// The type-tag character this argument contributes to the tag string.
    fn tag(self) -> u8 {
        match self {
            Arg::F(_) => b'f',
            Arg::I(_) => b'i',
            Arg::S(_) => b's',
        }
    }
}

/// Bytes of NUL padding that take `len` up to the next multiple of 4, **at least
/// one** — an OSC-string is NUL-terminated before it is padded, so a length that
/// is already a multiple of 4 still takes a full 4 bytes.
fn pad_to_4(len: usize) -> usize {
    4 - (len % 4)
}

/// Append `text` as an OSC-string: the bytes, a NUL, then padding to the next
/// 4-byte boundary.
///
/// An interior NUL would make the receiver read a shorter string than was
/// written and then mis-parse everything after it, so it is replaced rather than
/// passed through. The only string this sink sends is a preset name, which comes
/// from a file stem and never contains one; the substitution is a guard against
/// a future caller, not a live case.
fn push_string(buf: &mut Vec<u8>, text: &str) {
    let start = buf.len();
    buf.extend(text.bytes().map(|b| if b == 0 { b'?' } else { b }));
    let written = buf.len() - start;
    buf.extend(std::iter::repeat_n(0u8, pad_to_4(written)));
}

/// Encode one OSC message into `buf`, **replacing** whatever it held.
///
/// The buffer is passed in rather than returned so a per-frame send reuses one
/// allocation. The result is always a multiple of 4 bytes long, which is the
/// property the whole format rests on.
pub fn encode(buf: &mut Vec<u8>, address: &str, args: &[Arg<'_>]) {
    buf.clear();
    push_string(buf, address);

    // The type-tag string is itself an OSC-string, leading comma included, so it
    // takes the same NUL-terminate-then-pad treatment as the address.
    let start = buf.len();
    buf.push(b',');
    buf.extend(args.iter().map(|a| a.tag()));
    let written = buf.len() - start;
    buf.extend(std::iter::repeat_n(0u8, pad_to_4(written)));

    for arg in args {
        match *arg {
            Arg::F(v) => buf.extend_from_slice(&v.to_bits().to_be_bytes()),
            Arg::I(v) => buf.extend_from_slice(&v.to_be_bytes()),
            Arg::S(v) => push_string(buf, v),
        }
    }
}

//! The OSC 1.0 wire decoder, and the studio control vocabulary it parses
//! (ADR-0176).
//!
//! The mirror of [`super::encode`], written beside it so the padding rule has
//! one statement per direction. Everything here is a **pure function of a byte
//! slice** — no socket, no state, no clock — which is what lets the whole
//! vocabulary be round-tripped and fuzzed without opening a port.
//!
//! ## Nothing here allocates
//!
//! The decoder runs on the listener thread, which must not stall behind an
//! allocator while a show is playing, so [`Action`] is `Copy` and carries a
//! name as an inline [`Name`] rather than a `String`. That is what fixes
//! [`Name::CAP`]: a name longer than it is **rejected and counted**, not
//! truncated, because a truncated parameter name would silently drive the wrong
//! parameter.
//!
//! ## Accept only what is understood
//!
//! Every path out of [`decode`] is either one known address with exactly the
//! argument types its row declares, or a [`Reject`]. There is no
//! best-effort arm: an address under `/rlx/v1/ctl/` with the wrong type tags is
//! refused rather than coerced, because the sender is a program and a coerced
//! argument is a bug that presents as a picture doing something odd.
//!
//! OSC has no reply channel (ADR-0164), so a rejection is counted and reported
//! in aggregate through the `health` event; `ctl/ping` exists so a studio can
//! tell a dead player from a quiet one.

/// The control vocabulary's address prefix — the telemetry root plus `ctl`.
///
/// Versioned in the address exactly as telemetry is: a later action is additive
/// under this prefix, and a breaking change moves the `v1` segment.
pub const CONTROL_PREFIX: &str = "/rlx/v1/ctl";

/// A parameter or preset name carried without allocating.
///
/// The listener thread decodes into this and hands it to the render thread
/// through a fixed-capacity queue, so no packet touches the allocator. `CAP` is
/// generous against both populations it holds — the longest engine parameter
/// name is under twenty bytes, and a preset name is a `name = "..."` key an
/// author typed — and a name that exceeds it is refused rather than truncated.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name {
    bytes: [u8; Self::CAP],
    len: u8,
}

impl Name {
    /// The inline capacity, in bytes.
    pub const CAP: usize = 64;

    /// Copy `text` inline, or `None` when it does not fit.
    pub fn new(text: &str) -> Option<Self> {
        if text.len() > Self::CAP {
            return None;
        }
        let mut bytes = [0u8; Self::CAP];
        bytes
            .get_mut(..text.len())?
            .copy_from_slice(text.as_bytes());
        // `len` fits: `CAP` is 64 and the check above bounds `text.len()` by it.
        Some(Self {
            bytes,
            len: text.len() as u8,
        })
    }

    /// The name as a string slice.
    pub fn as_str(&self) -> &str {
        // The bytes came from a `&str` in `new` and nothing mutates them after,
        // so the prefix is valid UTF-8; the fallback keeps this total rather
        // than making a decoder path panic.
        self.bytes
            .get(..usize::from(self.len))
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }
}

impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// The transport verbs `ctl/transport` names.
///
/// `auto` and `hold` are the two **positions** of the console strip's one
/// rotation control, not two presses of it: a control surface sends a position
/// and expects to land there, so sending `auto` twice must not turn rotation
/// off. The shell reads the live state and acts only on a difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// Cut to the roster's successor.
    Next,
    /// Cut to the roster's predecessor.
    Prev,
    /// Hands-off rotation on.
    Auto,
    /// Hands-off rotation off.
    Hold,
}

impl Transport {
    /// The wire spelling, which is also what the spec's table prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Transport::Next => "next",
            Transport::Prev => "prev",
            Transport::Auto => "auto",
            Transport::Hold => "hold",
        }
    }

    /// Parse a wire spelling. Unknown verbs are rejected, never mapped onto a
    /// neighbour.
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "next" => Some(Transport::Next),
            "prev" => Some(Transport::Prev),
            "auto" => Some(Transport::Auto),
            "hold" => Some(Transport::Hold),
            _ => None,
        }
    }
}

/// One decoded control message.
///
/// `Copy` and allocation-free, so the listener can hand it to the render thread
/// without touching the allocator — see the module docs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Hold `value` on `name` until it is cleared.
    Param { name: Name, value: f32 },
    /// Drop that override; the preset's own binding resumes.
    ClearParam { name: Name },
    /// Drop every override.
    ClearParams,
    /// Dissolve to the named preset.
    Preset { name: Name },
    /// The console's transport, by name.
    Transport(Transport),
    /// Answered by a `pong` event carrying the same nonce.
    Ping(i32),
}

impl Action {
    /// The address this action travels under.
    pub fn address(&self) -> &'static str {
        match self {
            Action::Param { .. } => "/rlx/v1/ctl/param",
            Action::ClearParam { .. } => "/rlx/v1/ctl/param/clear",
            Action::ClearParams => "/rlx/v1/ctl/params/clear",
            Action::Preset { .. } => "/rlx/v1/ctl/preset",
            Action::Transport(_) => "/rlx/v1/ctl/transport",
            Action::Ping(_) => "/rlx/v1/ctl/ping",
        }
    }

    /// Encode this action into `buf`, **replacing** whatever it held.
    ///
    /// The player never sends one of these — the studio does. It exists so the
    /// round-trip test drives both directions through the shipped code rather
    /// than through a second hand-built byte string per row, which is what makes
    /// the test a statement about the pair.
    pub fn encode(&self, buf: &mut Vec<u8>) {
        use super::Arg;
        let address = self.address();
        match self {
            Action::Param { name, value } => {
                super::encode(buf, address, &[Arg::S(name.as_str()), Arg::F(*value)]);
            }
            Action::ClearParam { name } | Action::Preset { name } => {
                super::encode(buf, address, &[Arg::S(name.as_str())]);
            }
            Action::ClearParams => super::encode(buf, address, &[]),
            Action::Transport(verb) => super::encode(buf, address, &[Arg::S(verb.as_str())]),
            Action::Ping(nonce) => super::encode(buf, address, &[Arg::I(*nonce)]),
        }
    }
}

/// Why a datagram was not turned into an [`Action`].
///
/// Counted rather than answered — OSC has no reply channel. The variants are
/// what a test asserts against and what a future diagnostic would print; the
/// listener adds them all to one `rejected` total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    /// Not a well-formed OSC message: a length that is not a multiple of 4, a
    /// string that runs off the end, a missing or malformed type-tag string, or
    /// an argument the declared tags say is there and is not.
    Malformed,
    /// Well-formed OSC, but not an address this player answers to.
    UnknownAddress,
    /// A known address whose argument types are not the ones its row declares.
    WrongArguments,
    /// A known address carrying a name longer than [`Name::CAP`], or a verb the
    /// vocabulary does not name.
    Unusable,
}

/// Read an OSC-string starting at `*pos`, advancing `pos` past its padding.
///
/// The inverse of `encode::push_string`: bytes up to a NUL, then padding to the
/// next 4-byte boundary. Returns `None` on a run-off, on invalid UTF-8, or when
/// the terminator is missing — each of which is a datagram this player will not
/// guess at.
fn read_string<'a>(buf: &'a [u8], pos: &mut usize) -> Option<&'a str> {
    let rest = buf.get(*pos..)?;
    let nul = rest.iter().position(|b| *b == 0)?;
    let text = std::str::from_utf8(rest.get(..nul)?).ok()?;
    // The written length is the bytes plus the NUL, padded up; `pad_to_4` is at
    // least one, so this is exactly what the encoder advanced by.
    let written = nul + 1;
    let padded = written + ((4 - (written % 4)) % 4);
    *pos = pos.checked_add(padded)?;
    // A well-formed message never leaves `pos` past the end; a truncated one is
    // caught here rather than by the argument read that follows.
    (*pos <= buf.len()).then_some(text)
}

/// Read a big-endian 4-byte argument starting at `*pos`.
fn read_word(buf: &[u8], pos: &mut usize) -> Option<[u8; 4]> {
    let end = pos.checked_add(4)?;
    let word: [u8; 4] = buf.get(*pos..end)?.try_into().ok()?;
    *pos = end;
    Some(word)
}

/// Decode one datagram into the action it names.
///
/// **Total and panic-free for every input**, which is the property the listener
/// thread depends on: it hands whatever arrived on the socket straight to this.
pub fn decode(datagram: &[u8]) -> Result<Action, Reject> {
    // An OSC packet is a whole number of 4-byte words by construction. Checking
    // it here means every read below is against a slice whose shape is already
    // known, and it costs a mistyped sender one rejection instead of four.
    if datagram.is_empty() || !datagram.len().is_multiple_of(4) {
        return Err(Reject::Malformed);
    }
    let mut pos = 0;
    let address = read_string(datagram, &mut pos).ok_or(Reject::Malformed)?;
    let tags = read_string(datagram, &mut pos).ok_or(Reject::Malformed)?;
    let tags = tags.strip_prefix(',').ok_or(Reject::Malformed)?;

    // Addresses first, argument types second, so a well-formed message to an
    // address this build does not know is `UnknownAddress` rather than a
    // complaint about its arguments.
    let action = match address {
        "/rlx/v1/ctl/param" => {
            if tags != "sf" {
                return Err(Reject::WrongArguments);
            }
            let name = read_string(datagram, &mut pos).ok_or(Reject::Malformed)?;
            let value = f32::from_bits(u32::from_be_bytes(
                read_word(datagram, &mut pos).ok_or(Reject::Malformed)?,
            ));
            Action::Param {
                name: Name::new(name).ok_or(Reject::Unusable)?,
                value,
            }
        }
        "/rlx/v1/ctl/param/clear" | "/rlx/v1/ctl/preset" => {
            if tags != "s" {
                return Err(Reject::WrongArguments);
            }
            let raw = read_string(datagram, &mut pos).ok_or(Reject::Malformed)?;
            let name = Name::new(raw).ok_or(Reject::Unusable)?;
            if address == "/rlx/v1/ctl/preset" {
                Action::Preset { name }
            } else {
                Action::ClearParam { name }
            }
        }
        "/rlx/v1/ctl/params/clear" => {
            if !tags.is_empty() {
                return Err(Reject::WrongArguments);
            }
            Action::ClearParams
        }
        "/rlx/v1/ctl/transport" => {
            if tags != "s" {
                return Err(Reject::WrongArguments);
            }
            let raw = read_string(datagram, &mut pos).ok_or(Reject::Malformed)?;
            Action::Transport(Transport::parse(raw).ok_or(Reject::Unusable)?)
        }
        "/rlx/v1/ctl/ping" => {
            if tags != "i" {
                return Err(Reject::WrongArguments);
            }
            Action::Ping(i32::from_be_bytes(
                read_word(datagram, &mut pos).ok_or(Reject::Malformed)?,
            ))
        }
        _ => return Err(Reject::UnknownAddress),
    };
    Ok(action)
}

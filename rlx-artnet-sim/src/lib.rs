//! The simulated rig: an `ArtDmx` decoder, a receiver that reassembles
//! datagrams into a raster, and a PNG writer (ADR-0174).
//!
//! Art-Net is fire-and-forget UDP on port 6454 and a node never replies, so a
//! sender exercises the identical code path whether or not anything is
//! listening. That is what makes this substitution mechanical rather than
//! merely convenient: what leaves the socket is the whole of what the wire
//! carries, and a receiver reads it more exactly than a person watches a room.
//!
//! ## What this is not
//!
//! **Not a fixture emulator.** It models no node behaviour: no latching, no
//! refresh-rate preference, no stick gamma, no `ArtSync` handling. It reads the
//! wire and draws the raster. Everything it cannot see is the physical rig's to
//! answer, and ADR-0174 enumerates that list.
//!
//! ## Frame delimitation
//!
//! Art-Net without `ArtSync` has **no frame boundary** - unsynchronized
//! datagrams simply arrive. [`Receiver`] closes a frame when a universe index
//! repeats. That is exact for a sender emitting each universe once per frame,
//! and it is silently wrong for a sender that does not: one emitting a universe
//! twice per frame would be read as two frames, and one emitting a subset would
//! never close. The assumption is recorded here because nothing detects its
//! violation.
//!
//! ## The universe number is the 15-bit port address
//!
//! `ArtDmx` carries `SubUni` (byte 14) and `Net` (byte 15), which the
//! specification composes into a 15-bit port address `Net << 8 | SubUni` whose
//! low byte splits further into a 4-bit sub-net and a 4-bit universe. This
//! decoder does not split it: it reports the composed port address, so a rig
//! patched to "universes 0-23" is read as port addresses 0-23 and the byte on
//! the wire is what is compared. Splitting would rename universe 23 to
//! "sub-net 1, universe 7" and buy nothing, because no rig here addresses by
//! sub-net.

use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::time::Duration;

#[cfg(test)]
mod tests;

/// The eight-byte identifier every Art-Net packet opens with, null included.
pub const ARTNET_ID: [u8; 8] = *b"Art-Net\0";

/// `OpDmx` / `OpOutput`, the opcode of a DMX data packet. **Little-endian on
/// the wire**, unlike every other multi-byte field in the header.
pub const OP_DMX: u16 = 0x5000;

/// The protocol revision this decoder understands. Big-endian on the wire; the
/// specification tells a receiver to accept anything at or above its own.
pub const PROTOCOL_VERSION: u16 = 14;

/// Bytes before the channel data: identifier, opcode, version, sequence,
/// physical, port address, length.
pub const HEADER_LEN: usize = 18;

/// A DMX universe is 512 channels, so no `ArtDmx` body may exceed that.
pub const MAX_CHANNELS: usize = 512;

/// The rig's universe count - the vertical extent of its raster.
pub const RIG_UNIVERSES: usize = 24;

/// Pixels per universe - the horizontal extent. 170 pixels is 510 channels,
/// the largest whole RGB pixel count a universe holds.
pub const RIG_PIXELS: usize = 170;

/// Why a datagram is not an `ArtDmx` packet this decoder will read.
///
/// Every variant is a rejection rather than a repair: a packet that does not
/// match the specification is dropped, never guessed at, because a decoder that
/// guesses would hide exactly the encoder bug it exists to catch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer bytes than a header, so no field can be read at all.
    TooShort(usize),
    /// The first eight bytes are not `Art-Net\0`.
    BadIdentifier,
    /// A well-formed Art-Net packet of some other kind (`ArtPoll`, `ArtSync`).
    BadOpcode(u16),
    /// Protocol revision below [`PROTOCOL_VERSION`].
    OldProtocol(u16),
    /// The length field is odd. The specification requires an even count, and
    /// an odd one means the sender is writing a byte count it did not derive
    /// from its channel array.
    OddLength(u16),
    /// The length field is zero or above [`MAX_CHANNELS`].
    LengthOutOfRange(u16),
    /// The length field promises more channels than the datagram carries.
    ShortBody {
        /// Channels the header declared.
        declared: usize,
        /// Channels actually present after the header.
        present: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort(len) => {
                write!(f, "{len} bytes is shorter than an {HEADER_LEN}-byte header")
            }
            Self::BadIdentifier => f.write_str("the packet does not open with `Art-Net\\0`"),
            Self::BadOpcode(op) => write!(f, "opcode {op:#06x} is not ArtDmx ({OP_DMX:#06x})"),
            Self::OldProtocol(v) => write!(f, "protocol version {v} is below {PROTOCOL_VERSION}"),
            Self::OddLength(n) => write!(f, "length field {n} is odd"),
            Self::LengthOutOfRange(n) => write!(f, "length field {n} is not in 1..={MAX_CHANNELS}"),
            Self::ShortBody { declared, present } => {
                write!(
                    f,
                    "length field declares {declared} channels but {present} are present"
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// One decoded `ArtDmx` packet, borrowing its channel data from the datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtDmx<'a> {
    /// Sender's frame counter, or 0 when the sender declines to sequence.
    pub sequence: u8,
    /// Physical input port, informational only.
    pub physical: u8,
    /// The composed 15-bit port address - see this module's header for why it
    /// is not split into net / sub-net / universe.
    pub port_address: u16,
    /// Exactly as many channels as the length field declared.
    pub data: &'a [u8],
}

/// Read one datagram as an `ArtDmx` packet.
pub fn decode(datagram: &[u8]) -> Result<ArtDmx<'_>, DecodeError> {
    if datagram.len() < HEADER_LEN {
        return Err(DecodeError::TooShort(datagram.len()));
    }
    if datagram[0..8] != ARTNET_ID {
        return Err(DecodeError::BadIdentifier);
    }
    let opcode = u16::from_le_bytes([datagram[8], datagram[9]]);
    if opcode != OP_DMX {
        return Err(DecodeError::BadOpcode(opcode));
    }
    let version = u16::from_be_bytes([datagram[10], datagram[11]]);
    if version < PROTOCOL_VERSION {
        return Err(DecodeError::OldProtocol(version));
    }
    let declared = u16::from_be_bytes([datagram[16], datagram[17]]);
    if !declared.is_multiple_of(2) {
        return Err(DecodeError::OddLength(declared));
    }
    if declared == 0 || usize::from(declared) > MAX_CHANNELS {
        return Err(DecodeError::LengthOutOfRange(declared));
    }
    let present = datagram.len() - HEADER_LEN;
    if present < usize::from(declared) {
        return Err(DecodeError::ShortBody {
            declared: usize::from(declared),
            present,
        });
    }
    Ok(ArtDmx {
        sequence: datagram[12],
        physical: datagram[13],
        // Byte 14 is the low byte and byte 15 the high one: the field is
        // little-endian, like the opcode and unlike the length.
        port_address: u16::from(datagram[14]) | (u16::from(datagram[15]) << 8),
        data: &datagram[HEADER_LEN..HEADER_LEN + usize::from(declared)],
    })
}

/// The rig's addressable surface: one row per universe, one RGB pixel per
/// column.
///
/// Row-major RGB triples. Universe index is the row because that is what the
/// rig's patch makes it - a vertical coordinate - and the fixture map, not this
/// type, is where that claim is configurable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    universes: usize,
    pixels: usize,
    rgb: Vec<u8>,
}

impl Raster {
    /// An all-zero raster of the given geometry.
    pub fn black(universes: usize, pixels: usize) -> Self {
        Self {
            universes,
            pixels,
            rgb: vec![0u8; universes * pixels * 3],
        }
    }

    /// The rig's own geometry, all zero.
    pub fn rig() -> Self {
        Self::black(RIG_UNIVERSES, RIG_PIXELS)
    }

    /// Rows.
    pub fn universes(&self) -> usize {
        self.universes
    }

    /// Columns.
    pub fn pixels(&self) -> usize {
        self.pixels
    }

    /// One pixel's RGB triple.
    ///
    /// # Panics
    /// If `universe` or `pixel` is outside the geometry.
    pub fn pixel(&self, universe: usize, pixel: usize) -> [u8; 3] {
        let at = self.offset(universe, pixel);
        [self.rgb[at], self.rgb[at + 1], self.rgb[at + 2]]
    }

    /// Write one pixel.
    ///
    /// # Panics
    /// If `universe` or `pixel` is outside the geometry.
    pub fn set_pixel(&mut self, universe: usize, pixel: usize, rgb: [u8; 3]) {
        let at = self.offset(universe, pixel);
        self.rgb[at..at + 3].copy_from_slice(&rgb);
    }

    /// Write a universe's worth of channels, RGB triple by RGB triple.
    ///
    /// Channels past the raster's width are dropped and a trailing partial
    /// triple is ignored: a sender is free to address more pixels than this
    /// geometry models, and reading a partial pixel would invent a colour.
    ///
    /// # Panics
    /// If `universe` is outside the geometry.
    pub fn set_universe(&mut self, universe: usize, channels: &[u8]) {
        let count = (channels.len() / 3).min(self.pixels);
        let at = self.offset(universe, 0);
        self.rgb[at..at + count * 3].copy_from_slice(&channels[..count * 3]);
    }

    /// Whether every channel is zero - the assertable form of "the rig is
    /// dark", which watching a room cannot distinguish from a node that merely
    /// stopped receiving.
    pub fn is_black(&self) -> bool {
        self.rgb.iter().all(|c| *c == 0)
    }

    /// The raw row-major RGB bytes.
    pub fn rgb(&self) -> &[u8] {
        &self.rgb
    }

    /// Write a PNG, upscaled by an integer factor with nearest-neighbour.
    ///
    /// The rig's raster is a hairline - 170 x 24 - so a viewer that resamples
    /// would blur away the one thing worth looking at. `scale` is clamped up to
    /// 1 so a zero cannot produce an empty image.
    pub fn write_png(&self, path: &Path, scale: u32) -> io::Result<()> {
        let scale = scale.max(1);
        let width = self.pixels as u32 * scale;
        let height = self.universes as u32 * scale;
        let mut out = vec![0u8; (width * height * 3) as usize];
        for y in 0..height {
            let universe = (y / scale) as usize;
            for x in 0..width {
                let pixel = (x / scale) as usize;
                let src = self.offset(universe, pixel);
                let dst = ((y * width + x) * 3) as usize;
                out[dst..dst + 3].copy_from_slice(&self.rgb[src..src + 3]);
            }
        }
        image::save_buffer(path, &out, width, height, image::ExtendedColorType::Rgb8)
            .map_err(|err| io::Error::other(format!("{}: {err}", path.display())))
    }

    fn offset(&self, universe: usize, pixel: usize) -> usize {
        assert!(
            universe < self.universes && pixel < self.pixels,
            "({universe}, {pixel}) is outside a {} x {} raster",
            self.universes,
            self.pixels
        );
        (universe * self.pixels + pixel) * 3
    }
}

/// One delimited frame: the raster it painted, and which universes contributed.
///
/// `seen` is kept beside the raster because a black row and an unsent row look
/// identical in the pixels, and telling them apart is most of what a capture is
/// for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The reassembled surface.
    pub raster: Raster,
    /// One flag per universe: whether a datagram addressed it in this frame.
    pub seen: Vec<bool>,
}

impl Frame {
    /// How many universes contributed.
    pub fn seen_count(&self) -> usize {
        self.seen.iter().filter(|s| **s).count()
    }

    /// Whether every universe in the geometry contributed.
    pub fn is_complete(&self) -> bool {
        self.seen.iter().all(|s| *s)
    }
}

/// Why a datagram did not reach the raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveError {
    /// It is not a readable `ArtDmx` packet.
    Decode(DecodeError),
    /// It is, but it addresses a universe this geometry does not model.
    UniverseOutOfRig {
        /// The port address the packet carried.
        port_address: u16,
        /// The geometry's universe count.
        universes: usize,
    },
}

impl fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(err) => write!(f, "{err}"),
            Self::UniverseOutOfRig {
                port_address,
                universes,
            } => write!(
                f,
                "port address {port_address} is outside a {universes}-universe rig"
            ),
        }
    }
}

impl std::error::Error for ReceiveError {}

impl From<DecodeError> for ReceiveError {
    fn from(err: DecodeError) -> Self {
        Self::Decode(err)
    }
}

/// Reassembles datagrams into frames, closing one when a universe repeats.
///
/// A new frame starts **black** rather than carrying the previous frame's
/// pixels forward. The rig's nodes latch and this does not, deliberately: a
/// carried-forward raster would make a universe that stopped being sent
/// indistinguishable from one still being sent, which is the failure the
/// `seen` flags exist to expose.
#[derive(Debug, Clone)]
pub struct Receiver {
    raster: Raster,
    seen: Vec<bool>,
    open: bool,
}

impl Receiver {
    /// A receiver for the given geometry.
    pub fn new(universes: usize, pixels: usize) -> Self {
        Self {
            raster: Raster::black(universes, pixels),
            seen: vec![false; universes],
            open: false,
        }
    }

    /// A receiver for the rig's own 24 x 170 geometry.
    pub fn rig() -> Self {
        Self::new(RIG_UNIVERSES, RIG_PIXELS)
    }

    /// Feed one datagram.
    ///
    /// Returns the **previous** frame when this datagram's universe repeats one
    /// already in flight, which is the delimiter: the returned frame is
    /// complete and the datagram has started the next one.
    pub fn accept(&mut self, datagram: &[u8]) -> Result<Option<Frame>, ReceiveError> {
        let packet = decode(datagram)?;
        let universe = usize::from(packet.port_address);
        if universe >= self.seen.len() {
            return Err(ReceiveError::UniverseOutOfRig {
                port_address: packet.port_address,
                universes: self.seen.len(),
            });
        }
        let closed = self.seen[universe].then(|| self.take());
        self.seen[universe] = true;
        self.open = true;
        self.raster.set_universe(universe, packet.data);
        Ok(closed)
    }

    /// Close whatever is in flight - what a capture calls when the stream ends,
    /// since the last frame has no successor to delimit it.
    pub fn flush(&mut self) -> Option<Frame> {
        self.open.then(|| self.take())
    }

    fn take(&mut self) -> Frame {
        let frame = Frame {
            raster: self.raster.clone(),
            seen: self.seen.clone(),
        };
        self.raster = Raster::black(self.raster.universes(), self.raster.pixels());
        self.seen.fill(false);
        self.open = false;
        frame
    }
}

/// A bound UDP socket that hands raw datagrams to a caller-owned [`Receiver`].
///
/// The socket and the reassembly are separate on purpose: an integration test
/// asserts per-datagram properties - the channel count above all - that a
/// delimited [`Frame`] does not carry, so it needs the bytes before they become
/// pixels.
pub struct UdpCapture {
    socket: UdpSocket,
    buf: Vec<u8>,
}

impl UdpCapture {
    /// Bind the given address.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(addr)?,
            // A datagram cannot exceed a header plus a full universe.
            buf: vec![0u8; HEADER_LEN + MAX_CHANNELS],
        })
    }

    /// Bind loopback on an OS-chosen port, so a test can run concurrently with
    /// any other and with a real rig on 6454.
    pub fn bind_ephemeral() -> io::Result<Self> {
        Self::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
    }

    /// The bound address, which is how a test learns the ephemeral port.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Give up on a silent sender after `timeout`, so a capture loop cannot
    /// hang a test run.
    pub fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Receive one datagram, or `None` once the read timeout expires.
    ///
    /// The copy out of the read buffer is deliberate: this is a test
    /// instrument, and handing back an owned `Vec` is what lets the caller keep
    /// a datagram while receiving the next.
    pub fn recv(&mut self) -> io::Result<Option<Vec<u8>>> {
        match self.socket.recv_from(&mut self.buf) {
            Ok((len, _from)) => Ok(Some(self.buf[..len].to_vec())),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }
}

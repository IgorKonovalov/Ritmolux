//! The decoder is held to the Art-Net specification, not to our encoder.
//!
//! [`SPEC_FIXTURE`] is written out byte by byte from the specification's packet
//! definition, by hand, and every field of it is annotated with the offset and
//! the endianness it came from. That ordering is the whole reason this crate
//! exists before an emitter does: if the decoder's only input were an encoder
//! in this repository, a shared misreading of the specification would
//! round-trip perfectly and stay invisible on every test built on top of it
//! (ADR-0174).

use super::*;

/// A hand-built `ArtDmx` packet, **derived from the Art-Net specification** and
/// from no code in this repository.
///
/// | offset | bytes         | field       | reading                          |
/// |--------|---------------|-------------|----------------------------------|
/// | 0-7    | `41..00`      | ID          | `Art-Net` and a null             |
/// | 8-9    | `00 50`       | OpCode      | 0x5000 `OpDmx`, **little**-endian |
/// | 10-11  | `00 0e`       | ProtVer     | 14, **big**-endian               |
/// | 12     | `07`          | Sequence    | 7                                |
/// | 13     | `00`          | Physical    | 0                                |
/// | 14     | `17`          | SubUni      | 23, the port address' low byte   |
/// | 15     | `00`          | Net         | 0, the high byte                 |
/// | 16-17  | `00 06`       | Length      | 6, **big**-endian and even       |
/// | 18-23  | `ff 00 00 ...`| Data        | two RGB pixels: red, then green  |
const SPEC_FIXTURE: [u8; 24] = [
    0x41, 0x72, 0x74, 0x2d, 0x4e, 0x65, 0x74, 0x00, // "Art-Net\0"
    0x00, 0x50, // OpDmx, little-endian
    0x00, 0x0e, // protocol 14, big-endian
    0x07, // sequence
    0x00, // physical
    0x17, 0x00, // SubUni = 23, Net = 0
    0x00, 0x06, // length 6, big-endian
    0xff, 0x00, 0x00, // pixel 0: red
    0x00, 0xff, 0x00, // pixel 1: green
];

#[test]
fn the_spec_fixture_decodes_to_its_universe_and_channels() {
    let packet = decode(&SPEC_FIXTURE).expect("the spec-derived fixture decodes");
    assert_eq!(packet.port_address, 23);
    assert_eq!(packet.sequence, 7);
    assert_eq!(packet.physical, 0);
    assert_eq!(packet.data, &[0xff, 0x00, 0x00, 0x00, 0xff, 0x00]);
}

#[test]
fn a_wrong_identifier_is_rejected() {
    let mut bad = SPEC_FIXTURE;
    bad[3] = b'X';
    assert_eq!(decode(&bad), Err(DecodeError::BadIdentifier));
}

#[test]
fn a_wrong_opcode_is_rejected() {
    // 0x2000 is `OpPoll`: a well-formed Art-Net packet that is not DMX data.
    let mut bad = SPEC_FIXTURE;
    bad[8] = 0x00;
    bad[9] = 0x20;
    assert_eq!(decode(&bad), Err(DecodeError::BadOpcode(0x2000)));
}

#[test]
fn a_big_endian_opcode_is_rejected_as_the_wrong_opcode() {
    // The one field on the wire that is little-endian, so writing it the way
    // every neighbouring field is written produces 0x0050 rather than 0x5000.
    let mut bad = SPEC_FIXTURE;
    bad[8] = 0x50;
    bad[9] = 0x00;
    assert_eq!(decode(&bad), Err(DecodeError::BadOpcode(0x0050)));
}

#[test]
fn an_old_protocol_version_is_rejected() {
    let mut bad = SPEC_FIXTURE;
    bad[10] = 0x00;
    bad[11] = 0x0d;
    assert_eq!(decode(&bad), Err(DecodeError::OldProtocol(13)));
}

#[test]
fn an_odd_length_field_is_rejected() {
    let mut bad = SPEC_FIXTURE;
    bad[17] = 0x05;
    assert_eq!(decode(&bad), Err(DecodeError::OddLength(5)));
}

#[test]
fn a_little_endian_length_field_is_rejected_as_out_of_range() {
    // Length is big-endian; writing 6 the way the opcode is written puts 0x0600
    // on the wire, which is 1536 channels.
    let mut bad = SPEC_FIXTURE;
    bad[16] = 0x06;
    bad[17] = 0x00;
    assert_eq!(decode(&bad), Err(DecodeError::LengthOutOfRange(1536)));
}

#[test]
fn a_zero_length_field_is_rejected() {
    let mut bad = SPEC_FIXTURE;
    bad[16] = 0x00;
    bad[17] = 0x00;
    assert_eq!(decode(&bad), Err(DecodeError::LengthOutOfRange(0)));
}

#[test]
fn a_short_body_is_rejected() {
    let bad = &SPEC_FIXTURE[..SPEC_FIXTURE.len() - 2];
    assert_eq!(
        decode(bad),
        Err(DecodeError::ShortBody {
            declared: 6,
            present: 4
        })
    );
}

#[test]
fn a_datagram_shorter_than_a_header_is_rejected() {
    assert_eq!(decode(&SPEC_FIXTURE[..12]), Err(DecodeError::TooShort(12)));
}

#[test]
fn the_net_byte_lifts_the_port_address_by_256() {
    let mut packet = SPEC_FIXTURE;
    packet[15] = 0x01;
    let decoded = decode(&packet).expect("a packet on net 1 decodes");
    assert_eq!(decoded.port_address, 256 + 23);
}

/// Build one `ArtDmx` datagram, writing the same offsets [`SPEC_FIXTURE`]
/// documents. A test-local builder rather than an import: nothing in this crate
/// encodes Art-Net, and the tests below are about the receiver rather than
/// about an emitter.
fn datagram(universe: u16, channels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + channels.len());
    out.extend_from_slice(&ARTNET_ID);
    out.extend_from_slice(&OP_DMX.to_le_bytes());
    out.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    out.push(0); // sequence
    out.push(0); // physical
    out.push((universe & 0xff) as u8);
    out.push((universe >> 8) as u8);
    out.extend_from_slice(&(channels.len() as u16).to_be_bytes());
    out.extend_from_slice(channels);
    out
}

/// A raster whose every pixel is a distinct function of its coordinates, so a
/// transposed or off-by-one reassembly cannot pass by symmetry.
fn synthesized_rig() -> Raster {
    let mut raster = Raster::rig();
    for universe in 0..RIG_UNIVERSES {
        for pixel in 0..RIG_PIXELS {
            raster.set_pixel(
                universe,
                pixel,
                [
                    (universe * 10 + 3) as u8,
                    (pixel + 1) as u8,
                    (universe as u8) ^ (pixel as u8),
                ],
            );
        }
    }
    raster
}

/// The datagrams a sender emitting each universe once per frame would produce
/// for `raster`.
fn datagrams_for(raster: &Raster) -> Vec<Vec<u8>> {
    (0..raster.universes())
        .map(|universe| {
            let channels: Vec<u8> = (0..raster.pixels())
                .flat_map(|pixel| raster.pixel(universe, pixel))
                .collect();
            datagram(universe as u16, &channels)
        })
        .collect()
}

#[test]
fn a_synthesized_rig_round_trips_through_the_receiver() {
    let expected = synthesized_rig();
    let mut receiver = Receiver::rig();
    for datagram in datagrams_for(&expected) {
        assert_eq!(
            receiver.accept(&datagram).expect("every datagram is read"),
            None,
            "one frame's datagrams close no frame"
        );
    }
    let frame = receiver.flush().expect("the frame in flight closes");
    assert!(frame.is_complete(), "all 24 universes contributed");
    assert_eq!(frame.raster, expected);
}

#[test]
fn the_png_pixels_match_the_raster() {
    let expected = synthesized_rig();
    let path = std::env::temp_dir().join(format!(
        "rlx-artnet-sim-round-trip-{}.png",
        std::process::id()
    ));
    expected.write_png(&path, 3).expect("the PNG is written");

    let decoded = image::open(&path)
        .expect("the written file decodes")
        .to_rgb8();
    assert_eq!(decoded.width(), RIG_PIXELS as u32 * 3);
    assert_eq!(decoded.height(), RIG_UNIVERSES as u32 * 3);
    for universe in 0..RIG_UNIVERSES {
        for pixel in 0..RIG_PIXELS {
            // Every source pixel became a 3 x 3 block; sample its centre and
            // one corner, which differ only if the upscale resampled.
            let want = expected.pixel(universe, pixel);
            for (dx, dy) in [(0u32, 0u32), (1, 1), (2, 2)] {
                let got = decoded.get_pixel(pixel as u32 * 3 + dx, universe as u32 * 3 + dy);
                assert_eq!(got.0, want, "({universe}, {pixel}) offset ({dx}, {dy})");
            }
        }
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_repeated_universe_closes_a_frame() {
    let first = synthesized_rig();
    let mut second = first.clone();
    second.set_pixel(0, 0, [1, 2, 3]);

    let mut receiver = Receiver::rig();
    let mut frames = Vec::new();
    for datagram in datagrams_for(&first)
        .into_iter()
        .chain(datagrams_for(&second))
    {
        if let Some(frame) = receiver.accept(&datagram).expect("every datagram is read") {
            frames.push(frame);
        }
    }
    if let Some(frame) = receiver.flush() {
        frames.push(frame);
    }

    assert_eq!(frames.len(), 2, "two frames' datagrams produce two frames");
    assert_eq!(frames[0].raster, first);
    assert_eq!(frames[1].raster, second);
}

#[test]
fn a_new_frame_starts_black_rather_than_carrying_the_previous_one() {
    let lit = synthesized_rig();
    let mut receiver = Receiver::rig();
    for datagram in datagrams_for(&lit) {
        let _ = receiver.accept(&datagram).expect("every datagram is read");
    }
    // The second frame addresses universe 0 only: the delimiter fires and the
    // other 23 rows must come back black rather than inheriting frame one.
    let closed = receiver
        .accept(&datagram(0, &[0u8; 510]))
        .expect("the repeat is read")
        .expect("the repeat closes the first frame");
    assert_eq!(closed.raster, lit);

    let partial = receiver.flush().expect("the partial frame closes");
    assert!(partial.raster.is_black());
    assert_eq!(partial.seen_count(), 1);
    assert!(!partial.is_complete());
}

#[test]
fn a_universe_outside_the_geometry_is_refused() {
    let mut receiver = Receiver::rig();
    assert_eq!(
        receiver.accept(&datagram(24, &[0u8; 6])),
        Err(ReceiveError::UniverseOutOfRig {
            port_address: 24,
            universes: RIG_UNIVERSES
        })
    );
}

#[test]
fn a_udp_capture_reads_what_was_sent() {
    let mut capture = UdpCapture::bind_ephemeral().expect("an ephemeral port binds");
    capture
        .set_timeout(Some(Duration::from_secs(2)))
        .expect("the read timeout is set");
    let target = capture.local_addr().expect("the bound address is readable");

    let sender = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).expect("a sender binds");
    let sent = datagram(5, &[9u8; 510]);
    sender.send_to(&sent, target).expect("the datagram is sent");

    let got = capture
        .recv()
        .expect("the receive succeeds")
        .expect("a datagram arrives before the timeout");
    assert_eq!(got, sent);
}

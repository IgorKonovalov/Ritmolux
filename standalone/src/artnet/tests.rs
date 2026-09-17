//! The packet against the specification, and the fixture map against the file.
//!
//! The assertions on the header are **dimensionless properties** — an
//! identifier, an opcode and two endiannesses — so they hold whatever rig is
//! patched and whatever look drives it. The round-trip against the simulator's
//! decoder lives in `standalone/tests/artnet_loopback.rs`, where the datagrams
//! have crossed a socket.

use super::*;

/// The map the shipped defaults describe: one node on loopback carrying all 24
/// universes at 170 pixels.
fn default_map() -> config::Artnet {
    config::Artnet::default()
}

#[test]
fn the_packet_matches_the_art_net_header_specification() {
    let mut buf = Vec::new();
    encode(&mut buf, 7, 23, 170, [1, 2, 3]);

    assert_eq!(&buf[0..8], b"Art-Net\0", "the identifier");
    assert_eq!(
        u16::from_le_bytes([buf[8], buf[9]]),
        0x5000,
        "OpDmx is little-endian on the wire"
    );
    assert_eq!(
        [buf[8], buf[9]],
        [0x00, 0x50],
        "the opcode bytes, spelled out: a big-endian 0x5000 would be 0x50 0x00"
    );
    assert_eq!(
        u16::from_be_bytes([buf[10], buf[11]]),
        14,
        "the protocol version is big-endian"
    );
    assert_eq!([buf[10], buf[11]], [0x00, 0x0e], "the version bytes");
    assert_eq!(buf[12], 7, "the sequence");
    assert_eq!(buf[13], 0, "physical");
    assert_eq!([buf[14], buf[15]], [23, 0], "SubUni then Net");

    let declared = u16::from_be_bytes([buf[16], buf[17]]);
    assert_eq!(declared, 510, "the length field is big-endian");
    assert!(declared.is_multiple_of(2), "the length field must be even");
    assert_eq!(buf.len(), HEADER_LEN + 510, "the body is what it declared");
}

/// **Every datagram is a full universe.** The length is the cause of the stale
/// tail, and the only half of it visible from this side of the wire.
#[test]
fn a_frame_of_the_configured_width_carries_510_channels() {
    let mut buf = Vec::new();
    encode(&mut buf, 1, 0, usize::from(MAX_PIXELS), [9, 9, 9]);
    assert_eq!(u16::from_be_bytes([buf[16], buf[17]]), 510);
    assert_eq!(buf.len(), HEADER_LEN + 510);
}

#[test]
fn every_pixel_of_the_body_carries_the_colour() {
    let mut buf = Vec::new();
    encode(&mut buf, 1, 0, 170, [10, 20, 30]);
    let body = &buf[HEADER_LEN..];
    assert_eq!(body.len(), 510);
    for (pixel, rgb) in body.chunks_exact(3).enumerate() {
        assert_eq!(rgb, [10, 20, 30], "pixel {pixel}");
    }
}

#[test]
fn the_net_byte_carries_a_port_address_above_255() {
    let mut buf = Vec::new();
    encode(&mut buf, 1, 300, 2, [0, 0, 0]);
    assert_eq!([buf[14], buf[15]], [300u16 as u8, 1]);
}

/// The buffer is reused across frames, so a wider frame followed by a narrower
/// one must not leave the first one's tail on the wire.
#[test]
fn a_narrower_frame_does_not_inherit_the_previous_bodys_tail() {
    let mut buf = Vec::new();
    encode(&mut buf, 1, 0, 170, [255, 255, 255]);
    encode(&mut buf, 2, 0, 4, [0, 0, 0]);
    assert_eq!(buf.len(), HEADER_LEN + 12);
    assert!(buf[HEADER_LEN..].iter().all(|c| *c == 0));
}

#[test]
fn the_default_map_resolves_to_twenty_four_full_universes() {
    let sink = ArtnetSink::bind(&default_map()).expect("the shipped default map binds");
    let universes = sink.universes();
    assert_eq!(universes.len(), 24);
    for (index, universe) in universes.iter().enumerate() {
        assert_eq!(universe.port_address, index as u16);
        assert_eq!(universe.pixels, MAX_PIXELS);
    }
}

/// The universe index becomes a coordinate by the rule the file states, ends
/// included — which is what a look reads and what a mis-patched rig is repaired
/// through.
#[test]
fn the_universe_coordinate_spans_the_configured_range() {
    let sink = ArtnetSink::bind(&default_map()).expect("the shipped default map binds");
    let universes = sink.universes();
    assert_eq!(universes[0].coord, 0.0);
    assert_eq!(universes[23].coord, 1.0);
    assert!((universes[12].coord - 12.0 / 23.0).abs() < 1e-6);
    assert_eq!(sink.axis(), config::SpaceAxis::Y);
}

/// Swapping the two ends flips the rig, which is the repair a mis-read patch
/// needs and the reason they are two keys rather than one range.
#[test]
fn a_reversed_range_flips_the_coordinate() {
    let mut map = default_map();
    map.space.universe_min = 23;
    map.space.universe_max = 0;
    let sink = ArtnetSink::bind(&map).expect("a reversed axis binds");
    assert_eq!(sink.universes()[0].coord, 1.0);
    assert_eq!(sink.universes()[23].coord, 0.0);
}

#[test]
fn two_nodes_are_one_flat_universe_list_in_file_order() {
    let mut map = default_map();
    map.node = vec![
        config::ArtnetNode {
            target: "127.0.0.1:6454".to_owned(),
            universes: [0, 1],
            pixels: 170,
        },
        config::ArtnetNode {
            target: "127.0.0.2:6455".to_owned(),
            universes: [2, 3],
            pixels: 60,
        },
    ];
    let sink = ArtnetSink::bind(&map).expect("a two-node map binds");
    let universes = sink.universes();
    assert_eq!(universes.len(), 4);
    assert_eq!(universes[1].target.port(), 6454);
    assert_eq!(universes[2].target.port(), 6455);
    assert_eq!(universes[3].pixels, 60);
}

/// Two nodes claiming one universe would put two frames a frame on the wire for
/// it, which no receiver can tell from a doubled frame rate.
#[test]
fn an_overlapping_fixture_map_is_refused() {
    let mut map = default_map();
    map.node = vec![
        config::ArtnetNode {
            target: "127.0.0.1:6454".to_owned(),
            universes: [0, 5],
            pixels: 170,
        },
        config::ArtnetNode {
            target: "127.0.0.2:6454".to_owned(),
            universes: [5, 9],
            pixels: 170,
        },
    ];
    let err = ArtnetSink::bind(&map).expect_err("an overlapping map must be refused");
    assert!(err.contains("universe 5"), "the message names it: {err}");
}

#[test]
fn a_chain_wider_than_a_universe_is_refused() {
    let mut map = default_map();
    map.node[0].pixels = MAX_PIXELS + 1;
    let err = ArtnetSink::bind(&map).expect_err("171 pixels does not fit 512 channels");
    assert!(err.contains("171"), "the message names the width: {err}");
}

#[test]
fn a_zero_width_chain_is_refused() {
    let mut map = default_map();
    map.node[0].pixels = 0;
    assert!(ArtnetSink::bind(&map).is_err());
}

#[test]
fn a_backwards_universe_range_is_refused() {
    let mut map = default_map();
    map.node[0].universes = [9, 2];
    let err = ArtnetSink::bind(&map).expect_err("a backwards range must be refused");
    assert!(err.contains("backwards"), "{err}");
}

#[test]
fn an_empty_fixture_map_is_refused() {
    let mut map = default_map();
    map.node.clear();
    assert!(ArtnetSink::bind(&map).is_err());
}

#[test]
fn a_target_that_names_no_port_is_refused() {
    let mut map = default_map();
    map.node[0].target = "127.0.0.1".to_owned();
    assert!(ArtnetSink::bind(&map).is_err());
}

/// A degenerate span reads 0 rather than dividing by zero, because a one-row rig
/// is a legitimate map.
#[test]
fn a_single_point_axis_reads_zero() {
    assert_eq!(normalized(4, 4, 4), 0.0);
}

#[test]
fn a_coordinate_outside_the_declared_span_is_clamped() {
    assert_eq!(normalized(30, 0, 23), 1.0);
    assert_eq!(normalized(0, 4, 23), 0.0);
}

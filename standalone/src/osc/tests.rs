//! The encoder against OSC 1.0's own padding rules, and the fixed address table
//! against itself.
//!
//! Every assertion here is **exact and dimensionless** — a byte count derivable
//! from the spec on paper, or a big-endian bit pattern. Nothing tolerances, and
//! nothing depends on a socket, a clock or a machine: the sink's *transport* is
//! wall-clock paced and leaves the process, so the only part of it that can be
//! tested at all is this one, and it is tested exactly.

use super::{ADDRESS_COUNT, ADDRESS_PREFIX, Arg, Telemetry, encode, rms_of};

/// A telemetry snapshot with distinguishable values, so a transposed field in
/// [`Telemetry::messages`] shows up as a wrong number rather than as another 0.
fn sample(preset: &str) -> Telemetry<'_> {
    Telemetry {
        bass: 0.125,
        mid: 0.25,
        treb: 0.375,
        onset: 0.5,
        rms: 0.625,
        bass_raw: 1.5,
        mid_raw: 2.5,
        treb_raw: 3.5,
        onset_raw: 4.5,
        beat: true,
        beat_index: 7,
        beat_phase: 0.75,
        tempo: 128.0,
        preset,
    }
}

/// **The rule the whole format rests on, in the case that looks like it needs
/// no padding.** An OSC-string is NUL-terminated *and then* padded up, so a
/// 4-byte address takes 8 bytes on the wire — a `len.next_multiple_of(4)`
/// encoder passes every other test and fails this one, and the receiver then
/// reads the type tags as part of the address.
#[test]
fn an_address_of_length_four_still_takes_four_bytes_of_padding() {
    let mut buf = Vec::new();
    encode(&mut buf, "/abc", &[]);

    assert_eq!(&buf[..4], b"/abc");
    assert_eq!(
        &buf[4..8],
        &[0, 0, 0, 0],
        "a full pad, not zero bytes of it"
    );
    // The type-tag string of a no-argument message is a bare comma, which is
    // itself an OSC-string: 1 byte, so 3 of padding.
    assert_eq!(&buf[8..12], b",\0\0\0");
    assert_eq!(buf.len(), 12);
}

/// A 3-character string takes 4 bytes: three of content and the one NUL that
/// terminates it. The pad is what completes the boundary, not what creates it.
#[test]
fn a_three_character_string_pads_to_four() {
    let mut buf = Vec::new();
    encode(&mut buf, "/rlx/v1/preset", &[Arg::S("abc")]);

    // "/rlx/v1/preset" is 14 bytes, so 2 of padding; ",s" is 2, so 2 more.
    assert_eq!(&buf[..14], b"/rlx/v1/preset");
    assert_eq!(&buf[14..16], &[0, 0]);
    assert_eq!(&buf[16..20], b",s\0\0");
    assert_eq!(&buf[20..24], b"abc\0", "three bytes and the terminator");
    assert_eq!(buf.len(), 24);

    // And the 4-character case takes eight, for the same reason the address did.
    encode(&mut buf, "/rlx/v1/preset", &[Arg::S("abcd")]);
    assert_eq!(&buf[20..28], b"abcd\0\0\0\0");
    assert_eq!(buf.len(), 28);
}

/// A float message, byte for byte. 128.0 is `0x43000000` in IEEE-754, and OSC is
/// big-endian — so a little-endian slip shows up here as `00 00 00 43`.
#[test]
fn a_float_message_is_big_endian_and_exactly_sized() {
    let mut buf = Vec::new();
    encode(&mut buf, "/rlx/v1/tempo", &[Arg::F(128.0)]);

    // 13-byte address + 3 pad; ",f" + 2 pad; 4 bytes of payload.
    assert_eq!(&buf[..13], b"/rlx/v1/tempo");
    assert_eq!(&buf[13..16], &[0, 0, 0]);
    assert_eq!(&buf[16..20], b",f\0\0");
    assert_eq!(&buf[20..24], &[0x43, 0x00, 0x00, 0x00]);
    assert_eq!(buf.len(), 24);
}

/// The same for `i`, whose two's-complement negative case is where a hand-rolled
/// encoder most easily disagrees with the spec.
#[test]
fn an_int_message_is_big_endian_twos_complement() {
    let mut buf = Vec::new();
    encode(&mut buf, "/rlx/v1/beat/index", &[Arg::I(1)]);

    // 18-byte address + 2 pad = 20; ",i" + 2 pad = 4; then 4 of payload.
    assert_eq!(&buf[18..20], &[0, 0]);
    assert_eq!(&buf[20..24], b",i\0\0");
    assert_eq!(&buf[24..28], &[0, 0, 0, 1]);
    assert_eq!(buf.len(), 28);

    encode(&mut buf, "/rlx/v1/beat/index", &[Arg::I(-2)]);
    assert_eq!(&buf[24..28], &[0xff, 0xff, 0xff, 0xfe]);
}

/// **Every message the sink can emit is a multiple of 4 bytes long.** Swept
/// across preset-name lengths 0 through 12, because the name is the only
/// variable-length element in the set and the boundary case recurs every four
/// characters rather than once.
#[test]
fn every_message_in_the_fixed_set_is_a_multiple_of_four() {
    let mut buf = Vec::new();
    for len in 0..=12 {
        let name = "x".repeat(len);
        for (address, arg) in sample(&name).messages() {
            encode(&mut buf, address, &[arg]);
            assert_eq!(
                buf.len() % 4,
                0,
                "{address} with a {len}-character preset name encoded to {} bytes",
                buf.len()
            );
            assert!(
                buf.len() >= 8,
                "{address} encoded to less than two elements"
            );
        }
    }
}

/// The address table: the roster is the size it claims, every address is
/// versioned, and no two addresses collide — a duplicate would make one signal
/// silently overwrite another in the console's mapping.
#[test]
fn the_address_table_is_versioned_and_free_of_collisions() {
    let messages = sample("rose_star").messages();
    assert_eq!(messages.len(), ADDRESS_COUNT);

    let mut addresses: Vec<&str> = messages.iter().map(|(a, _)| *a).collect();
    for address in &addresses {
        assert!(
            address.starts_with(ADDRESS_PREFIX),
            "{address} is outside the versioned prefix"
        );
    }
    addresses.sort_unstable();
    let before = addresses.len();
    addresses.dedup();
    assert_eq!(before, addresses.len(), "two signals share an address");
}

/// The set carries the values it was given, in the slots the table names — the
/// assertion that catches a transposed `mid`/`treb` or a raw twin bound to its
/// normalized field.
#[test]
fn the_table_binds_each_value_to_its_own_address() {
    let messages = sample("rose_star").messages();
    let find = |address: &str| {
        messages
            .iter()
            .find(|(a, _)| *a == address)
            .unwrap_or_else(|| panic!("{address} is not in the table"))
            .1
    };

    assert_eq!(find("/rlx/v1/level/bass"), Arg::F(0.125));
    assert_eq!(find("/rlx/v1/level/mid"), Arg::F(0.25));
    assert_eq!(find("/rlx/v1/level/treb"), Arg::F(0.375));
    assert_eq!(find("/rlx/v1/level/onset"), Arg::F(0.5));
    assert_eq!(find("/rlx/v1/level/rms"), Arg::F(0.625));
    assert_eq!(find("/rlx/v1/raw/bass"), Arg::F(1.5));
    assert_eq!(find("/rlx/v1/raw/mid"), Arg::F(2.5));
    assert_eq!(find("/rlx/v1/raw/treb"), Arg::F(3.5));
    assert_eq!(find("/rlx/v1/raw/onset"), Arg::F(4.5));
    assert_eq!(find("/rlx/v1/beat/trigger"), Arg::I(1));
    assert_eq!(find("/rlx/v1/beat/index"), Arg::I(7));
    assert_eq!(find("/rlx/v1/beat/phase"), Arg::F(0.75));
    assert_eq!(find("/rlx/v1/tempo"), Arg::F(128.0));
    assert_eq!(find("/rlx/v1/preset"), Arg::S("rose_star"));
}

/// The beat trigger is the discrete event, so it is 0 on a frame with no onset —
/// and the counter saturates rather than wrapping negative, which is what a
/// console watching a ratchet needs from a `u32` crossing `i32::MAX`.
#[test]
fn the_beat_trigger_is_binary_and_the_counter_saturates() {
    let mut quiet = sample("p");
    quiet.beat = false;
    let messages = quiet.messages();
    let trigger = messages
        .iter()
        .find(|(a, _)| *a == "/rlx/v1/beat/trigger")
        .expect("the trigger is in the table")
        .1;
    assert_eq!(trigger, Arg::I(0));

    let mut long = sample("p");
    long.beat_index = u32::MAX;
    let messages = long.messages();
    let index = messages
        .iter()
        .find(|(a, _)| *a == "/rlx/v1/beat/index")
        .expect("the counter is in the table")
        .1;
    assert_eq!(index, Arg::I(i32::MAX));
}

/// RMS on signals whose answer is arithmetic rather than measured: a square wave
/// at full scale is exactly 1, silence is exactly 0, an empty trace is 0 rather
/// than a NaN, and a full-scale sine is `1/sqrt(2)`.
#[test]
fn rms_matches_its_closed_form() {
    assert_eq!(rms_of(&[]), 0.0);
    assert_eq!(rms_of(&[0.0; 64]), 0.0);
    assert_eq!(rms_of(&[1.0, -1.0, 1.0, -1.0]), 1.0);
    assert_eq!(rms_of(&[0.5, -0.5]), 0.5);

    // A whole number of periods of a unit sine: RMS is 1/sqrt(2). The tolerance
    // is float round-off on the sum, not a modelling allowance.
    let n = 1024;
    let sine: Vec<f32> = (0..n)
        .map(|i| (std::f32::consts::TAU * i as f32 / n as f32).sin())
        .collect();
    let expected = std::f32::consts::FRAC_1_SQRT_2;
    assert!(
        (rms_of(&sine) - expected).abs() < 1e-6,
        "sine RMS was {}, expected {expected}",
        rms_of(&sine)
    );
}

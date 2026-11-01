//! Host-build guarantees for the sequence library: const encoders and runtime accessors.

use example::Serializable;

/// Wire form: big-endian opcode, then an `FwSizeStoreType` length (a `u16`, which is what
/// the fixture falls back to) and the string bytes.
#[test]
fn const_encoders_on_host() {
    const N: usize = example::Konst::Ref::wasmSeq::LOAD__size("helloworld");
    const BYTES: [u8; N] = example::Konst::Ref::wasmSeq::LOAD__encode::<N>("helloworld");

    assert_eq!(
        BYTES,
        [
            0x10, 0x00, 0x70, 0x02, // opcode 0x10007002
            0x00, 0x0a, // length 10
            b'h', b'e', b'l', b'l', b'o', b'w', b'o', b'r', b'l', b'd',
        ]
    );
}

/// The const encoder writes whatever width `FwSizeStoreType` is, big-endian.
#[test]
fn const_string_prefix_is_sized_by_the_dictionary() {
    let mut buffer = [0u8; 16];

    let end = example::put_str(&mut buffer, 0, "hey", 40, 8);
    assert_eq!(end, example::str_len("hey", 40, 8));
    assert_eq!(&buffer[..end], &[0, 0, 0, 0, 0, 0, 0, 3, b'h', b'e', b'y']);

    let end = example::put_str(&mut buffer, 0, "hey", 2, 1);
    assert_eq!(&buffer[..end], &[2, b'h', b'e']);
}

/// A string round-trips through any prefix width, and its size counts that prefix.
#[test]
fn string_round_trips_through_a_wide_prefix() {
    use example::StrTruncate;

    type Wide = fprime_core::String<8, u64>;
    assert_eq!(Wide::SIZE, 8 + 8);

    let mut buffer = [0u8; Wide::SIZE];
    let mut offset = 0usize;
    let hi: Wide = StrTruncate::truncate("hi");
    hi.serialize_to(&mut buffer, &mut offset);
    assert_eq!(&buffer[..offset], &[0, 0, 0, 0, 0, 0, 0, 2, b'h', b'i']);

    assert_eq!(Wide::deserialize(&buffer).as_str(), "hi");
}

/// The dictionary's `String` takes its prefix from `FwSizeStoreType`.
#[test]
fn dictionary_string_uses_the_size_store_type() {
    assert_eq!(
        <example::String<40> as Serializable>::SIZE,
        size_of::<example::Dict::FwSizeStoreType>() + 40
    );
}

/// Every generated shape (enum, struct) must round-trip off-target.
#[test]
fn round_trip_on_host() {
    let mut buffer = [0u8; 64];
    let mut offset = 0usize;

    // An enum serialises as its representation type, big-endian.
    example::Dict::Ref::Choice::BLUE.serialize_to(&mut buffer, &mut offset);
    assert_eq!((&buffer[..4], offset), (&[0, 0, 0, 3][..], 4));

    // A struct serialises its members in dictionary order.
    let pair = example::Dict::Ref::ChoicePair {
        firstChoice: example::Dict::Ref::Choice::RED,
        secondChoice: example::Dict::Ref::Choice::BLUE,
    };
    let mut offset = 0usize;
    pair.serialize_to(&mut buffer, &mut offset);
    assert_eq!(&buffer[..8], &[0, 0, 0, 2, 0, 0, 0, 3]);
    assert_eq!(
        example::Dict::Ref::ChoicePair::deserialize(&buffer).secondChoice,
        example::Dict::Ref::Choice::BLUE
    );
}

/// Regression: `exit`/`panic`/`time` collide with libc symbols off-target.
#[test]
#[should_panic(expected = "fprime_v1.cmd was called in a host build")]
fn cmd_accessor_aborts_on_host() {
    let _ = example::CdhCore.cmdDisp.CMD_NO_OP();
}

/// Same hazard, for `time` — the most dangerous collision since libc's takes a pointer.
#[test]
#[should_panic(expected = "fprime_v1.time was called in a host build")]
fn clock_read_aborts_on_host() {
    let mut buffer = [0u8; 11];
    unsafe { example::time::time_read(&mut buffer) };
}

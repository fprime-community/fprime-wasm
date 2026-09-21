//! Host-build guarantees for the sequence library: const encoders and runtime accessors.

use example::Serializable;

/// Wire form: big-endian opcode, then a `u16` length and the string bytes.
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

/// Every generated shape (enum, struct) must round-trip off-target.
#[test]
fn round_trip_on_host() {
    let mut buffer = [0u8; 64];
    let mut offset = 0usize;

    // An enum serialises as its representation type, big-endian.
    example::Defs::Ref::Choice::BLUE.serialize_to(&mut buffer, &mut offset);
    assert_eq!((&buffer[..4], offset), (&[0, 0, 0, 3][..], 4));

    // A struct serialises its members in dictionary order.
    let pair = example::Defs::Ref::ChoicePair {
        firstChoice: example::Defs::Ref::Choice::RED,
        secondChoice: example::Defs::Ref::Choice::BLUE,
    };
    let mut offset = 0usize;
    pair.serialize_to(&mut buffer, &mut offset);
    assert_eq!(&buffer[..8], &[0, 0, 0, 2, 0, 0, 0, 3]);
    assert_eq!(
        example::Defs::Ref::ChoicePair::deserialize(&buffer).secondChoice,
        example::Defs::Ref::Choice::BLUE
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

//! Generated `Desc` consts: one per dictionary point, for tests to name.

use example::Desc;
use fprime_core::Serializable;
use fprime_core::desc::{Chan, IntoWire, Prm};

/// Serializes through the descriptor's wire type, serving only the bytes it wrote. The
/// descriptor is what pins that type; it carries no encoder of its own.
fn wire<I: Serializable>(_chan: Chan<I>, value: impl IntoWire<I>) -> Vec<u8> {
    serialize(value.into_wire())
}

fn wire_prm<I: Serializable>(_prm: Prm<I>, value: impl IntoWire<I>) -> Vec<u8> {
    serialize(value.into_wire())
}

fn serialize<I: Serializable>(value: I) -> Vec<u8> {
    let mut buffer = vec![0u8; I::SIZE];
    let mut offset = 0;
    value.serialize_to(&mut buffer, &mut offset);
    buffer.truncate(offset);
    buffer
}

#[test]
fn scalar_channel_encodes_big_endian() {
    let chan = Desc::Ref::typeDemo::ScalarF32Ch;
    assert_eq!(chan.id, 0x10005012);
    assert_eq!(chan.path, "Ref.typeDemo.ScalarF32Ch");
    assert_eq!(chan.ty, "F32");
    // 21.5f32 is 0x41AC0000.
    assert_eq!(wire(chan, 21.5), vec![0x41, 0xac, 0x00, 0x00]);
}

/// Enum channels take the generated enum, not a bare number.
#[test]
fn enum_channel_takes_generated_enum() {
    let chan = Desc::Ref::typeDemo::ChoiceCh;
    assert_eq!(chan.ty, "Ref.Choice");
    assert_eq!(
        wire(chan, example::Defs::Ref::Choice::BLUE),
        vec![0x00, 0x00, 0x00, 0x03]
    );
}

/// `string size N` still takes a bare `&str`; wire form is `u16` length then bytes.
#[test]
fn string_channel_takes_str() {
    let chan = Desc::Ref::typeDemo::NameCh;
    assert_eq!(chan.ty, "string size 40");
    assert_eq!(wire(chan, "safe"), vec![0x00, 0x04, b's', b'a', b'f', b'e']);
    // The buffer is sized for the cap, but only what was written is served.
    assert_eq!(wire(chan, ""), vec![0x00, 0x00]);
}

/// More than the point holds is truncated to the cap, as assigning an `Fw::String` is —
/// never a panic in the middle of a test run.
#[test]
fn string_channel_truncates_to_the_dictionary_cap() {
    let chan = Desc::Ref::typeDemo::NameCh; // string size 40
    let bytes = wire(chan, "x".repeat(50).as_str());
    assert_eq!(&bytes[..2], &[0x00, 40]);
    assert_eq!(bytes.len(), 2 + 40);
}

/// Truncation stops at a character boundary: 40 bytes of a 3-byte character lands
/// mid-character, so 39 are sent.
#[test]
fn string_channel_truncates_whole_characters() {
    let chan = Desc::Ref::typeDemo::NameCh;
    let bytes = wire(chan, "€".repeat(14).as_str());
    assert_eq!(&bytes[..2], &[0x00, 39]);
    assert_eq!(bytes.len(), 2 + 39);
}

/// An alias behaves as its underlying type (`FwSizeType` is `u64`).
#[test]
fn aliased_channel_resolves_underlying_type() {
    let chan = Desc::CdhCore::events::EventsDropped;
    assert_eq!(chan.ty, "FwSizeType");
    assert_eq!(wire(chan, 3), vec![0, 0, 0, 0, 0, 0, 0, 3]);
}

/// Parameters are a distinct descriptor type from channels.
#[test]
fn parameter_is_its_own_descriptor_type() {
    let prm = Desc::Ref::recvBuffComp::parameter1;
    assert_eq!(prm.id, 0x10022000);
    assert_eq!(prm.path, "Ref.recvBuffComp.parameter1");
    assert_eq!(wire_prm(prm, 7), vec![0, 0, 0, 7]);
}

/// A bare command descriptor matches on opcode alone.
#[test]
fn command_descriptor_carries_opcode_and_name() {
    let cmd = Desc::Ref::wasmSeq::LOAD;
    assert_eq!(cmd.opcode, 0x10007002);
    assert_eq!(cmd.path, "Ref.wasmSeq.LOAD");
}

/// `LOAD("helloworld")` carries the exact ComBuffer the sequence itself encodes.
#[test]
fn command_with_arguments_carries_exact_combuffer() {
    const LEN: usize = example::Konst::Ref::wasmSeq::LOAD__size("helloworld");
    const BUFFER: [u8; LEN] = example::Konst::Ref::wasmSeq::LOAD__encode::<LEN>("helloworld");
    // A `const` item of reference type gets a `'static` reference without leaning on
    // rvalue static promotion, and stays deduplicable across call sites.
    const BYTES: &[u8] = &BUFFER;

    let cmd = Desc::Ref::wasmSeq::LOAD.with(BYTES);
    assert_eq!(cmd.opcode(), 0x10007002);
    assert_eq!(cmd.path, "Ref.wasmSeq.LOAD");
    // `args()` is what a recorded `Call::Command` payload holds: the harness splits the
    // opcode off before recording.
    assert_eq!(
        cmd.args(),
        &[
            0x00, 0x0a, b'h', b'e', b'l', b'l', b'o', b'w', b'o', b'r', b'l', b'd'
        ]
    );
}

/// Codes must match `Fw::CmdResponse`.
#[test]
fn responses_carry_name_and_code() {
    assert_eq!(Desc::Response::OK.code, 0);
    assert_eq!(Desc::Response::OK.name, "OK");
    assert_eq!(Desc::Response::EXECUTION_ERROR.code, 4);
    assert_eq!(Desc::Response::EXECUTION_ERROR.name, "EXECUTION_ERROR");
}

/// `Copy` even when the value type isn't — the impls don't require `I: Copy`.
#[test]
fn descriptor_is_copy_regardless_of_value_type() {
    // `Ref.ScalarStruct` derives Clone, Debug, PartialEq and Serializable — not Copy.
    let chan = Desc::Ref::typeDemo::ScalarStructCh;
    let copied = chan;
    assert_eq!(chan.path, copied.path);
}

/// Members are written in dictionary `index` order, not literal order.
#[test]
fn struct_channel_encodes_members_in_dictionary_order() {
    let chan = Desc::Ref::typeDemo::ScalarStructCh;
    let bytes = wire(
        chan,
        example::Defs::Ref::ScalarStruct {
            // Deliberately out of declaration order, to prove the derive orders it.
            f64: 0.0,
            u8: 0xab,
            i8: -1,
            i16: 0,
            i32: 0,
            i64: 0,
            u16: 0,
            u32: 0,
            u64: 0,
            f32: 0.0,
        },
    );
    assert_eq!(
        bytes.len(),
        <example::Defs::Ref::ScalarStruct as Serializable>::SIZE
    );
    // i8, i16, i32, i64, u8, ... — so byte 0 is `i8` and byte 15 is `u8`.
    assert_eq!(bytes[0], 0xff, "i8 comes first: {bytes:02x?}");
    assert_eq!(bytes[15], 0xab, "u8 comes after the four signed widths");
}

/// An array channel takes the generated array type.
#[test]
fn array_channel_takes_generated_array() {
    use example::Defs::Ref::Choice;
    let chan = Desc::Ref::typeDemo::ChoicesCh;
    assert_eq!(chan.ty, "Ref.ManyChoices");
    assert_eq!(
        wire(chan, [Choice::RED, Choice::BLUE]),
        vec![0, 0, 0, 2, 0, 0, 0, 3]
    );
}

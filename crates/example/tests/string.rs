//! `fprime_core::String`: the fixed-capacity string every `string size N` is.

use core::fmt::Write;
use fprime_core::{Serializable, StrTruncate};

type S8 = fprime_core::String<8, u16>;

fn serialize<I: Serializable>(value: &I) -> Vec<u8> {
    let mut buffer = vec![0u8; I::SIZE];
    let mut offset = 0;
    value.serialize_to(&mut buffer, &mut offset);
    buffer.truncate(offset);
    buffer
}

/// A push that does not fit is refused whole, leaving what was there.
#[test]
fn push_refuses_what_does_not_fit() {
    let mut s = S8::new();
    assert!(s.is_empty());
    assert_eq!(s.push_str("hello"), Ok(()));
    assert_eq!(s.push_str("world"), Err(()));
    assert_eq!(s, "hello");
    assert_eq!(s.push('!'), Ok(()));
    assert_eq!(s.push('€'), Err(()));
    assert_eq!(s.len(), 6);
    assert_eq!(s.capacity(), 8);

    s.clear();
    assert_eq!(s, "");
}

/// It reads as a `str`, so `&str` methods need no conversion.
#[test]
fn derefs_to_str() {
    let s: S8 = "a,b".try_into().unwrap();
    assert!(s.starts_with("a,"));
    assert_eq!(s.split(',').count(), 2);
    assert_eq!(format!("{s} {s:?}"), r#"a,b "a,b""#);
    assert!(S8::try_from("too long by far").is_err());
}

/// Formatting is `write!` or `fprime_core::format!`; overflow is an error, not a cut.
#[test]
fn formats_in_place() {
    let mut s = S8::new();
    write!(s, "{}-{}", 12, 34).unwrap();
    assert_eq!(s, "12-34");
    assert!(write!(s, "{}", 1234).is_err());

    let s: S8 = fprime_core::format!(8, "{:>4}", 7);
    assert_eq!(s, "   7");
}

/// Truncation keeps whole characters: 8 bytes of 3-byte characters keeps 6.
#[test]
fn truncate_stops_on_a_character_boundary() {
    let s: S8 = StrTruncate::truncate("€€€");
    assert_eq!(s, "€€");

    let s: S8 = StrTruncate::truncate("abcdefghij");
    assert_eq!(s, "abcdefgh");
}

/// Every prefix width puts the length big-endian ahead of the bytes, and reads it back.
#[test]
fn round_trips_through_every_prefix() {
    fn check<P: fprime_core::StringPrefixType>(prefix: &[u8]) {
        let s: fprime_core::String<8, P> = StrTruncate::truncate("hi");
        assert_eq!(<fprime_core::String<8, P>>::SIZE, prefix.len() + 8);

        let bytes = serialize(&s);
        assert_eq!(&bytes[..prefix.len()], prefix);
        assert_eq!(&bytes[prefix.len()..], b"hi");

        let back = <fprime_core::String<8, P>>::deserialize(&bytes);
        assert_eq!(back, s);
    }

    check::<u8>(&[2]);
    check::<u16>(&[0, 2]);
    check::<u32>(&[0, 0, 0, 2]);
    check::<u64>(&[0, 0, 0, 0, 0, 0, 0, 2]);
}

/// A `&str` serializes as the string type would, cut to its capacity.
#[test]
fn serialize_str_truncates_to_capacity() {
    let mut buffer = [0u8; 10];
    let mut offset = 0;
    S8::serialize_str("abcdefghij", &mut buffer, &mut offset);
    assert_eq!(&buffer[..offset], b"\x00\x08abcdefgh");
}

/// The length a string's 32-bit field holds would not fit a `u8` prefix.
#[test]
#[should_panic(expected = "fprime_v1.panic was called in a host build")]
fn length_too_long_for_its_prefix_panics() {
    let s: fprime_core::String<300, u8> = StrTruncate::truncate(&"x".repeat(300));
    serialize(&s);
}

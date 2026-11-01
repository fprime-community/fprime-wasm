use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::Serializable;

/// Stack-backed String type
/// The capacity specifies the
pub type String<const N: usize> = heapless::String<N, u32>;

pub trait StrTruncate<const N: usize> {
    fn truncate(s: &str) -> Self;
}

impl<const N: usize> StrTruncate<N> for String<N> {
    fn truncate(s: &str) -> String<N> {
        let mut out: heapless::Vec<u8, N, u32> = heapless::Vec::new();
        let n = core::cmp::min(s.len(), N);
        unsafe {
            // Avoid `copy_nonoverlapping` since that induces a memcpy and without
            // bulk-memory operations in spacewasm it would cost a large amout of `.wasm` size.
            out.set_len(n);
            for (i, c) in s.get_unchecked(..n).as_bytes().iter().enumerate() {
                *out.get_unchecked_mut(i) = *c;
            }
            heapless::String::from_utf8_unchecked(out)
        }
    }
}

mod private {
    use core::{
        fmt::{Debug, Display},
        ops::{Add, AddAssign, Sub, SubAssign},
    };

    use crate::Serializable;

    pub trait Sealed:
        Send
        + Sync
        + Copy
        + Display
        + Debug
        + PartialEq
        + Add<Output = Self>
        + AddAssign
        + Sub<Output = Self>
        + SubAssign
        + PartialOrd
        + TryFrom<usize, Error: Debug>
        + TryInto<usize, Error: Debug>
        + Serializable
    {
    }

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
}

pub trait StringPrefixType: private::Sealed {}

impl StringPrefixType for u8 {}
impl StringPrefixType for u16 {}
impl StringPrefixType for u32 {}
impl StringPrefixType for u64 {}

/// A `string size N` as it goes on the wire: a `PrefixTy` length, then the bytes.
///
/// F Prime serializes a string's length as `FwSizeStoreType`, so generated code names
/// this with the dictionary's alias for `PrefixTy`. It reads as the [`String`] it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialString<const N: usize, PrefixTy: StringPrefixType>(
    String<N>,
    PhantomData<PrefixTy>,
);

impl<const N: usize, PrefixTy: StringPrefixType> SerialString<N, PrefixTy> {
    pub const fn new() -> Self {
        SerialString(String::new(), PhantomData)
    }
}

impl<const N: usize, PrefixTy: StringPrefixType> From<String<N>> for SerialString<N, PrefixTy> {
    fn from(s: String<N>) -> Self {
        SerialString(s, PhantomData)
    }
}

impl<const N: usize, PrefixTy: StringPrefixType> Deref for SerialString<N, PrefixTy> {
    type Target = String<N>;

    fn deref(&self) -> &String<N> {
        &self.0
    }
}

impl<const N: usize, PrefixTy: StringPrefixType> DerefMut for SerialString<N, PrefixTy> {
    fn deref_mut(&mut self) -> &mut String<N> {
        &mut self.0
    }
}

impl<const N: usize, PrefixTy: StringPrefixType> StrTruncate<N> for SerialString<N, PrefixTy> {
    fn truncate(s: &str) -> Self {
        SerialString(<String<N> as StrTruncate<N>>::truncate(s), PhantomData)
    }
}

impl<const N: usize, PrefixTy: StringPrefixType> Serializable for SerialString<N, PrefixTy> {
    const SIZE: usize = PrefixTy::SIZE + N;

    #[inline(always)]
    fn serialize_to(&self, to: &mut [u8], offset: &mut usize) {
        let bytes = self.0.as_bytes();
        let n = bytes.len();
        if let Ok(n) = PrefixTy::try_from(n) {
            n.serialize_to(to, offset);
        } else {
            crate::panic(crate::PanicCode::TooLargeForPrefix)
        }
        let Some(dst) = to.get_mut(*offset..*offset + n) else {
            crate::panic(crate::PanicCode::Truncated)
        };

        dst.copy_from_slice(bytes);
        *offset += n;
    }

    fn deserialize_from(from: &[u8], offset: &mut usize) -> Self {
        let Ok(n) = PrefixTy::deserialize_from(from, offset).try_into() else {
            crate::panic(crate::PanicCode::TooLargeForPrefix);
        };

        if n > N {
            crate::panic(crate::PanicCode::Truncated);
        }

        let Some(src) = from.get(*offset..*offset + n) else {
            crate::panic(crate::PanicCode::Truncated)
        };

        let mut out: heapless::Vec<u8, N, u32> = heapless::Vec::new();
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), out.as_mut_ptr(), n);
            out.set_len(n);
        }

        *offset += n;

        // The host serializes an Fw::String, so these bytes are already valid UTF-8.
        // Validating pulls in `core::str` validation machinery, so only pay for it
        // in debug builds.
        if cfg!(debug_assertions) && core::str::from_utf8(&out).is_err() {
            crate::panic(crate::PanicCode::InvalidStatus);
        }

        unsafe { SerialString(String::from_utf8_unchecked(out), PhantomData) }
    }
}

/// Serialize a `&str` into a `SerialString<N, PREFIX>` wire field, truncated to `N` bytes.
#[inline(always)]
pub fn serialize_str<const N: usize, PREFIX: StringPrefixType>(
    s: &str,
    to: &mut [u8],
    offset: &mut usize,
) {
    let bytes = s.as_bytes();
    let n = core::cmp::min(bytes.len(), N);
    if let Ok(n) = PREFIX::try_from(n) {
        n.serialize_to(to, offset);
    } else {
        crate::panic(crate::PanicCode::TooLargeForPrefix);
    };

    let Some(dst) = to.get_mut(*offset..*offset + n) else {
        crate::panic(crate::PanicCode::Truncated)
    };

    dst.copy_from_slice(&bytes[..n]);
    *offset += n;
}

use crate::Serializable;

/// Stack-backed string of capacity `N` whose length goes on the wire as a `P`.
///
/// `P` is the dictionary's `FwSizeStoreType`; generated code names this through its own
/// `crate::String<N>`, which fills `P` in. In memory it is a plain [`heapless::String`].
///
/// `P` has no default so that reaching this alias instead of the dictionary's is a
/// compile error, not a string with the wrong prefix.
pub type String<const N: usize, P> = heapless::String<N, <P as StringPrefixType>::Len>;

pub trait StrTruncate<const N: usize> {
    fn truncate(s: &str) -> Self;
}

impl<const N: usize, L: StringLen> StrTruncate<N> for heapless::String<N, L> {
    fn truncate(s: &str) -> Self {
        let mut out: heapless::Vec<u8, N, L> = heapless::Vec::new();
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
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for usize {}
}

/// A type a string's length can be serialized as, and the length type a
/// [`heapless::String`] keeps it in.
pub trait StringPrefixType:
    private::Sealed + Copy + TryFrom<usize> + TryInto<usize> + Serializable
{
    type Len: StringLen<Prefix = Self>;
}

/// The other direction: which prefix a [`heapless::String`]'s length type serializes as.
///
/// heapless has no `u64` length, so a `u64` prefix is kept in a `usize`.
pub trait StringLen: private::Sealed + heapless::LenType {
    type Prefix: StringPrefixType<Len = Self>;
}

macro_rules! prefix {
    ($prefix:ty, $len:ty) => {
        impl StringPrefixType for $prefix {
            type Len = $len;
        }

        impl StringLen for $len {
            type Prefix = $prefix;
        }
    };
}

prefix!(u8, u8);
prefix!(u16, u16);
prefix!(u32, u32);
prefix!(u64, usize);

impl<const N: usize, L: StringLen> Serializable for heapless::String<N, L> {
    const SIZE: usize = <L::Prefix as Serializable>::SIZE + N;

    #[inline(always)]
    fn serialize_to(&self, to: &mut [u8], offset: &mut usize) {
        serialize_str::<Self>(self, to, offset)
    }

    fn deserialize_from(from: &[u8], offset: &mut usize) -> Self {
        let Ok(n) = L::Prefix::deserialize_from(from, offset).try_into() else {
            crate::panic(crate::PanicCode::TooLargeForPrefix);
        };

        if n > N {
            crate::panic(crate::PanicCode::Truncated);
        }

        let Some(src) = from.get(*offset..*offset + n) else {
            crate::panic(crate::PanicCode::Truncated)
        };

        let mut out: heapless::Vec<u8, N, L> = heapless::Vec::new();
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

        unsafe { heapless::String::from_utf8_unchecked(out) }
    }
}

/// A string type's wire shape: what [`serialize_str`] needs to write a `&str` as one.
pub trait WireString {
    const CAPACITY: usize;
    type Prefix: StringPrefixType;
}

impl<const N: usize, L: StringLen> WireString for heapless::String<N, L> {
    const CAPACITY: usize = N;
    type Prefix = L::Prefix;
}

/// Serialize a `&str` as an `S` wire field, truncated to its capacity.
#[inline(always)]
pub fn serialize_str<S: WireString>(s: &str, to: &mut [u8], offset: &mut usize) {
    let bytes = s.as_bytes();
    let n = core::cmp::min(bytes.len(), S::CAPACITY);
    let Ok(prefix) = S::Prefix::try_from(n) else {
        crate::panic(crate::PanicCode::TooLargeForPrefix);
    };
    prefix.serialize_to(to, offset);

    let Some(dst) = to.get_mut(*offset..*offset + n) else {
        crate::panic(crate::PanicCode::Truncated)
    };

    dst.copy_from_slice(&bytes[..n]);
    *offset += n;
}

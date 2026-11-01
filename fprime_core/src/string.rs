use core::fmt;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};

use crate::Serializable;

/// Stack-backed string of capacity `N` whose length goes on the wire as a `P`.
///
/// `P` is the dictionary's `FwSizeStoreType`; generated code names this through its own
/// `crate::String<N>`, which fills `P` in. `P` has no default so that reaching this type
/// instead of the dictionary's is a compile error, not a string with the wrong prefix.
///
/// The length is held as a `u32` whatever `P` is: a word-aligned string is copied with
/// inline loads and stores, where a `u16` length makes LLVM reach for a `memcpy` that,
/// without bulk-memory in spacewasm, costs hundreds of bytes of `.wasm`.
pub struct String<const N: usize, P: StringPrefixType> {
    len: u32,
    buf: [MaybeUninit<u8>; N],
    prefix: PhantomData<P>,
}

mod private {
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
}

/// A type a string's length can be serialized as.
pub trait StringPrefixType:
    private::Sealed + Copy + TryFrom<usize> + TryInto<usize> + Serializable
{
}

impl StringPrefixType for u8 {}
impl StringPrefixType for u16 {}
impl StringPrefixType for u32 {}
impl StringPrefixType for u64 {}

/// Byte-at-a-time, rather than `copy_from_slice`, for the same reason as the `u32` length.
#[inline(always)]
fn copy_bytes(dst: &mut [MaybeUninit<u8>], src: &[u8]) {
    for (d, s) in dst.iter_mut().zip(src) {
        d.write(*s);
    }
}

impl<const N: usize, P: StringPrefixType> String<N, P> {
    pub const fn new() -> Self {
        const { assert!(N <= u32::MAX as usize) };

        String {
            len: 0,
            buf: [const { MaybeUninit::uninit() }; N],
            prefix: PhantomData,
        }
    }

    /// The string `args` formats to, or an error if it is longer than `N` bytes.
    pub fn format(args: fmt::Arguments<'_>) -> Result<Self, fmt::Error> {
        let mut out = Self::new();
        fmt::Write::write_fmt(&mut out, args)?;
        Ok(out)
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn as_bytes(&self) -> &[u8] {
        // SAFETY: the first `len` bytes are always initialized
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr() as *const u8, self.len as usize) }
    }

    pub const fn as_str(&self) -> &str {
        // SAFETY: every write appends whole `&str`s or `char`s, so the bytes are UTF-8
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }

    pub fn as_mut_str(&mut self) -> &mut str {
        // SAFETY: as `as_str`; `&mut str` cannot be used to break UTF-8
        unsafe {
            let bytes =
                core::slice::from_raw_parts_mut(self.buf.as_mut_ptr() as *mut u8, self.len());
            core::str::from_utf8_unchecked_mut(bytes)
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Appends `s` whole, or leaves the string as it was if `s` does not fit.
    pub fn push_str(&mut self, s: &str) -> Result<(), ()> {
        let len = self.len();
        let Some(dst) = self.buf.get_mut(len..len + s.len()) else {
            return Err(());
        };

        copy_bytes(dst, s.as_bytes());
        self.len += s.len() as u32;
        Ok(())
    }

    /// Appends `c`, or leaves the string as it was if `c` does not fit.
    pub fn push(&mut self, c: char) -> Result<(), ()> {
        self.push_str(c.encode_utf8(&mut [0; 4]))
    }

    /// Serialize `s` as this string type would be, truncated to `N` bytes.
    ///
    /// The command accessors use this so a caller can pass a `&str` straight through.
    #[inline(always)]
    pub fn serialize_str(s: &str, to: &mut [u8], offset: &mut usize) {
        let bytes = s.as_bytes();
        let n = core::cmp::min(bytes.len(), N);
        let Ok(prefix) = P::try_from(n) else {
            crate::panic(crate::PanicCode::TooLargeForPrefix);
        };
        prefix.serialize_to(to, offset);

        let Some(dst) = to.get_mut(*offset..*offset + n) else {
            crate::panic(crate::PanicCode::Truncated)
        };

        dst.copy_from_slice(&bytes[..n]);
        *offset += n;
    }
}

impl<const N: usize, P: StringPrefixType> Default for String<N, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, P: StringPrefixType> Clone for String<N, P> {
    fn clone(&self) -> Self {
        String {
            len: self.len,
            buf: self.buf,
            prefix: PhantomData,
        }
    }
}

impl<const N: usize, P: StringPrefixType> Deref for String<N, P> {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize, P: StringPrefixType> DerefMut for String<N, P> {
    fn deref_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl<const N: usize, P: StringPrefixType> AsRef<str> for String<N, P> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize, P: StringPrefixType> AsRef<[u8]> for String<N, P> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'a, const N: usize, P: StringPrefixType> TryFrom<&'a str> for String<N, P> {
    type Error = ();

    fn try_from(s: &'a str) -> Result<Self, ()> {
        let mut out = Self::new();
        out.push_str(s)?;
        Ok(out)
    }
}

impl<const N: usize, P: StringPrefixType> fmt::Write for String<N, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s).map_err(|_| fmt::Error)
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c).map_err(|_| fmt::Error)
    }
}

impl<const N: usize, P: StringPrefixType> fmt::Debug for String<N, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize, P: StringPrefixType> fmt::Display for String<N, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl<const N: usize, P: StringPrefixType> PartialEq for String<N, P> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize, P: StringPrefixType> Eq for String<N, P> {}

impl<const N: usize, P: StringPrefixType> PartialEq<str> for String<N, P> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize, P: StringPrefixType> PartialEq<&str> for String<N, P> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

pub trait StrTruncate<const N: usize> {
    fn truncate(s: &str) -> Self;
}

impl<const N: usize, P: StringPrefixType> StrTruncate<N> for String<N, P> {
    /// `s`, cut to the last whole character that fits in `N` bytes.
    fn truncate(s: &str) -> Self {
        let mut n = core::cmp::min(s.len(), N);
        while !s.is_char_boundary(n) {
            n -= 1;
        }

        let mut out = Self::new();
        copy_bytes(&mut out.buf, &s.as_bytes()[..n]);
        out.len = n as u32;
        out
    }
}

impl<const N: usize, P: StringPrefixType> Serializable for String<N, P> {
    const SIZE: usize = P::SIZE + N;

    #[inline(always)]
    fn serialize_to(&self, to: &mut [u8], offset: &mut usize) {
        Self::serialize_str(self.as_str(), to, offset)
    }

    fn deserialize_from(from: &[u8], offset: &mut usize) -> Self {
        let Ok(n) = P::deserialize_from(from, offset).try_into() else {
            crate::panic(crate::PanicCode::TooLargeForPrefix);
        };

        if n > N {
            crate::panic(crate::PanicCode::Truncated);
        }

        let Some(src) = from.get(*offset..*offset + n) else {
            crate::panic(crate::PanicCode::Truncated)
        };

        // The host serializes an Fw::String, so these bytes are already valid UTF-8.
        // Validating pulls in `core::str` validation machinery, so only pay for it
        // in debug builds.
        if cfg!(debug_assertions) && core::str::from_utf8(src).is_err() {
            crate::panic(crate::PanicCode::InvalidStatus);
        }

        let mut out = Self::new();
        copy_bytes(&mut out.buf, src);
        out.len = n as u32;

        *offset += n;
        out
    }
}

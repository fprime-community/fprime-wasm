//! An array of strings. A string nested in an aggregate is a `String<N>` rather
//! than a `&str`, so this shape can never be const encoded and always pays for
//! the runtime accessor.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    Ref.typeDemo.SEND_NAMES([
        StrTruncate::truncate("first"),
        StrTruncate::truncate("second"),
    ]);
}

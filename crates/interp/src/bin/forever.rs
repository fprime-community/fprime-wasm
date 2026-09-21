//! Never returns. Only the instruction limit stops it.

#![no_std]
#![no_main]

use interp::*;

#[fprime_main]
pub fn main() {
    // `black_box` gives the loop an effect LLVM cannot see through, so it is not
    // replaced with `unreachable`.
    loop {
        core::hint::black_box(0u32);
    }
}

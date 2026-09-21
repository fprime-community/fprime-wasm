//! Asks for more memory than any pool will serve. The refusal must not trap it.

#![no_std]
#![no_main]

use interp::*;

#[fprime_main]
pub fn main() {
    // A refused `memory.grow` yields -1 rather than trapping; the sequence carries on.
    let previous = core::arch::wasm32::memory_grow(0, HUNGRY_PAGES);
    core::hint::black_box(previous);
}

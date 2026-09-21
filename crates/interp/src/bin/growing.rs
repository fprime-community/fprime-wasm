//! Grows linear memory by a little, which the pool has room for.

#![no_std]
#![no_main]

use interp::*;

#[fprime_main]
pub fn main() {
    // `memory_grow`'s memory index is a legacy const generic, hence the positional 0.
    let previous = core::arch::wasm32::memory_grow(0, GROW_PAGES);
    core::hint::black_box(previous);
}

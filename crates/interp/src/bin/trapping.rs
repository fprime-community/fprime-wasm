//! Stores well past the end of linear memory, which traps.

#![no_std]
#![no_main]

use interp::*;

#[fprime_main]
pub fn main() {
    // Volatile so the store survives optimisation, and an address no guest pool could
    // ever cover so the trap does not depend on how much memory this module declares.
    unsafe { core::ptr::write_volatile(0x7FFF_FFF0u32 as *mut u8, 1) };
}

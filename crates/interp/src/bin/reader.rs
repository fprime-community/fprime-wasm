//! Reads one telemetry channel and nothing else.

#![no_std]
#![no_main]

use interp::*;

#[fprime_main]
pub fn main() {
    let (dropped, _) = CdhCore.events.EventsDropped();
    // The value is never used, so without this the read itself could be optimised away.
    core::hint::black_box(dropped);
}

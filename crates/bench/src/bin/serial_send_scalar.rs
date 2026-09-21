//! One scalar send. The fixed cost of an output port plus the `send` path.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let outbox = serial_out!(u32, 0);

    outbox.send(7);
}

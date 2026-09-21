//! A string argument that is not a literal.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let busy = CdhCore.events.EventsDropped().0 > 0;
    CdhCore
        .cmdDisp
        .CMD_NO_OP_STRING(if busy { "busy" } else { "idle" });
}

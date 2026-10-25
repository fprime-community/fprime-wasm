//! A sequence in restrict mode: plain steps, values and `if`, nothing more.

#![no_std]
#![no_main]

use example::*;

#[fprime_main(restrict)]
pub fn main() {
    const DROP_LIMIT: u64 = 2;

    set_fail_mode(FailMode::Permissive);

    let (dropped, _) = Tlm::CdhCore.events.EventsDropped();

    if dropped > DROP_LIMIT {
        CdhCore.cmdDisp.CMD_NO_OP_STRING("TOO MANY DROPPED");
        Ref.dpDemo.SelectColor(RED);
    } else {
        Ref.dpDemo.SelectColor(GREEN);
    }

    // Restrict mode rejects loops, so this would not compile:
    //
    // loop {
    //     CdhCore.cmdDisp.CMD_NO_OP();
    // }
    //
    // error: loops are not allowed in restrict mode
}

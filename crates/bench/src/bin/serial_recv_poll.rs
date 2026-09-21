//! The polling receive, against [`serial_recv_block`]: what the `Option` and the
//! status check cost over a blocking one.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let inbox = serial_in!(u32, 0);

    match inbox.recv_poll() {
        Some(request) if request > 2 => {
            CdhCore.cmdDisp.CMD_NO_OP();
        }
        _ => rsleep(100_000),
    }
}

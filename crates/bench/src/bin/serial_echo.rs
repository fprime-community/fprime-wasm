//! Receive, act, reply: the shape of the `worker` example without its loop, and
//! the smallest thing a request-handling sequence can be.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let inbox = serial_in!(u32, 0);
    let outbox = serial_out!(u32, 0);

    let request = inbox.recv_block();
    CdhCore.cmdDisp.CMD_NO_OP();
    outbox.send(request + 1);
}

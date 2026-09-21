//! A long-running sequence that handles queue messages

#![no_std]
#![no_main]

use example::*;

#[fprime_main]
pub fn main() {
    set_fail_mode(FailMode::Permissive);

    let inbox = serial_in!(u32, 1);
    let outbox = serial_out!(u32, 1);

    loop {
        match inbox.recv_poll() {
            Some(request) => {
                CdhCore.cmdDisp.CMD_NO_OP();
                outbox.send(request);
            }
            // Nothing to do: wait a little and look again.
            None => rsleep(100_000),
        }
    }
}

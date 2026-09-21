//! Four input ports of the same type, against [`serial_recv_block`]: the marginal
//! cost of one more port once the receive path is already paid for.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let a = serial_in!(u32, 0);
    let b = serial_in!(u32, 1);
    let c = serial_in!(u32, 2);
    let d = serial_in!(u32, 3);

    if a.recv_block() + b.recv_block() + c.recv_block() + d.recv_block() > 2 {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

//! One blocking receive of a scalar. The fixed cost of an input port plus the
//! `recv_block` path.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let inbox = serial_in!(u32, 0);

    if inbox.recv_block() > 2 {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

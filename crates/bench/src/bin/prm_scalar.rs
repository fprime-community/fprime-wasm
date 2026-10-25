//! A scalar parameter read.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    if Prm::Ref.recvBuffComp.parameter1() > 0 {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

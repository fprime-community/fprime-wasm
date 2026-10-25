//! A structure telemetry read.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let (scalars, _) = Tlm::Ref.typeDemo.ScalarStructCh();
    if scalars.u32 > 0 {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

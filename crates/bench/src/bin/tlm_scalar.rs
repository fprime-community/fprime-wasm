//! One scalar telemetry read, value and time both used.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let (value, time) = Tlm::Ref.typeDemo.ScalarU32Ch();
    if value > 2 && time.seconds > 1 {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

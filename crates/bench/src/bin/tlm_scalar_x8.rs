//! Eight scalar telemetry reads, for the marginal per-channel cost.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let total = Tlm::Ref.typeDemo.ScalarU8Ch().0 as u64
        + Tlm::Ref.typeDemo.ScalarU16Ch().0 as u64
        + Tlm::Ref.typeDemo.ScalarU32Ch().0 as u64
        + Tlm::Ref.typeDemo.ScalarU64Ch().0
        + Tlm::Ref.typeDemo.ScalarI8Ch().0 as u64
        + Tlm::Ref.typeDemo.ScalarI16Ch().0 as u64
        + Tlm::Ref.typeDemo.ScalarI32Ch().0 as u64
        + Tlm::Ref.typeDemo.ScalarI64Ch().0 as u64;

    if total > 0 {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

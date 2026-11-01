//! A string telemetry read, which deserializes into a `String<N>`.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let (name, _) = Tlm::Ref.typeDemo.NameCh();
    if !name.is_empty() {
        CdhCore.cmdDisp.CMD_NO_OP();
    }
}

//! A scalar argument that is not a constant, so the call keeps the runtime
//! accessor instead of being const encoded.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let priority = Tlm::CdhCore.cmdDisp.CommandsDispatched().0;
    Ref.dpDemo.Dp(IMMEDIATE, priority, PROC_TYPE_NONE);
}

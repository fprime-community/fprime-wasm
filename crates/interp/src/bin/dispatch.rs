//! Sends exactly one command, so the recording has exactly one thing in it.

#![no_std]
#![no_main]

use interp::*;

#[fprime_main]
pub fn main() {
    // Permissive: whether the host accepts the command is the test's business, not this
    // sequence's, and a refusal must not cut the run short.
    set_fail_mode(FailMode::Permissive);

    Ref.wasmSeq.LOAD("helloworld");
}

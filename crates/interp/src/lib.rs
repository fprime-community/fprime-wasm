//! Fixtures for the interpreter harness.
//!
//! Each bin here is the smallest real sequence that reaches one behaviour
//! `fprime_test::interpreter` has to report: a trap, an endless loop, a grow, a telemetry
//! read. They are not examples of how to write a sequence — `crates/example` is that.
#![no_std]
#![allow(nonstandard_style)]
#![allow(dead_code)]
#![allow(unused_imports)]

include!(concat!(env!("OUT_DIR"), "/dictionary.rs"));

/// Pages the `growing` fixture asks for. `--page-size=1` is set for the whole workspace, so
/// this is also the number of bytes the peak must come out above the declared size.
pub const GROW_PAGES: usize = 8;

/// Pages the `hungry` fixture asks for: above `Limits::guest_memory` by more than any test
/// would raise it, so the pool has to refuse.
pub const HUNGRY_PAGES: usize = 100_000;

// For the tests, which name dictionary points but do not build for Wasm.
#[cfg(not(target_family = "wasm"))]
include!(concat!(env!("OUT_DIR"), "/descriptors.rs"));

pub use Defs::*;
pub use fprime_core::*;

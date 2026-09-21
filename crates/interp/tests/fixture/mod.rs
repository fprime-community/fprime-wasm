//! Loading the fixture modules this crate's bins compile to.
//!
//! `cargo test -p interp` needs them built first, which `fprime-wasm test` does and CI does
//! explicitly. Shared by every test file here, so not everything is used by each one.
#![allow(dead_code)]

use fprime_test::interpreter;
use fprime_test::interpreter::{Limits, Report};
use fprime_test::wasm;
use std::path::Path;

/// The compiled `sequence.wasm`, as bytes.
pub fn module(sequence: &str) -> Vec<u8> {
    fprime_test::locate::module(Path::new(env!("CARGO_MANIFEST_DIR")), sequence)
        .unwrap_or_else(|err| panic!("could not load the `{sequence}` fixture: {err:#}"))
        .1
}

/// Linear memory the module declares, read statically — the figure a run must agree with,
/// and which the linker rather than this test chooses.
pub fn declared_memory(bytes: &[u8]) -> u64 {
    wasm::read(bytes)
        .expect("a linked module should parse")
        .memory
        .expect("a memory section")
        .initial_bytes()
}

/// One fixture, run against `limits` with no script.
pub fn run_with(sequence: &str, limits: &Limits) -> Report {
    interpreter::run(module(sequence), limits, None)
        .unwrap_or_else(|err| panic!("`{sequence}` should have run: {err:#}"))
}

/// [`run_with`], against the on-board defaults.
pub fn run(sequence: &str) -> Report {
    run_with(sequence, &Limits::default())
}

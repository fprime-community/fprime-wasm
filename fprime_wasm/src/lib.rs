//! Creating and inspecting F Prime Wasm sequence projects. The command itself is in
//! `src/cmd/`.
//!
//! * [`abi`] — the `fprime_v1` host interface, shared with `Svc::WasmSequencer`
//! * [`harness`] — a `spacewasm` interpreter standing in for the on-board one
//! * [`config`] — `sequencer.toml`, the limits it is held to
//! * [`verify`] — its measurements, as budgets, tables and JSON
//! * [`wasm`] — a static read of a module, for what `spacewasm` discards
//! * [`scaffold`], [`project`] — generating a sequence crate, and editing one
//!
//! [`harness`] installs `spacewasm`'s process-global allocator, which is
//! single-threaded by construction: nothing that runs the interpreter may run
//! alongside anything else that does. Hence one integration-test binary rather than
//! parallel unit tests.

pub mod abi;
pub mod config;
pub mod harness;
pub mod project;
pub mod scaffold;
pub mod verify;
pub mod wasm;

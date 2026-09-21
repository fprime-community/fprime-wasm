//! Creating and inspecting F Prime Wasm sequence projects. The command itself is in
//! `src/cmd/`.
//!
//! * [`verify`] — loading a module on the on-board interpreter, as budgets, tables and JSON
//! * [`scaffold`] — generating a sequence crate, and editing one
//!

pub mod scaffold;
pub mod verify;

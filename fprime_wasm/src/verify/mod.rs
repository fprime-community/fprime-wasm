//! Sizing a sequence against `Svc::WasmSequencer::Config`.
//!
//! [`fprime_test::interpreter::validate`] loads a module and reports what that cost; this
//! turns the figures into budgets and presents them. Nothing is executed — for what a
//! sequence *does*, `fprime-wasm test` runs it.

#[cfg(test)]
mod fixture;
mod measure;
mod render;
pub mod report;
mod table;

pub use measure::{Budget, Verified, verify};
pub use render::{detail, limits_line, summary};

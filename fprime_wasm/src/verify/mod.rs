//! Sizing a sequence against `Svc::WasmSequencer::Config`.
//!
//! [`crate::harness`] measures; this turns the figures into budgets and presents
//! them.

mod describe;
mod measure;
mod render;
pub mod report;
mod table;

pub use measure::{Budget, Verified, discover, verify};
pub use render::{detail, info_count, issues, limits_line, summary};

pub(crate) use describe::{
    channel_name, command_name, describe_call, describe_outcome, parameter_name,
};

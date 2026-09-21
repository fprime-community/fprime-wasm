//! Testing F Prime Wasm sequences.
//!
//! ```ignore
//! use fprime_test::*;
//! use sequences::*;
//!
//! #[fprime_test(sequence = "safing")]
//! fn retries_power_off_once(t: Test) {
//!     t.initial_telemetry(Ref.power.BatteryVoltage, 21.5);
//!
//!     t.expect_command(Ref.power.PWR_OFF()).responds(EXECUTION_ERROR);
//!     t.expect_command(Ref.power.PWR_OFF()).responds(OK).sets_telemetry(Ref.power.State, OFF);
//!     t.expect_event_containing("safe mode");
//!     t.expect_exit(0);
//! }
//! ```
//!
//! Listed steps must occur in order; the sequence may do other things in between.
//! `t.never_command(..)` (and the other `never_*` methods) assert something must not happen at
//! all.
//!
//! Underneath the DSL is the host side of a sequence run, which `fprime-wasm` shares:
//!
//! * [`abi`] — the `fprime_v1` host interface, shared with `Svc::WasmSequencer`
//! * [`interpreter`] — a `spacewasm` interpreter standing in for the on-board one, loading and
//!   running one module at a time
//! * [`config`] — `sequencer.toml`, the limits a run is held to
//! * [`wasm`] — a static read of a module, for what `spacewasm` discards
//! * [`project`] — the sequence crate, and where its modules land
//! * [`describe`] — recorded calls and outcomes, as words
//!
//! [`interpreter`] installs `spacewasm`'s process-global allocator, which is single-threaded by
//! construction: nothing that loads a module may run alongside anything else that does, and
//! holds the lock that enforces that.

pub mod abi;
pub mod case;
pub mod config;
pub mod describe;
pub mod interpreter;
pub mod locate;
pub(crate) mod plan;
pub mod project;
pub(crate) mod report;
pub mod step;
pub mod test;
pub mod wasm;

pub use case::{Case, run};
pub use step::{
    CmdMatch, IntoCommand, SerialRecvMatch, SerialSendMatch, SleepMatch, cmd, serial_recv,
    serial_send, sleep,
};
pub use test::{Pending, Test};

/// A named `Fw::CmdResponse`, and a named event severity.
pub use fprime_core::desc::{Response, Severity};

/// The limits a test runs against, and how a run can end.
pub use interpreter::{Limits, Outcome};

/// Event severities, as `Svc::WasmSequencer` classifies them.
pub mod severity {
    use fprime_core::desc::Severity;

    pub const FATAL: Severity = Severity::new(1, "FATAL");
    pub const WARNING_HI: Severity = Severity::new(2, "WARNING_HI");
    pub const WARNING_LO: Severity = Severity::new(3, "WARNING_LO");
    pub const COMMAND: Severity = Severity::new(4, "COMMAND");
    pub const ACTIVITY_HI: Severity = Severity::new(5, "ACTIVITY_HI");
    pub const ACTIVITY_LO: Severity = Severity::new(6, "ACTIVITY_LO");
    pub const DIAGNOSTIC: Severity = Severity::new(7, "DIAGNOSTIC");
}

#[cfg(test)]
mod tests {
    #[test]
    fn severities_match_abi() {
        for severity in [
            super::severity::FATAL,
            super::severity::WARNING_HI,
            super::severity::WARNING_LO,
            super::severity::COMMAND,
            super::severity::ACTIVITY_HI,
            super::severity::ACTIVITY_LO,
            super::severity::DIAGNOSTIC,
        ] {
            assert_eq!(
                crate::abi::severity(severity.code).name(),
                severity.name,
                "severity {} is named differently by the ABI",
                severity.code
            );
        }
    }
}

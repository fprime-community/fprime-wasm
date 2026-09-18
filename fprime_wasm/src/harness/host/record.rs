//! What the guest did, and what the sequencer would have done differently.

use super::Responses;
use std::collections::BTreeMap;

/// How the guest stopped calling into the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// `fprime_v1.exit`, the nominal end of a sequence.
    Exit(i32),
    /// `fprime_v1.panic`, raised by `fprime_core`'s panic handler or by a checked
    /// command failure.
    Panic(i32),
}

/// One host call the guest made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Exit {
        code: i32,
    },
    Panic {
        code: i32,
    },
    Args {
        capacity: u32,
        written: u32,
    },
    Time {
        len: u32,
    },
    Telemetry {
        id: i64,
        value_len: u32,
        status: i32,
    },
    Parameter {
        id: i64,
        value_len: u32,
        status: i32,
    },
    Command {
        opcode: u32,
        payload: Vec<u8>,
        response: i32,
    },
    Event {
        severity: i32,
        message: String,
        truncated: bool,
    },
    RelativeSleep {
        us: u64,
    },
    AbsoluteSleep {
        us: u64,
    },
    SerialSend {
        index: i32,
        len: u32,
    },
    SerialRecv {
        index: i32,
        blocking: bool,
        received: u32,
        status: i32,
    },
}

/// How much an issue matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The sequencer would behave differently on board from what the guest asked,
    /// and the guest cannot see it. Always reported.
    Warning,
    /// A statement about how `verify` fed the sequence rather than about the
    /// sequence — chiefly that an unsupplied telemetry channel or parameter read as
    /// zero. Useful when interpreting one module, but it fires on almost every run,
    /// so it is only reported when asked for.
    Info,
}

/// Something the sequencer would do differently from what the guest asked, or that
/// `verify` cannot model. None of these stop the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issues {
    /// Index into [`Recording::calls`] of the call that produced it.
    pub call: usize,
    pub kind: Kind,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Recording {
    pub calls: Vec<Call>,
    pub issues: Vec<Issues>,
    pub stop: Option<Stop>,
    pub(super) responses: Responses,
    /// Serial messages already delivered, so a port yields its canned message once
    /// rather than on every poll.
    pub(super) serial_drained: BTreeMap<i32, bool>,
}

impl Recording {
    pub(super) fn new(responses: Responses) -> Self {
        Recording {
            responses,
            ..Recording::default()
        }
    }

    pub(super) fn push(&mut self, call: Call) {
        self.calls.push(call);
    }

    pub(super) fn warn(&mut self, message: impl Into<String>) {
        self.issue(Kind::Warning, message);
    }

    /// Note how `verify` fed the sequence, rather than something it did wrong.
    pub(super) fn inform(&mut self, message: impl Into<String>) {
        self.issue(Kind::Info, message);
    }

    fn issue(&mut self, kind: Kind, message: impl Into<String>) {
        let call = self.calls.len().saturating_sub(1);
        self.issues.push(Issues {
            call,
            kind,
            message: message.into(),
        });
    }

    /// Issues of one kind, in the order they were recorded.
    pub fn issues_of(&self, kind: Kind) -> impl Iterator<Item = &Issues> {
        self.issues.iter().filter(move |issue| issue.kind == kind)
    }

    /// Commands the guest dispatched, in order, as `(opcode, payload)`.
    pub fn commands(&self) -> impl Iterator<Item = (u32, &[u8])> {
        self.calls.iter().filter_map(|call| match call {
            Call::Command {
                opcode, payload, ..
            } => Some((*opcode, payload.as_slice())),
            _ => None,
        })
    }

    /// Telemetry channel ids the guest read, deduplicated and sorted.
    pub fn telemetry_read(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .calls
            .iter()
            .filter_map(|call| match call {
                Call::Telemetry { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Parameter ids the guest read, deduplicated and sorted.
    pub fn parameters_read(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .calls
            .iter()
            .filter_map(|call| match call {
                Call::Parameter { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Total microseconds the sequence asked to sleep for. Absolute sleeps are
    /// excluded: their duration depends on the wall clock, not the sequence.
    pub fn relative_sleep_us(&self) -> u64 {
        self.calls
            .iter()
            .filter_map(|call| match call {
                Call::RelativeSleep { us } => Some(*us),
                _ => None,
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(calls: Vec<Call>) -> Recording {
        Recording {
            calls,
            ..Recording::default()
        }
    }

    #[test]
    fn summarises_commands_in_order() {
        let recording = recorded(vec![
            Call::Command {
                opcode: 0x10007002,
                payload: vec![1, 2],
                response: 0,
            },
            Call::Time { len: 11 },
            Call::Command {
                opcode: 0x200,
                payload: vec![],
                response: 0,
            },
        ]);
        let commands: Vec<_> = recording
            .commands()
            .map(|(opcode, payload)| (opcode, payload.to_vec()))
            .collect();
        assert_eq!(commands, vec![(0x10007002, vec![1, 2]), (0x200, vec![])]);
    }

    #[test]
    fn deduplicates_and_sorts_channel_and_parameter_ids() {
        let recording = recorded(vec![
            Call::Telemetry {
                id: 9,
                value_len: 4,
                status: 0,
            },
            Call::Telemetry {
                id: 2,
                value_len: 4,
                status: 0,
            },
            Call::Telemetry {
                id: 9,
                value_len: 4,
                status: 0,
            },
            Call::Parameter {
                id: 5,
                value_len: 4,
                status: 1,
            },
        ]);
        assert_eq!(recording.telemetry_read(), vec![2, 9]);
        assert_eq!(recording.parameters_read(), vec![5]);
    }

    /// Absolute sleeps are wall-clock and must not be added into a duration the
    /// sequence is said to spend sleeping.
    #[test]
    fn totals_relative_sleeps_only() {
        let recording = recorded(vec![
            Call::RelativeSleep { us: 1_500 },
            Call::AbsoluteSleep { us: 999_999_999 },
            Call::RelativeSleep { us: 500 },
        ]);
        assert_eq!(recording.relative_sleep_us(), 2_000);
    }

    #[test]
    fn a_finding_points_at_the_call_that_caused_it() {
        let mut recording = recorded(vec![Call::Time { len: 11 }]);
        recording.push(Call::Event {
            severity: 1,
            message: "no".into(),
            truncated: false,
        });
        recording.warn("forbidden severity");
        assert_eq!(recording.issues[0].call, 1);
    }

    /// An issue is also reachable before any call has been recorded; it must not
    /// underflow the index.
    #[test]
    fn a_finding_with_no_calls_yet_is_index_zero() {
        let mut recording = Recording::default();
        recording.warn("early");
        assert_eq!(recording.issues[0].call, 0);
    }
}

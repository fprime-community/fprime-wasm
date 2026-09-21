//! Builders for the things a sequence test can check: a command, an event, a sleep, a
//! telemetry or parameter read, a serial send or receive.
//!
//! ```ignore
//! t.expect_command(Ref.power.PWR_OFF());
//! t.expect_event_containing("safe mode");
//! t.expect_sleep(sleep().of_secs(1.0));
//! ```
//!
//! Each builder narrows what counts as a match — a bare `cmd(0x10)` matches that opcode with
//! any arguments, `cmd(0x10).args(&[0x01])` only that exact payload.

use crate::interpreter::Call;
use fprime_core::desc::{Cmd, CmdDesc, Response};
use std::panic::Location;

/// Where in the test file a step was written, for the failure message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Where {
    pub file: &'static str,
    pub line: u32,
}

impl Where {
    #[track_caller]
    pub(crate) fn here() -> Self {
        let caller = Location::caller();
        Where {
            file: caller.file(),
            line: caller.line(),
        }
    }
}

impl std::fmt::Display for Where {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.file, self.line)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Args {
    /// Any arguments at all — what a bare `Ref.power.PWR_OFF` means.
    Any,
    /// Exactly these bytes — what `Ref.power.PWR_OFF()` means.
    Exact(Vec<u8>),
    /// These bytes at the front, for a command whose tail a test does not care about.
    Prefix(Vec<u8>),
}

impl Args {
    fn accepts(&self, payload: &[u8]) -> bool {
        match self {
            Args::Any => true,
            Args::Exact(bytes) => payload == bytes.as_slice(),
            Args::Prefix(bytes) => payload.starts_with(bytes),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matcher {
    Command {
        opcode: u32,
        /// The dictionary name, when the step came from a descriptor. `None` for a
        /// hand-written opcode, where the failure message falls back to the dictionary.
        path: Option<&'static str>,
        args: Args,
    },
    Event {
        contains: Option<String>,
        exactly: Option<String>,
    },
    Sleep {
        exact_us: Option<u64>,
        at_least_us: Option<u64>,
        /// `None` matches either kind.
        absolute: Option<bool>,
    },
    TelemetryRead {
        id: i64,
        path: &'static str,
    },
    ParameterRead {
        id: i64,
        path: &'static str,
    },
    SerialSend {
        port: i32,
        /// Exact bytes, when the test says what was sent.
        data: Option<Vec<u8>>,
        len: Option<u32>,
    },
    SerialRecv {
        port: i32,
        /// `None` matches either a blocking or a polling receive.
        blocking: Option<bool>,
        /// Whether the receive found a message. `None` matches either.
        received: Option<bool>,
    },
    ArgsRead,
    TimeRead,
}

impl Matcher {
    /// Whether `call` is the thing this step is waiting for. Blind to the command's
    /// *response*, which is what the test itself supplies.
    pub(crate) fn matches(&self, call: &Call) -> bool {
        match (self, call) {
            (
                Matcher::Command { opcode, args, .. },
                Call::Command {
                    opcode: seen,
                    payload,
                    ..
                },
            ) => opcode == seen && args.accepts(payload),

            (Matcher::Event { contains, exactly }, Call::Event { message, .. }) => {
                contains
                    .as_ref()
                    .is_none_or(|needle| message.contains(needle))
                    && exactly.as_ref().is_none_or(|whole| message == whole)
            }

            (
                Matcher::Sleep {
                    exact_us,
                    at_least_us,
                    absolute,
                },
                Call::RelativeSleep { us } | Call::AbsoluteSleep { us },
            ) => {
                let is_absolute = matches!(call, Call::AbsoluteSleep { .. });
                absolute.is_none_or(|wanted| wanted == is_absolute)
                    && exact_us.is_none_or(|wanted| wanted == *us)
                    && at_least_us.is_none_or(|least| *us >= least)
            }

            (Matcher::TelemetryRead { id, .. }, Call::Telemetry { id: seen, .. }) => id == seen,
            (Matcher::ParameterRead { id, .. }, Call::Parameter { id: seen, .. }) => id == seen,

            (
                Matcher::SerialSend { port, data, len },
                Call::SerialSend {
                    index,
                    len: seen_len,
                    payload,
                },
            ) => {
                port == index
                    && len.is_none_or(|wanted| wanted == *seen_len)
                    && data.as_ref().is_none_or(|bytes| bytes == payload)
            }

            (
                Matcher::SerialRecv {
                    port,
                    blocking,
                    received,
                },
                Call::SerialRecv {
                    index,
                    blocking: seen_blocking,
                    received: seen_received,
                    ..
                },
            ) => {
                port == index
                    && blocking.is_none_or(|wanted| wanted == *seen_blocking)
                    && received.is_none_or(|wanted| wanted == (*seen_received > 0))
            }

            (Matcher::ArgsRead, Call::Args { .. }) => true,
            (Matcher::TimeRead, Call::Time { .. }) => true,

            _ => false,
        }
    }

    /// How the step reads in a failure message, without a dictionary.
    pub(crate) fn describe(&self) -> String {
        match self {
            Matcher::Command { opcode, path, args } => {
                let named = path.map(|p| format!(" {p}")).unwrap_or_default();
                let arguments = match args {
                    Args::Any => " any arguments".to_string(),
                    Args::Exact(bytes) if bytes.is_empty() => " no arguments".to_string(),
                    Args::Exact(bytes) => format!("  [{}]", hex(bytes)),
                    Args::Prefix(bytes) => format!("  starting [{}]", hex(bytes)),
                };
                format!("cmd{named}  {opcode:#010x}{arguments}")
            }
            Matcher::Event { contains, exactly } => match (contains, exactly) {
                (Some(needle), _) => format!("event containing {needle:?}"),
                (_, Some(whole)) => format!("event {whole:?}"),
                _ => "any event".to_string(),
            },
            Matcher::Sleep {
                exact_us,
                at_least_us,
                absolute,
            } => {
                let kind = match absolute {
                    Some(true) => "absolute sleep",
                    Some(false) => "relative sleep",
                    None => "sleep",
                };
                match (exact_us, at_least_us) {
                    (Some(us), _) => format!("{kind} of {us} us"),
                    (_, Some(us)) => format!("{kind} of at least {us} us"),
                    _ => format!("any {kind}"),
                }
            }
            Matcher::TelemetryRead { path, id } => format!("read of {path} ({id})"),
            Matcher::ParameterRead { path, id } => format!("read of parameter {path} ({id})"),
            Matcher::SerialSend { port, data, len } => {
                let what = match (data, len) {
                    (Some(bytes), _) => format!(" of [{}]", hex(bytes)),
                    (_, Some(len)) => format!(" of {len} bytes"),
                    _ => String::new(),
                };
                format!("serial_send on port {port}{what}")
            }
            Matcher::SerialRecv {
                port,
                blocking,
                received,
            } => {
                let kind = match blocking {
                    Some(true) => "blocking ",
                    Some(false) => "polling ",
                    None => "",
                };
                let outcome = match received {
                    Some(true) => ", finding a message",
                    Some(false) => ", finding nothing",
                    None => "",
                };
                format!("{kind}serial_recv on port {port}{outcome}")
            }
            Matcher::ArgsRead => "the sequence reading its arguments".to_string(),
            Matcher::TimeRead => "the sequence reading the clock".to_string(),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the spacecraft does when a step fires.
#[derive(Debug, Clone)]
pub(crate) enum Effect {
    SetTelemetry {
        id: i64,
        wire: Vec<u8>,
    },
    SetParameter {
        id: i64,
        wire: Vec<u8>,
    },
    /// A message arriving on a serial input port because the sequence reached this step.
    QueueSerial {
        port: i32,
        message: Vec<u8>,
    },
}

/// One listed step.
pub struct Step {
    pub(crate) matcher: Matcher,
    /// The `Fw::CmdResponse` to answer a matched command with. `None` leaves the nominal OK.
    pub(crate) response: Option<Response>,
    pub(crate) effects: Vec<Effect>,
    /// A note the author attached, carried into the failure message.
    pub(crate) note: Option<&'static str>,
    pub(crate) at: Where,
}

impl Step {
    pub(crate) fn new(matcher: Matcher, at: Where) -> Self {
        Step {
            matcher,
            response: None,
            effects: Vec::new(),
            note: None,
            at,
        }
    }
}

/// A command, written any of these ways:
///
/// ```ignore
/// t.expect_command(Ref.power.PWR_OFF());  // this exact call
/// t.expect_command(Ref.power.PWR_OFF);    // this opcode, with any arguments
/// t.expect_command(cmd(0x1234));          // an opcode with no dictionary entry
/// ```
pub trait IntoCommand {
    #[doc(hidden)]
    fn into_matcher(self) -> Matcher;
}

/// A command with no arguments attached: this opcode, whatever it was sent with.
impl IntoCommand for CmdDesc {
    fn into_matcher(self) -> Matcher {
        Matcher::Command {
            opcode: self.opcode,
            path: Some(self.path),
            args: Args::Any,
        }
    }
}

/// A command with its exact `Fw::ComBuffer`, from the same const encoder the sequence uses.
impl IntoCommand for Cmd {
    fn into_matcher(self) -> Matcher {
        Matcher::Command {
            opcode: self.opcode(),
            path: Some(self.path),
            args: Args::Exact(self.args().to_vec()),
        }
    }
}

/// Any command with this opcode, for a command whose arguments cannot be written as
/// literals, or a test with no dictionary to hand.
///
/// ```ignore
/// t.expect_command(cmd(0x1234));                       // any arguments
/// t.expect_command(cmd(0x1234).args(&[0x01]));         // exactly these bytes
/// t.expect_command(cmd(0x1234).args_starting(&[0x01])); // these bytes, then anything
/// ```
pub fn cmd(opcode: u32) -> CmdMatch {
    CmdMatch {
        opcode,
        args: Args::Any,
    }
}

pub struct CmdMatch {
    opcode: u32,
    args: Args,
}

impl CmdMatch {
    /// Exactly these argument bytes, opcode excluded.
    pub fn args(mut self, bytes: &[u8]) -> Self {
        self.args = Args::Exact(bytes.to_vec());
        self
    }

    /// These argument bytes at the front.
    pub fn args_starting(mut self, bytes: &[u8]) -> Self {
        self.args = Args::Prefix(bytes.to_vec());
        self
    }
}

impl IntoCommand for CmdMatch {
    fn into_matcher(self) -> Matcher {
        Matcher::Command {
            opcode: self.opcode,
            path: None,
            args: self.args,
        }
    }
}

/// An event matcher, for building test fixtures directly against `Matcher` — not part of the
/// API an author writes. Use `Test::expect_event`, `expect_event_containing`, or
/// `expect_event_exactly` instead.
#[cfg(test)]
pub(crate) fn event() -> EventMatch {
    EventMatch {
        contains: None,
        exactly: None,
    }
}

#[cfg(test)]
pub(crate) struct EventMatch {
    contains: Option<String>,
    exactly: Option<String>,
}

#[cfg(test)]
impl EventMatch {
    pub(crate) fn containing(mut self, needle: &str) -> Self {
        self.contains = Some(needle.to_string());
        self
    }

    pub(crate) fn exactly(mut self, message: &str) -> Self {
        self.exactly = Some(message.to_string());
        self
    }

    pub(crate) fn into_matcher(self) -> Matcher {
        Matcher::Event {
            contains: self.contains,
            exactly: self.exactly,
        }
    }
}

/// Any sleep, relative or absolute.
///
/// ```ignore
/// t.expect_sleep(sleep());                     // any sleep at all
/// t.expect_sleep(sleep().of_secs(1.0));         // exactly one second
/// t.expect_sleep(sleep().at_least_secs(1.0).relative()); // at least a second, `rsleep` only
/// ```
pub fn sleep() -> SleepMatch {
    SleepMatch {
        exact_us: None,
        at_least_us: None,
        absolute: None,
    }
}

pub struct SleepMatch {
    exact_us: Option<u64>,
    at_least_us: Option<u64>,
    absolute: Option<bool>,
}

impl SleepMatch {
    pub fn of_us(mut self, us: u64) -> Self {
        self.exact_us = Some(us);
        self
    }

    pub fn at_least_us(mut self, us: u64) -> Self {
        self.at_least_us = Some(us);
        self
    }

    /// Seconds, for the durations a sequence actually waits.
    pub fn of_secs(self, secs: f64) -> Self {
        let us = (secs * 1_000_000.0) as u64;
        self.of_us(us)
    }

    pub fn at_least_secs(self, secs: f64) -> Self {
        let us = (secs * 1_000_000.0) as u64;
        self.at_least_us(us)
    }

    /// Only `asleep`, which waits until a time rather than for a duration.
    pub fn absolute(mut self) -> Self {
        self.absolute = Some(true);
        self
    }

    /// Only `rsleep`.
    pub fn relative(mut self) -> Self {
        self.absolute = Some(false);
        self
    }
}

impl SleepMatch {
    pub(crate) fn into_matcher(self) -> Matcher {
        Matcher::Sleep {
            exact_us: self.exact_us,
            at_least_us: self.at_least_us,
            absolute: self.absolute,
        }
    }
}

/// The guest sending on a serial output port.
///
/// ```ignore
/// t.expect_serial_send(serial_send(0));                 // any send on port 0
/// t.expect_serial_send(serial_send(0).of(42u32));        // this value
/// ```
pub fn serial_send(port: i32) -> SerialSendMatch {
    SerialSendMatch {
        port,
        data: None,
        len: None,
    }
}

pub struct SerialSendMatch {
    port: i32,
    data: Option<Vec<u8>>,
    len: Option<u32>,
}

impl SerialSendMatch {
    /// Exactly this value, for a port a sequence drives through `Sender<T, N>`.
    pub fn of<S: fprime_core::Serializable>(mut self, value: S) -> Self {
        let mut buffer = vec![0u8; S::SIZE];
        let mut offset = 0;
        value.serialize_to(&mut buffer, &mut offset);
        buffer.truncate(offset);
        self.data = Some(buffer);
        self
    }

    pub fn of_len(mut self, len: u32) -> Self {
        self.len = Some(len);
        self
    }
}

impl SerialSendMatch {
    pub(crate) fn into_matcher(self) -> Matcher {
        Matcher::SerialSend {
            port: self.port,
            data: self.data,
            len: self.len,
        }
    }
}

/// The guest receiving on a serial input port.
///
/// ```ignore
/// t.expect_serial_recv(serial_recv(0));                                // any receive
/// t.expect_serial_recv(serial_recv(0).blocking().finding_a_message()); // `block_recv`, a hit
/// t.expect_serial_recv(serial_recv(0).polling().finding_nothing());    // `recv`, empty queue
/// ```
pub fn serial_recv(port: i32) -> SerialRecvMatch {
    SerialRecvMatch {
        port,
        blocking: None,
        received: None,
    }
}

pub struct SerialRecvMatch {
    port: i32,
    blocking: Option<bool>,
    received: Option<bool>,
}

impl SerialRecvMatch {
    /// Only a blocking receive — `Queue::block_recv`, which waits on board.
    pub fn blocking(mut self) -> Self {
        self.blocking = Some(true);
        self
    }

    /// Only a polling receive — `Queue::recv`, which returns `None` on an empty queue.
    pub fn polling(mut self) -> Self {
        self.blocking = Some(false);
        self
    }

    /// A receive that found a message.
    pub fn finding_a_message(mut self) -> Self {
        self.received = Some(true);
        self
    }

    /// A receive that found nothing — what a polling loop does between messages.
    pub fn finding_nothing(mut self) -> Self {
        self.received = Some(false);
        self
    }
}

impl SerialRecvMatch {
    pub(crate) fn into_matcher(self) -> Matcher {
        Matcher::SerialRecv {
            port: self.port,
            blocking: self.blocking,
            received: self.received,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(opcode: u32, payload: &[u8]) -> Call {
        Call::Command {
            opcode,
            payload: payload.to_vec(),
            response: 0,
        }
    }

    #[test]
    fn bare_matches_any_args_encoded_does_not() {
        let bare = CmdDesc {
            opcode: 0x1000001,
            path: "CdhCore.cmdDisp.CMD_NO_OP_STRING",
        };
        let any = bare.into_matcher();
        assert!(any.matches(&command(0x1000001, &[0x00, 0x02, b'h', b'i'])));
        assert!(any.matches(&command(0x1000001, &[])));
        // A different opcode is never the same step.
        assert!(!any.matches(&command(0x1000002, &[])));

        let exact = bare
            .with(&[0x01, 0x00, 0x00, 0x01, 0x00, 0x02, b'h', b'i'])
            .into_matcher();
        assert!(exact.matches(&command(0x1000001, &[0x00, 0x02, b'h', b'i'])));
        assert!(
            !exact.matches(&command(0x1000001, &[0x00, 0x02, b'n', b'o'])),
            "different arguments are a different step"
        );
    }

    #[test]
    fn prefix_match_ignores_tail() {
        let step = cmd(0x10).args_starting(&[0xde, 0xad]).into_matcher();
        assert!(step.matches(&command(0x10, &[0xde, 0xad, 0xbe, 0xef])));
        assert!(step.matches(&command(0x10, &[0xde, 0xad])));
        assert!(!step.matches(&command(0x10, &[0xde])));
    }

    #[test]
    fn event_matches_substring_or_whole_message() {
        let call = Call::Event {
            severity: 5,
            message: "entering safe mode now".into(),
            truncated: false,
        };
        assert!(
            event()
                .containing("safe mode")
                .into_matcher()
                .matches(&call)
        );
        assert!(!event().containing("safing").into_matcher().matches(&call));
        assert!(
            event()
                .exactly("entering safe mode now")
                .into_matcher()
                .matches(&call)
        );
        assert!(!event().exactly("safe mode").into_matcher().matches(&call));
        // A bare `event()` matches any event at all.
        assert!(event().into_matcher().matches(&call));
    }

    #[test]
    fn sleep_matches_by_duration_and_kind() {
        let relative = Call::RelativeSleep { us: 5_000_000 };
        let absolute = Call::AbsoluteSleep { us: 5_000_000 };

        assert!(sleep().into_matcher().matches(&relative));
        assert!(sleep().into_matcher().matches(&absolute));
        assert!(sleep().relative().into_matcher().matches(&relative));
        assert!(!sleep().relative().into_matcher().matches(&absolute));
        assert!(sleep().of_secs(5.0).into_matcher().matches(&relative));
        assert!(!sleep().of_secs(4.0).into_matcher().matches(&relative));
        // "At least" is what a test wants when the sequence computes its own backoff.
        assert!(sleep().at_least_secs(1.0).into_matcher().matches(&relative));
        assert!(!sleep().at_least_secs(6.0).into_matcher().matches(&relative));
    }

    #[test]
    fn command_step_ignores_response() {
        let step = cmd(0x10).into_matcher();
        for response in [0, 4, 5] {
            assert!(step.matches(&Call::Command {
                opcode: 0x10,
                payload: vec![],
                response
            }));
        }
    }

    #[test]
    fn step_never_matches_other_kind() {
        let calls = [
            command(0x10, &[]),
            Call::Event {
                severity: 5,
                message: "x".into(),
                truncated: false,
            },
            Call::RelativeSleep { us: 1 },
            Call::Telemetry {
                id: 4,
                value_len: 4,
                status: 0,
            },
            Call::Parameter {
                id: 4,
                value_len: 4,
                status: 1,
            },
        ];
        let matchers = [
            cmd(0x10).into_matcher(),
            event().into_matcher(),
            sleep().into_matcher(),
            Matcher::TelemetryRead { id: 4, path: "c" },
            Matcher::ParameterRead { id: 4, path: "p" },
        ];

        for (i, matcher) in matchers.iter().enumerate() {
            for (j, call) in calls.iter().enumerate() {
                assert_eq!(
                    matcher.matches(call),
                    i == j,
                    "matcher {i} against call {j}: {}",
                    matcher.describe()
                );
            }
        }
    }

    #[test]
    fn serial_send_matches_on_payload_not_width() {
        let sent = Call::SerialSend {
            index: 0,
            len: 4,
            payload: vec![0, 0, 0, 2],
        };

        assert!(serial_send(0).into_matcher().matches(&sent));
        assert!(serial_send(0).of(2u32).into_matcher().matches(&sent));
        assert!(
            !serial_send(0).of(1u32).into_matcher().matches(&sent),
            "a different value of the same width is a different message"
        );
        // A width-only assertion is still available when that is the point.
        assert!(serial_send(0).of_len(4).into_matcher().matches(&sent));
        assert!(!serial_send(0).of_len(8).into_matcher().matches(&sent));
        // And a port is never confused with another.
        assert!(!serial_send(1).into_matcher().matches(&sent));
    }

    #[test]
    fn serial_receive_matches_on_kind_and_hit() {
        let blocking_hit = Call::SerialRecv {
            index: 0,
            blocking: true,
            received: 4,
            status: 0,
        };
        let polling_miss = Call::SerialRecv {
            index: 0,
            blocking: false,
            received: 0,
            status: 1,
        };

        assert!(serial_recv(0).into_matcher().matches(&blocking_hit));
        assert!(serial_recv(0).into_matcher().matches(&polling_miss));

        assert!(
            serial_recv(0)
                .blocking()
                .into_matcher()
                .matches(&blocking_hit)
        );
        assert!(
            !serial_recv(0)
                .blocking()
                .into_matcher()
                .matches(&polling_miss)
        );
        assert!(
            serial_recv(0)
                .polling()
                .into_matcher()
                .matches(&polling_miss)
        );

        assert!(
            serial_recv(0)
                .finding_a_message()
                .into_matcher()
                .matches(&blocking_hit)
        );
        assert!(
            serial_recv(0)
                .finding_nothing()
                .into_matcher()
                .matches(&polling_miss)
        );
        assert!(
            !serial_recv(0)
                .finding_nothing()
                .into_matcher()
                .matches(&blocking_hit)
        );
    }

    #[test]
    fn args_and_clock_are_matchable() {
        let args = Call::Args {
            capacity: 8,
            written: 4,
        };
        let time = Call::Time { len: 11 };

        assert!(Matcher::ArgsRead.matches(&args));
        assert!(!Matcher::ArgsRead.matches(&time));
        assert!(Matcher::TimeRead.matches(&time));
        assert!(!Matcher::TimeRead.matches(&args));
    }

    #[test]
    fn every_matcher_describes_itself() {
        let described = [
            cmd(0x10).into_matcher().describe(),
            cmd(0x10).args(&[0xab]).into_matcher().describe(),
            event().containing("x").into_matcher().describe(),
            sleep().of_us(5).into_matcher().describe(),
            Matcher::TelemetryRead { id: 4, path: "c" }.describe(),
        ];
        for text in &described {
            assert!(!text.is_empty());
        }
        assert!(described[0].contains("any arguments"), "{}", described[0]);
        assert!(described[1].contains("ab"), "{}", described[1]);

        // The surfaces added for the long-running case render too, or a failure on one of them
        // would show an empty step.
        for text in [
            serial_send(0).of(1u8).into_matcher().describe(),
            serial_recv(1)
                .blocking()
                .finding_nothing()
                .into_matcher()
                .describe(),
            Matcher::ArgsRead.describe(),
            Matcher::TimeRead.describe(),
        ] {
            assert!(!text.is_empty());
        }
        assert!(
            serial_recv(1)
                .blocking()
                .finding_nothing()
                .into_matcher()
                .describe()
                .contains("port 1")
        );
    }
}

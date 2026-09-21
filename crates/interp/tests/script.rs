//! Scripted runs: the host a sequence test installs.
//!
//! Every test goes through [`fprime_test::interpreter`], which holds the lock that keeps two
//! modules out of `spacewasm`'s single-threaded allocator at once — so libtest's threads
//! queue here rather than running concurrently.

mod fixture;

use fprime_test::interpreter;
use fprime_test::interpreter::{Call, Kind, Limits, Report, Script};
use std::cell::RefCell;
use std::rc::Rc;

/// Records what it's asked and answers from a fixed plan.
#[derive(Default)]
struct Scripted {
    /// Response to give each command, in order; `None` falls through to the nominal OK.
    responses: Vec<Option<i32>>,
    commands: usize,
    /// Values to serve for successive reads of any channel, last repeating.
    telemetry: Vec<Vec<u8>>,
    reads: usize,
    clock_us: Option<u64>,
}

impl Script for Scripted {
    fn command(&mut self, _opcode: u32, _payload: &[u8]) -> Option<i32> {
        let response = self.responses.get(self.commands).copied().flatten();
        self.commands += 1;
        response
    }

    fn telemetry(&mut self, _id: i64) -> Option<Vec<u8>> {
        if self.telemetry.is_empty() {
            return None;
        }
        let index = self.reads.min(self.telemetry.len() - 1);
        self.reads += 1;
        Some(self.telemetry[index].clone())
    }

    fn now_us(&mut self) -> Option<u64> {
        self.clock_us
    }
}

/// Runs a fixture with a script installed; returns the report and the script.
fn run_scripted(sequence: &str, script: Scripted) -> (Report, Rc<RefCell<Scripted>>) {
    let script = Rc::new(RefCell::new(script));
    let report = interpreter::run(
        fixture::module(sequence),
        &Limits::default(),
        Some(script.clone()),
    )
    .unwrap_or_else(|err| panic!("`{sequence}` should have run: {err:#}"));
    (report, script)
}

#[test]
fn script_refuses_command() {
    let (report, script) = run_scripted(
        "dispatch",
        Scripted {
            // 4 is Fw::CmdResponse::EXECUTION_ERROR.
            responses: vec![Some(4)],
            ..Scripted::default()
        },
    );

    let recorded: Vec<i32> = report
        .recording
        .calls
        .iter()
        .filter_map(|call| match call {
            Call::Command { response, .. } => Some(*response),
            _ => None,
        })
        .collect();
    assert_eq!(recorded, vec![4], "the scripted response must be recorded");
    assert_eq!(script.borrow().commands, 1, "the script must be consulted");
}

/// A script is the only source of a telemetry value.
#[test]
fn script_answers_telemetry() {
    // `EventsDropped` is `FwSizeType`, so eight big-endian bytes.
    let (report, script) = run_scripted(
        "reader",
        Scripted {
            telemetry: vec![vec![0, 0, 0, 0, 0, 0, 0, 7]],
            ..Scripted::default()
        },
    );

    assert!(report.outcome.is_nominal(), "{:?}", report.outcome);
    assert_eq!(script.borrow().reads, 1, "the script must be asked");
    // A supplied value means no zero-fill note.
    assert_eq!(report.recording.issues_of(Kind::Info).count(), 0);
}

/// A script may supply a clock; otherwise `time` reads zero.
#[test]
fn script_gives_guest_clock() {
    let (report, _) = run_scripted(
        "nominal",
        Scripted {
            clock_us: Some(3_500_000),
            ..Scripted::default()
        },
    );
    assert!(report.outcome.is_nominal(), "{:?}", report.outcome);
}

/// A script that declines everything must behave exactly as no script at all.
#[test]
fn declining_script_matches_no_script() {
    let (scripted, _) = run_scripted("dispatch", Scripted::default());
    let plain = fixture::run("dispatch");

    assert_eq!(scripted.recording.calls, plain.recording.calls);
    assert_eq!(scripted.outcome, plain.outcome);
    assert_eq!(scripted.instructions, plain.instructions);
    assert!(
        matches!(
            plain.recording.calls.first(),
            Some(Call::Command { response: 0, .. })
        ),
        "{:?}",
        plain.recording.calls
    );
}

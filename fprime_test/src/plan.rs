//! What a test declared, and the subsequence walk over what the sequence did.
//!
//! [`Plan`] implements [`Script`] to drive the run live, then [`walk`] re-derives the same
//! match over the recording afterwards, using the same [`Matcher::matches`].

use crate::interpreter::{Call, Outcome, Script};
use crate::step::{Effect, Matcher, Step};
use std::collections::BTreeMap;

/// Successive values for one point, the last repeating.
#[derive(Debug, Clone)]
struct Queue {
    values: Vec<Vec<u8>>,
    taken: usize,
}

impl Queue {
    fn of(values: Vec<Vec<u8>>) -> Self {
        Queue { values, taken: 0 }
    }

    fn next(&mut self) -> Option<Vec<u8>> {
        if self.values.is_empty() {
            return None;
        }
        let index = self.taken.min(self.values.len() - 1);
        self.taken += 1;
        Some(self.values[index].clone())
    }

    /// How many times this point was read, for the "what this test supplied" block.
    fn reads(&self) -> usize {
        self.taken
    }
}

/// The outcome a test requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expected {
    /// A clean exit or a plain return from `main`. The default.
    Nominal,
    Exit(i32),
    Panic(i32),
    AnyPanic,
    /// The sequence had not finished when the run ended — accepts `Suspended` only, not
    /// `OutOfInstructions`.
    StillRunning,
    Exactly(Outcome),
}

impl Expected {
    pub(crate) fn accepts(&self, outcome: &Outcome) -> bool {
        match self {
            Expected::Nominal => outcome.is_nominal(),
            Expected::Exit(code) => matches!(outcome, Outcome::Exited(seen) if seen == code),
            Expected::Panic(code) => matches!(outcome, Outcome::Panicked(seen) if seen == code),
            Expected::AnyPanic => matches!(outcome, Outcome::Panicked(_)),
            Expected::StillRunning => matches!(outcome, Outcome::Suspended),
            Expected::Exactly(wanted) => wanted == outcome,
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Expected::Nominal => "a nominal outcome (exit 0, or a return from `main`)".into(),
            Expected::Exit(code) => format!("exit({code})"),
            Expected::Panic(code) => format!("panic({code})"),
            Expected::AnyPanic => "a panic".into(),
            Expected::StillRunning => "the sequence still running".into(),
            Expected::Exactly(outcome) => format!("{outcome:?}"),
        }
    }
}

/// A supplied point, for the "what this test supplied" block of a failure message.
#[derive(Debug, Clone)]
pub(crate) struct Supplied {
    pub path: &'static str,
    pub shown: String,
    pub reads: usize,
    pub is_parameter: bool,
}

#[derive(Default)]
pub(crate) struct Context {
    pub(crate) steps: Vec<Step>,
    /// Checked after the run, not during.
    pub(crate) forbidden: Vec<Step>,
    pub(crate) outcome: Option<Expected>,

    /// Cursor into `steps`, advanced during the run.
    cursor: usize,
    telemetry: BTreeMap<i64, Queue>,
    parameters: BTreeMap<i64, Queue>,
    /// What each point was supplied as, in declaration order, for the report.
    supplied: Vec<(i64, Supplied)>,
    /// Advanced by the guest's own sleeps, and reported to `time`.
    clock_us: u64,

    /// Limits to run against, replacing the project's `sequencer.toml` wholesale.
    pub(crate) limits: Option<crate::interpreter::Limits>,
    /// Just the instruction ceiling.
    pub(crate) max_instructions: Option<u64>,

    /// Messages waiting on each serial input port, delivered one per receive.
    serial: BTreeMap<i32, Vec<Vec<u8>>>,
    /// How many messages each port has delivered, for the report.
    serial_taken: BTreeMap<i32, usize>,
    /// Arguments the sequence was invoked with.
    args: Option<Vec<u8>>,

    /// Stop the run once every listed step has matched — for a sequence that never parks on
    /// its own.
    pub(crate) stop_after_steps: bool,
    /// Host calls the script has been asked about, which tracks `Recording::calls`.
    calls_seen: u64,
    /// The call the *test* cut the run at, if it did — `None` means the sequence parked on
    /// its own.
    pub(crate) stopped_at_call: Option<u64>,
}

impl Context {
    /// List a step, returning its index so the caller can still qualify it.
    pub(crate) fn push_step(&mut self, step: Step) -> usize {
        self.steps.push(step);
        self.steps.len() - 1
    }

    pub(crate) fn push_forbidden(&mut self, step: Step) {
        self.forbidden.push(step);
    }

    /// The last `expect_*` wins.
    pub(crate) fn require_outcome(&mut self, expected: Expected) {
        self.outcome = Some(expected);
    }

    pub(crate) fn give_telemetry(&mut self, id: i64, values: Vec<Vec<u8>>, supplied: Supplied) {
        self.telemetry.insert(id, Queue::of(values));
        self.supplied.push((id, supplied));
    }

    pub(crate) fn give_parameter(&mut self, id: i64, values: Vec<Vec<u8>>, supplied: Supplied) {
        self.parameters.insert(id, Queue::of(values));
        self.supplied.push((id, supplied));
    }

    /// Queue a message on a serial input port. Repeatable: they are delivered in order.
    pub(crate) fn queue_serial(&mut self, port: i32, message: Vec<u8>) {
        self.serial.entry(port).or_default().push(message);
    }

    pub(crate) fn give_args(&mut self, args: Vec<u8>) {
        self.args = Some(args);
    }

    /// How many messages were queued on `port`, and how many the sequence took.
    pub(crate) fn serial_traffic(&self, port: i32) -> (usize, usize) {
        (
            self.serial.get(&port).map(Vec::len).unwrap_or(0),
            self.serial_taken.get(&port).copied().unwrap_or(0),
        )
    }

    /// What each point was supplied as, with the read counts filled in from the run.
    pub(crate) fn supplied(&self) -> Vec<Supplied> {
        self.supplied
            .iter()
            .map(|(id, entry)| {
                let queue = match entry.is_parameter {
                    true => self.parameters.get(id),
                    false => self.telemetry.get(id),
                };
                Supplied {
                    reads: queue.map(Queue::reads).unwrap_or(0),
                    ..entry.clone()
                }
            })
            .collect()
    }

    /// Try the step at the cursor against `call`; on a match, apply its effects and advance.
    fn advance(&mut self, call: &Call) -> Option<usize> {
        let index = self.cursor;
        if !self.steps.get(index)?.matcher.matches(call) {
            return None;
        }
        self.cursor += 1;

        // Taken by value so the borrow of `self.steps` ends before the effects mutate the
        // queues.
        let effects = self.steps[index].effects.clone();
        for effect in effects {
            match effect {
                Effect::SetTelemetry { id, wire } => {
                    self.telemetry.insert(id, Queue::of(vec![wire]));
                }
                Effect::SetParameter { id, wire } => {
                    self.parameters.insert(id, Queue::of(vec![wire]));
                }
                Effect::QueueSerial { port, message } => {
                    self.serial.entry(port).or_default().push(message);
                }
            }
        }
        Some(index)
    }
}

impl Script for Context {
    fn command(&mut self, opcode: u32, payload: &[u8]) -> Option<i32> {
        let call = Call::Command {
            opcode,
            payload: payload.to_vec(),
            response: 0,
        };
        let matched = self.advance(&call)?;
        self.steps[matched].response.map(|response| response.code)
    }

    fn telemetry(&mut self, id: i64) -> Option<Vec<u8>> {
        // Advanced before reading, so a `sets` here takes effect on the *next* read.
        self.advance(&Call::Telemetry {
            id,
            value_len: 0,
            status: 0,
        });
        self.telemetry.get_mut(&id)?.next()
    }

    fn parameter(&mut self, id: i64) -> Option<Vec<u8>> {
        self.advance(&Call::Parameter {
            id,
            value_len: 0,
            status: 1,
        });
        self.parameters.get_mut(&id)?.next()
    }

    fn event(&mut self, severity: i32, message: &str) {
        self.advance(&Call::Event {
            severity,
            message: message.to_string(),
            truncated: false,
        });
    }

    fn sleep(&mut self, us: u64, absolute: bool) {
        // Relative advances by duration; absolute moves forward only, never back.
        self.clock_us = match absolute {
            true => self.clock_us.max(us),
            false => self.clock_us.saturating_add(us),
        };
        let call = match absolute {
            true => Call::AbsoluteSleep { us },
            false => Call::RelativeSleep { us },
        };
        self.advance(&call);
    }

    fn now_us(&mut self) -> Option<u64> {
        // Where a `time_read` step advances; `time` reaches the script only here.
        self.advance(&Call::Time {
            len: crate::abi::TIME_SERIALIZED_SIZE,
        });
        Some(self.clock_us)
    }

    fn serial_recv(&mut self, port: i32, blocking: bool) -> Option<Vec<u8>> {
        let taken = self.serial_taken.entry(port).or_insert(0);
        let message = self
            .serial
            .get(&port)
            .and_then(|queued| queued.get(*taken))
            .cloned();
        if message.is_some() {
            *taken += 1;
        }
        // `blocking` must come from the guest; the two passes need to observe the same call.
        self.advance(&Call::SerialRecv {
            index: port,
            blocking,
            received: message.as_ref().map(|m| m.len() as u32).unwrap_or(0),
            status: 0,
        });
        message
    }

    fn serial_send(&mut self, port: i32, data: &[u8]) {
        self.advance(&Call::SerialSend {
            index: port,
            len: data.len() as u32,
            payload: data.to_vec(),
        });
    }

    fn args(&mut self) -> Option<Vec<u8>> {
        let args = self.args.clone();
        self.advance(&Call::Args {
            capacity: 0,
            written: args.as_ref().map(|a| a.len() as u32).unwrap_or(0),
        });
        args
    }

    fn should_stop(&mut self) -> bool {
        self.calls_seen += 1;
        if !self.stop_after_steps || self.cursor < self.steps.len() {
            return false;
        }
        // Recorded once: the index the transcript prints for the call being cut.
        if self.stopped_at_call.is_none() {
            self.stopped_at_call = Some(self.calls_seen - 1);
        }
        true
    }
}

/// Where every step and every `never` rule landed in the recording.
#[derive(Debug, Default)]
pub(crate) struct Walk {
    /// For each step, the index into the recording where it matched.
    pub matched: Vec<Option<usize>>,
    /// For each `never` rule, the first index where it fired.
    pub fired: Vec<Option<usize>>,
}

impl Walk {
    /// The first step that did not match, if any.
    pub fn first_unmatched(&self) -> Option<usize> {
        self.matched.iter().position(Option::is_none)
    }

    pub fn any_fired(&self) -> bool {
        self.fired.iter().any(Option::is_some)
    }
}

/// Re-derives the match over the recording; `never` rules are checked over the whole thing.
pub(crate) fn walk(steps: &[Step], forbidden: &[Step], calls: &[Call]) -> Walk {
    let mut matched = vec![None; steps.len()];
    let mut cursor = 0usize;

    for (index, call) in calls.iter().enumerate() {
        if cursor >= steps.len() {
            break;
        }
        if steps[cursor].matcher.matches(call) {
            matched[cursor] = Some(index);
            cursor += 1;
        }
    }

    let fired = forbidden
        .iter()
        .map(|rule| calls.iter().position(|call| rule.matcher.matches(call)))
        .collect();

    Walk { matched, fired }
}

/// Whether a step's pattern occurs anywhere in the recording.
pub(crate) fn found_anywhere(matcher: &Matcher, calls: &[Call]) -> Option<usize> {
    calls.iter().position(|call| matcher.matches(call))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::{IntoCommand, Where, cmd, event, sleep};

    fn step(matcher: Matcher) -> Step {
        Step::new(
            matcher,
            Where {
                file: "t.rs",
                line: 1,
            },
        )
    }

    fn command(opcode: u32) -> Call {
        Call::Command {
            opcode,
            payload: vec![],
            response: 0,
        }
    }

    #[test]
    fn walk_is_a_subsequence() {
        let steps = vec![
            step(cmd(0x10).into_matcher()),
            step(cmd(0x30).into_matcher()),
        ];
        let calls = vec![
            command(0x10),
            command(0x20), // not listed, and not a failure
            Call::RelativeSleep { us: 5 },
            command(0x30),
        ];
        let result = walk(&steps, &[], &calls);
        assert_eq!(result.matched, vec![Some(0), Some(3)]);
        assert_eq!(result.first_unmatched(), None);
    }

    #[test]
    fn repeated_steps_consume_separate_calls() {
        let steps = vec![
            step(cmd(0x10).into_matcher()),
            step(cmd(0x10).into_matcher()),
        ];
        let one = walk(&steps, &[], &[command(0x10)]);
        assert_eq!(one.matched, vec![Some(0), None], "one call is one step");
        assert_eq!(one.first_unmatched(), Some(1));

        let two = walk(&steps, &[], &[command(0x10), command(0x10)]);
        assert_eq!(two.matched, vec![Some(0), Some(1)]);
    }

    #[test]
    fn step_cannot_match_before_predecessor() {
        let steps = vec![
            step(cmd(0x10).into_matcher()),
            step(cmd(0x20).into_matcher()),
        ];
        // The sequence sent them the other way round.
        let calls = vec![command(0x20), command(0x10)];
        let result = walk(&steps, &[], &calls);
        assert_eq!(result.matched, vec![Some(1), None]);
        assert_eq!(result.first_unmatched(), Some(1));
        // And the diagnosis is "too early", not "never happened", because it *is* there.
        assert_eq!(
            found_anywhere(&steps[1].matcher, &calls),
            Some(0),
            "the report must be able to tell these two cases apart"
        );
    }

    #[test]
    fn never_rule_scans_whole_recording() {
        let forbidden = vec![step(cmd(0x99).into_matcher())];
        let clean = walk(&[], &forbidden, &[command(0x10)]);
        assert_eq!(clean.fired, vec![None]);
        assert!(!clean.any_fired());

        // Fires even though it is the last thing the sequence did, which an at-the-cursor
        // check could have missed.
        let dirty = walk(&[], &forbidden, &[command(0x10), command(0x99)]);
        assert_eq!(dirty.fired, vec![Some(1)]);
        assert!(dirty.any_fired());
    }

    #[test]
    fn series_repeats_last_value() {
        let mut plan = Context::default();
        plan.give_telemetry(
            4,
            vec![vec![0], vec![1]],
            Supplied {
                path: "c",
                shown: "false, true".into(),
                reads: 0,
                is_parameter: false,
            },
        );

        assert_eq!(plan.telemetry(4), Some(vec![0]));
        assert_eq!(plan.telemetry(4), Some(vec![1]));
        assert_eq!(plan.telemetry(4), Some(vec![1]));
        assert_eq!(plan.supplied()[0].reads, 3);
        // A channel nobody supplied is left to the host, which zero-fills it.
        assert_eq!(plan.telemetry(99), None);
    }

    #[test]
    fn matched_step_supplies_response() {
        use fprime_core::desc::Response;
        let mut first = step(cmd(0x10).into_matcher());
        first.response = Some(Response::new(4, "EXECUTION_ERROR"));
        let second = step(cmd(0x10).into_matcher());

        let mut plan = Context {
            steps: vec![first, second],
            ..Context::default()
        };
        assert_eq!(plan.command(0x10, &[]), Some(4));
        // The second step has no `responds`, so the host is left with the nominal OK.
        assert_eq!(plan.command(0x10, &[]), None);
        // And a third call matches nothing, so it is also left alone.
        assert_eq!(plan.command(0x10, &[]), None);
    }

    #[test]
    fn effect_changes_channel_next_read() {
        let mut ready = step(cmd(0x10).into_matcher());
        ready.effects.push(Effect::SetTelemetry {
            id: 4,
            wire: vec![1],
        });

        let mut plan = Context {
            steps: vec![ready],
            ..Context::default()
        };
        plan.give_telemetry(
            4,
            vec![vec![0]],
            Supplied {
                path: "Ready",
                shown: "false".into(),
                reads: 0,
                is_parameter: false,
            },
        );

        assert_eq!(plan.telemetry(4), Some(vec![0]), "not ready yet");
        plan.command(0x10, &[]);
        assert_eq!(plan.telemetry(4), Some(vec![1]), "the step made it ready");
        assert_eq!(plan.telemetry(4), Some(vec![1]), "and it stays ready");
    }

    #[test]
    fn clock_advances_with_sleeps() {
        let mut plan = Context::default();
        assert_eq!(plan.now_us(), Some(0));

        plan.sleep(1_500_000, false);
        assert_eq!(plan.now_us(), Some(1_500_000));

        plan.sleep(5_000_000, true);
        assert_eq!(plan.now_us(), Some(5_000_000));

        // Asking to wake at a time already past must not rewind the clock.
        plan.sleep(1_000, true);
        assert_eq!(plan.now_us(), Some(5_000_000));
    }

    #[test]
    fn serial_messages_deliver_in_order_then_empty() {
        let mut plan = Context::default();
        plan.queue_serial(0, vec![1]);
        plan.queue_serial(0, vec![2]);

        assert_eq!(plan.serial_recv(0, true), Some(vec![1]));
        assert_eq!(plan.serial_recv(0, true), Some(vec![2]));
        assert_eq!(
            plan.serial_recv(0, true),
            None,
            "and then the port is empty"
        );
        assert_eq!(plan.serial_recv(0, true), None);
        // The pair a deadlock diagnosis reports: queued against taken.
        assert_eq!(plan.serial_traffic(0), (2, 2));
        // A port nobody queued anything on is empty from the start.
        assert_eq!(plan.serial_recv(3, true), None);
        assert_eq!(plan.serial_traffic(3), (0, 0));
    }

    #[test]
    fn step_effect_queues_another_message() {
        let mut reply = step(cmd(0x10).into_matcher());
        reply.effects.push(Effect::QueueSerial {
            port: 0,
            message: vec![9],
        });

        let mut plan = Context {
            steps: vec![],
            ..Context::default()
        };
        plan.push_step(reply);
        plan.queue_serial(0, vec![1]);

        assert_eq!(plan.serial_recv(0, true), Some(vec![1]));
        assert_eq!(plan.serial_recv(0, true), None, "nothing more yet");
        plan.command(0x10, &[]);
        assert_eq!(
            plan.serial_recv(0, true),
            Some(vec![9]),
            "the step brought one in"
        );
    }

    #[test]
    fn armed_stop_waits_for_last_step() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.push_step(step(cmd(0x20).into_matcher()));
        plan.stop_after_steps = true;

        // Two calls before the first step matches: not satisfied, so no stop.
        assert!(!plan.should_stop());
        assert!(!plan.should_stop());
        plan.command(0x10, &[]);
        assert!(!plan.should_stop(), "one step still outstanding");
        plan.command(0x20, &[]);
        assert!(plan.should_stop(), "every step has matched");
        assert!(plan.stop_after_steps);
        assert_eq!(
            plan.stopped_at_call,
            Some(3),
            "the index the transcript prints for the call being cut"
        );
        // Asked again, it does not move the recorded index.
        assert!(plan.should_stop());
        assert_eq!(plan.stopped_at_call, Some(3));
    }

    #[test]
    fn unarmed_plan_never_stops() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.command(0x10, &[]);
        for _ in 0..5 {
            assert!(!plan.should_stop());
        }
        assert_eq!(plan.stopped_at_call, None);
    }

    #[test]
    fn still_running_accepts_only_suspended() {
        assert!(Expected::StillRunning.accepts(&Outcome::Suspended));
        for other in [
            Outcome::OutOfInstructions,
            Outcome::Exited(0),
            Outcome::Returned,
            Outcome::Panicked(2),
            Outcome::Trapped("x".into()),
        ] {
            assert!(
                !Expected::StillRunning.accepts(&other),
                "{other:?} must not satisfy `expect_still_running`"
            );
        }
    }

    #[test]
    fn every_expected_outcome_accepts_only_itself() {
        let cases = [
            (Expected::Nominal, Outcome::Exited(0), true),
            (Expected::Nominal, Outcome::Returned, true),
            (Expected::Nominal, Outcome::Exited(1), false),
            (Expected::Exit(3), Outcome::Exited(3), true),
            (Expected::Exit(3), Outcome::Exited(4), false),
            (Expected::Panic(2), Outcome::Panicked(2), true),
            (Expected::Panic(2), Outcome::Panicked(7), false),
            (Expected::AnyPanic, Outcome::Panicked(7), true),
            (Expected::AnyPanic, Outcome::Exited(0), false),
        ];
        for (expected, outcome, accepted) in cases {
            assert_eq!(
                expected.accepts(&outcome),
                accepted,
                "{expected:?} against {outcome:?}"
            );
        }
    }

    #[test]
    fn absent_kind_step_stops_walk() {
        let steps = vec![
            step(cmd(0x10).into_matcher()),
            step(event().containing("safe").into_matcher()),
            step(sleep().into_matcher()),
        ];
        let calls = vec![command(0x10), Call::RelativeSleep { us: 1 }];
        let result = walk(&steps, &[], &calls);
        assert_eq!(result.first_unmatched(), Some(1));
        // The sleep is there, but the walk never got past the event.
        assert_eq!(result.matched, vec![Some(0), None, None]);
    }
}

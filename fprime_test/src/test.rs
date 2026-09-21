//! Public facing #[fprime_test] API

use crate::interpreter::{Limits, Outcome};
use crate::plan::{Context, Expected, Supplied};
use crate::step::{Effect, IntoCommand, Matcher, SerialRecvMatch, SerialSendMatch};
use crate::step::{SleepMatch, Step, Where};
use fprime_core::Serializable;
use fprime_core::desc::{Chan, IntoWire, Prm, Response};
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

/// One sequence test under construction.
#[derive(Clone)]
pub struct Test {
    pub(crate) plan: Rc<RefCell<Context>>,
}

impl Test {
    pub(crate) fn new() -> Self {
        Test {
            plan: Rc::new(RefCell::new(Context::default())),
        }
    }
}

/// Wire bytes for one supplied value.
///
/// The buffer is exactly `Serializable::SIZE`, which is what guarantees `serialize_to` cannot
/// take its `PanicCode::Truncated` arm and abort the whole test binary. A string writes less
/// than that, so only what it wrote is served.
fn wire<I: Serializable>(value: I) -> Vec<u8> {
    let mut buffer = vec![0u8; I::SIZE];
    let mut offset = 0;
    value.serialize_to(&mut buffer, &mut offset);
    buffer.truncate(offset);
    buffer
}

/// How a supplied value reads in a failure message.
fn shown<I: Debug>(value: &I) -> String {
    format!("{value:?}")
}

/// What the spacecraft reports, before the sequence starts.
impl Test {
    /// This channel reads as `value`, every time, for the whole run.
    ///
    /// ```ignore
    /// t.initial_telemetry(Ref.power.BatteryVoltage, 21.5);
    /// t.initial_telemetry(Ref.typeDemo.NameCh, "safe"); // a string channel
    /// ```
    pub fn initial_telemetry<I: Serializable + Debug>(
        &self,
        chan: Chan<I>,
        value: impl IntoWire<I>,
    ) -> &Self {
        let value = value.into_wire();
        let shown = shown(&value);
        let bytes = wire(value);
        self.plan.borrow_mut().give_telemetry(
            chan.id,
            vec![bytes],
            Supplied {
                path: chan.path,
                shown,
                reads: 0,
                is_parameter: false,
            },
        );
        self
    }

    /// Successive reads of this channel see successive values; the last one repeats.
    ///
    /// ```ignore
    /// // Reads as 1.0, then 2.0, then 3.0 forever after.
    /// t.initial_telemetry_each(Ref.power.BatteryVoltage, [1.0, 2.0, 3.0]);
    /// ```
    pub fn initial_telemetry_each<I: Serializable + Debug, V: IntoWire<I>, const N: usize>(
        &self,
        chan: Chan<I>,
        values: [V; N],
    ) -> &Self {
        let values = values.map(IntoWire::into_wire);
        let shown = values.iter().map(shown).collect::<Vec<_>>().join(", ");
        let bytes = values.into_iter().map(wire).collect();
        self.plan.borrow_mut().give_telemetry(
            chan.id,
            bytes,
            Supplied {
                path: chan.path,
                shown,
                reads: 0,
                is_parameter: false,
            },
        );
        self
    }

    /// This parameter reads as `value`.
    ///
    /// ```ignore
    /// t.initial_parameter(Ref.wasmSeq.INSTRUCTION_FUEL, 20);
    /// ```
    pub fn initial_parameter<I: Serializable + Debug>(
        &self,
        prm: Prm<I>,
        value: impl IntoWire<I>,
    ) -> &Self {
        let value = value.into_wire();
        let shown = shown(&value);
        let bytes = wire(value);
        self.plan.borrow_mut().give_parameter(
            prm.id,
            vec![bytes],
            Supplied {
                path: prm.path,
                shown,
                reads: 0,
                is_parameter: true,
            },
        );
        self
    }

    /// Successive reads of this parameter see successive values; the last one repeats.
    ///
    /// ```ignore
    /// t.initial_parameter_each(Ref.wasmSeq.INSTRUCTION_FUEL, [10, 20, 30]);
    /// ```
    pub fn initial_parameter_each<I: Serializable + Debug, V: IntoWire<I>, const N: usize>(
        &self,
        prm: Prm<I>,
        values: [V; N],
    ) -> &Self {
        let values = values.map(IntoWire::into_wire);
        let shown = values.iter().map(shown).collect::<Vec<_>>().join(", ");
        let bytes = values.into_iter().map(wire).collect();
        self.plan.borrow_mut().give_parameter(
            prm.id,
            bytes,
            Supplied {
                path: prm.path,
                shown,
                reads: 0,
                is_parameter: true,
            },
        );
        self
    }

    /// A value waiting on a serial input port, for a port a sequence reads through
    /// `Queue<T, N>`. Repeatable: delivered in order, one per receive.
    ///
    /// ```ignore
    /// t.initial_serial(0, 100u32);
    /// ```
    pub fn initial_serial<S: Serializable>(&self, port: i32, value: S) -> &Self {
        self.plan.borrow_mut().queue_serial(port, wire(value));
        self
    }

    /// The arguments the sequence was invoked with, as `fprime_v1.args` delivers them.
    ///
    /// ```ignore
    /// t.initial_args(b"a warm restart");
    /// ```
    pub fn initial_args(&self, args: &[u8]) -> &Self {
        self.plan.borrow_mut().give_args(args.to_vec());
        self
    }
}

/// What the sequence must do.
impl Test {
    #[track_caller]
    fn add_step(&self, matcher: Matcher) -> Pending<'_> {
        let index = self
            .plan
            .borrow_mut()
            .push_step(Step::new(matcher, Where::here()));
        Pending { test: self, index }
    }

    #[track_caller]
    fn add_forbidden(&self, matcher: Matcher) -> &Self {
        self.plan
            .borrow_mut()
            .push_forbidden(Step::new(matcher, Where::here()));
        self
    }

    /// The next command the sequence must send, after everything already listed. Write it any
    /// of these ways:
    ///
    /// ```ignore
    /// t.expect_command(Ref.power.PWR_OFF());  // this exact call
    /// t.expect_command(Ref.power.PWR_OFF);    // this opcode, with any arguments
    /// t.expect_command(cmd(0x1234));          // an opcode with no dictionary entry
    /// ```
    ///
    /// Chain onto what it returns to say how the spacecraft answers, or what changes when the
    /// command arrives:
    ///
    /// ```ignore
    /// t.expect_command(Ref.power.PWR_OFF())
    ///     .responds(EXECUTION_ERROR)
    ///     .sets_telemetry(Ref.power.State, OFF)
    ///     .because("a refusal still turns the flag off");
    /// ```
    #[track_caller]
    pub fn expect_command<S: IntoCommand>(&self, step: S) -> Pending<'_> {
        self.add_step(step.into_matcher())
    }

    /// A command the sequence must never send, anywhere in the run.
    ///
    /// ```ignore
    /// t.never_command(Ref.wasmSeq.LOAD);       // this opcode, any arguments
    /// t.never_command(Ref.wasmSeq.LOAD("x"));  // this exact call
    /// ```
    #[track_caller]
    pub fn never_command<S: IntoCommand>(&self, step: S) -> &Self {
        self.add_forbidden(step.into_matcher())
    }

    /// The next event the sequence must emit, whatever it says.
    ///
    /// ```ignore
    /// t.expect_event();
    /// ```
    #[track_caller]
    pub fn expect_event(&self) -> Pending<'_> {
        self.add_step(Matcher::Event {
            contains: None,
            exactly: None,
        })
    }

    /// The next event the sequence must emit, whose message contains this.
    ///
    /// ```ignore
    /// t.expect_event_containing("safe mode");
    /// ```
    #[track_caller]
    pub fn expect_event_containing(&self, needle: &str) -> Pending<'_> {
        self.add_step(Matcher::Event {
            contains: Some(needle.to_string()),
            exactly: None,
        })
    }

    /// The next event the sequence must emit, whose message is exactly this (after the
    /// sequencer's truncation).
    ///
    /// ```ignore
    /// t.expect_event_exactly("entering safe mode");
    /// ```
    #[track_caller]
    pub fn expect_event_exactly(&self, message: &str) -> Pending<'_> {
        self.add_step(Matcher::Event {
            contains: None,
            exactly: Some(message.to_string()),
        })
    }

    /// An event the sequence must never emit, whatever it says.
    ///
    /// ```ignore
    /// t.never_event();
    /// ```
    #[track_caller]
    pub fn never_event(&self) -> &Self {
        self.add_forbidden(Matcher::Event {
            contains: None,
            exactly: None,
        })
    }

    /// An event the sequence must never emit, whose message contains this.
    ///
    /// ```ignore
    /// t.never_event_containing("safe mode");
    /// ```
    #[track_caller]
    pub fn never_event_containing(&self, needle: &str) -> &Self {
        self.add_forbidden(Matcher::Event {
            contains: Some(needle.to_string()),
            exactly: None,
        })
    }

    /// An event the sequence must never emit, whose message is exactly this (after the
    /// sequencer's truncation).
    ///
    /// ```ignore
    /// t.never_event_exactly("entering safe mode");
    /// ```
    #[track_caller]
    pub fn never_event_exactly(&self, message: &str) -> &Self {
        self.add_forbidden(Matcher::Event {
            contains: None,
            exactly: Some(message.to_string()),
        })
    }

    /// The next sleep the sequence must take.
    ///
    /// ```ignore
    /// t.expect_sleep(sleep().of_secs(1.0).relative());
    /// ```
    #[track_caller]
    pub fn expect_sleep(&self, step: SleepMatch) -> Pending<'_> {
        self.add_step(step.into_matcher())
    }

    /// A sleep the sequence must never take.
    ///
    /// ```ignore
    /// t.never_sleep(sleep().absolute());
    /// ```
    #[track_caller]
    pub fn never_sleep(&self, step: SleepMatch) -> &Self {
        self.add_forbidden(step.into_matcher())
    }

    /// The next read of this telemetry channel, whatever it reads as.
    ///
    /// ```ignore
    /// t.expect_telemetry_read(CdhCore.events.EventsDropped);
    /// ```
    #[track_caller]
    pub fn expect_telemetry_read<I: Serializable>(&self, chan: Chan<I>) -> Pending<'_> {
        self.add_step(Matcher::TelemetryRead {
            id: chan.id,
            path: chan.path,
        })
    }

    /// A read of this telemetry channel that must never happen.
    ///
    /// ```ignore
    /// t.never_telemetry_read(CdhCore.events.EventsDropped);
    /// ```
    #[track_caller]
    pub fn never_telemetry_read<I: Serializable>(&self, chan: Chan<I>) -> &Self {
        self.add_forbidden(Matcher::TelemetryRead {
            id: chan.id,
            path: chan.path,
        })
    }

    /// The next read of this parameter, whatever it reads as.
    ///
    /// ```ignore
    /// t.expect_parameter_read(Ref.wasmSeq.INSTRUCTION_FUEL);
    /// ```
    #[track_caller]
    pub fn expect_parameter_read<I: Serializable>(&self, prm: Prm<I>) -> Pending<'_> {
        self.add_step(Matcher::ParameterRead {
            id: prm.id,
            path: prm.path,
        })
    }

    /// A read of this parameter that must never happen.
    ///
    /// ```ignore
    /// t.never_parameter_read(Ref.wasmSeq.INSTRUCTION_FUEL);
    /// ```
    #[track_caller]
    pub fn never_parameter_read<I: Serializable>(&self, prm: Prm<I>) -> &Self {
        self.add_forbidden(Matcher::ParameterRead {
            id: prm.id,
            path: prm.path,
        })
    }

    /// The next send on a serial output port.
    ///
    /// ```ignore
    /// t.expect_serial_send(serial_send(0).of(42u32));
    /// ```
    #[track_caller]
    pub fn expect_serial_send(&self, step: SerialSendMatch) -> Pending<'_> {
        self.add_step(step.into_matcher())
    }

    /// A send on a serial output port that must never happen.
    ///
    /// ```ignore
    /// t.never_serial_send(serial_send(0));
    /// ```
    #[track_caller]
    pub fn never_serial_send(&self, step: SerialSendMatch) -> &Self {
        self.add_forbidden(step.into_matcher())
    }

    /// The next receive on a serial input port.
    ///
    /// ```ignore
    /// t.expect_serial_recv(serial_recv(0).blocking().finding_a_message());
    /// ```
    #[track_caller]
    pub fn expect_serial_recv(&self, step: SerialRecvMatch) -> Pending<'_> {
        self.add_step(step.into_matcher())
    }

    /// A receive on a serial input port that must never happen.
    ///
    /// ```ignore
    /// t.never_serial_recv(serial_recv(0).finding_a_message());
    /// ```
    #[track_caller]
    pub fn never_serial_recv(&self, step: SerialRecvMatch) -> &Self {
        self.add_forbidden(step.into_matcher())
    }

    /// The next time the sequence reads its own invocation arguments.
    ///
    /// ```ignore
    /// t.expect_args_read();
    /// ```
    #[track_caller]
    pub fn expect_args_read(&self) -> Pending<'_> {
        self.add_step(Matcher::ArgsRead)
    }

    /// The sequence must never read its own invocation arguments.
    ///
    /// ```ignore
    /// t.never_args_read();
    /// ```
    #[track_caller]
    pub fn never_args_read(&self) -> &Self {
        self.add_forbidden(Matcher::ArgsRead)
    }

    /// The next time the sequence reads the clock.
    ///
    /// ```ignore
    /// t.expect_time_read();
    /// ```
    #[track_caller]
    pub fn expect_time_read(&self) -> Pending<'_> {
        self.add_step(Matcher::TimeRead)
    }

    /// The sequence must never read the clock.
    ///
    /// ```ignore
    /// t.never_time_read();
    /// ```
    #[track_caller]
    pub fn never_time_read(&self) -> &Self {
        self.add_forbidden(Matcher::TimeRead)
    }
}

/// How the sequence must end.
///
/// With none of these, the requirement is a nominal outcome — a clean exit or a plain return
/// from `main`.
impl Test {
    /// The sequence called `fprime_v1.exit` with this code.
    ///
    /// ```ignore
    /// t.expect_exit(0);
    /// ```
    pub fn expect_exit(&self, code: i32) -> &Self {
        self.plan.borrow_mut().require_outcome(Expected::Exit(code));
        self
    }

    /// The sequence panicked with this `PanicCode`.
    ///
    /// ```ignore
    /// t.expect_panic(2); // PanicCode::CmdFailed
    /// ```
    pub fn expect_panic(&self, code: i32) -> &Self {
        self.plan
            .borrow_mut()
            .require_outcome(Expected::Panic(code));
        self
    }

    /// The sequence panicked, whatever the code.
    pub fn expect_panic_any(&self) -> &Self {
        self.plan.borrow_mut().require_outcome(Expected::AnyPanic);
        self
    }

    /// A nominal end: exit 0, or a return from `main`. The default, stated.
    ///
    /// ```ignore
    /// t.expect_ok();
    /// ```
    pub fn expect_ok(&self) -> &Self {
        self.plan.borrow_mut().require_outcome(Expected::Nominal);
        self
    }

    /// The sequence had not finished when the run ended — for one that never exits. Add
    /// [`Test::stop_after_last_step`] for a sequence that polls rather than blocking.
    ///
    /// ```ignore
    /// t.expect_still_running();
    /// ```
    pub fn expect_still_running(&self) -> &Self {
        self.plan
            .borrow_mut()
            .require_outcome(Expected::StillRunning);
        self
    }

    /// Any other outcome, verbatim — a trap, or running out of instructions.
    ///
    /// ```ignore
    /// t.expect_outcome(Outcome::OutOfInstructions);
    /// ```
    pub fn expect_outcome(&self, outcome: Outcome) -> &Self {
        self.plan
            .borrow_mut()
            .require_outcome(Expected::Exactly(outcome));
        self
    }
}

/// Configuration.
impl Test {
    /// Run against these limits instead of the ones the file supplied.
    ///
    /// ```ignore
    /// t.limits(Limits { max_instructions: 10_000, ..Limits::default() });
    /// ```
    ///
    /// This replaces every limit, so the `..Limits::default()` above is the *stock* sequencer,
    /// not the deployment's — a whole configuration written in Rust. To run against another of
    /// the deployment's own configurations, name its file instead and leave this alone:
    ///
    /// ```ignore
    /// #[fprime_test(sequence = "safing", limits = "sequencer-payload.toml")]
    /// ```
    ///
    /// If a test does both, this wins.
    pub fn limits(&self, limits: Limits) -> &Self {
        self.plan.borrow_mut().limits = Some(limits);
        self
    }

    /// Stop the sequence as soon as every listed step has matched.
    /// Only for a sequence that never finishes on its own.
    ///
    /// This bounds every `never_*` rule too: one only covers the sequence up to the call that
    /// stopped it.
    ///
    /// ```ignore
    /// t.stop_after_last_step();
    /// t.expect_still_running();
    /// ```
    pub fn stop_after_last_step(&self) -> &Self {
        self.plan.borrow_mut().stop_after_steps = true;
        self
    }

    /// The one limit a polling test routinely needs to raise.
    ///
    /// ```ignore
    /// t.max_instructions(1_000_000);
    /// ```
    pub fn max_instructions(&self, instructions: u64) -> &Self {
        self.plan.borrow_mut().max_instructions = Some(instructions);
        self
    }
}

/// A step that has been listed and can still be qualified — what every `expect_*` method
/// returns.
///
/// ```ignore
/// t.expect_command(Ref.power.PWR_OFF())
///     .responds(EXECUTION_ERROR)      // how the spacecraft answers
///     .sets_telemetry(Ref.power.State, OFF) // what changes when this step fires
///     .because("a refusal still turns the flag off"); // why this step is here
/// ```
pub struct Pending<'a> {
    test: &'a Test,
    index: usize,
}

impl<'a> Pending<'a> {
    /// The response the spacecraft gives this command. Unlisted commands, and listed commands
    /// without a `responds`, answer `Fw::CmdResponse::OK`.
    ///
    /// ```ignore
    /// t.expect_command(Ref.power.PWR_OFF()).responds(EXECUTION_ERROR);
    /// ```
    pub fn responds(self, response: Response) -> Self {
        self.with(|step| step.response = Some(response))
    }

    /// When this step fires, this channel starts reading as `value` — replacing whatever it
    /// had been going to report, an `initial_telemetry_each` series included.
    ///
    /// ```ignore
    /// t.expect_command(Ref.power.PWR_OFF()).sets_telemetry(Ref.power.State, OFF);
    /// ```
    pub fn sets_telemetry<I: Serializable>(self, chan: Chan<I>, value: impl IntoWire<I>) -> Self {
        let wire = wire(value.into_wire());
        self.with(|step| {
            step.effects
                .push(Effect::SetTelemetry { id: chan.id, wire })
        })
    }

    /// The same, for a parameter.
    ///
    /// ```ignore
    /// t.expect_command(Ref.wasmSeq.SET_FUEL()).sets_parameter(Ref.wasmSeq.INSTRUCTION_FUEL, 5);
    /// ```
    pub fn sets_parameter<I: Serializable>(self, prm: Prm<I>, value: impl IntoWire<I>) -> Self {
        let wire = wire(value.into_wire());
        self.with(|step| step.effects.push(Effect::SetParameter { id: prm.id, wire }))
    }

    /// When this step fires, this value arrives on a serial input port.
    ///
    /// ```ignore
    /// t.expect_serial_send(serial_send(0)).queues_serial(0, 20u32);
    /// ```
    pub fn queues_serial<S: Serializable>(self, port: i32, value: S) -> Self {
        let message = wire(value);
        self.with(|step| step.effects.push(Effect::QueueSerial { port, message }))
    }

    /// Why this step is here, carried into the failure message if it is the one that fails.
    ///
    /// ```ignore
    /// t.expect_command(Ref.power.PWR_OFF())
    ///     .because("the retry after the first PWR_OFF fails");
    /// ```
    pub fn because(self, why: &'static str) -> Self {
        self.with(|step| step.note = Some(why))
    }

    fn with(self, change: impl FnOnce(&mut Step)) -> Self {
        change(&mut self.test.plan.borrow_mut().steps[self.index]);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fprime_core::desc::Chan;

    /// A stand-in for a generated descriptor: a `u32` channel.
    const COUNT: Chan<u32> = Chan::new(0x1000000, "CdhCore.cmdDisp.CommandsDispatched", "U32");

    #[test]
    fn initial_value_is_encoded_and_shown() {
        let test = Test::new();
        test.initial_telemetry(COUNT, 7);

        let plan = test.plan.borrow();
        let supplied = plan.supplied();
        assert_eq!(supplied.len(), 1);
        assert_eq!(supplied[0].path, "CdhCore.cmdDisp.CommandsDispatched");
        // Shown as `7`, not as `00 00 00 07`: this block exists to be read.
        assert_eq!(supplied[0].shown, "7");
    }

    #[test]
    fn series_is_shown_in_order() {
        let test = Test::new();
        test.initial_telemetry_each(COUNT, [1, 2, 3]);
        assert_eq!(test.plan.borrow().supplied()[0].shown, "1, 2, 3");
    }

    #[test]
    fn steps_and_initial_values_chain_without_binding() {
        let test = Test::new();
        test.initial_telemetry(COUNT, 1)
            .initial_telemetry(COUNT, 2)
            .expect_exit(0);
        test.expect_command(crate::step::cmd(0x10))
            .responds(Response::new(4, "EXECUTION_ERROR"))
            .sets_telemetry(COUNT, 9)
            .because("the first attempt is refused");

        let plan = test.plan.borrow();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].response.expect("a response").code, 4);
        assert_eq!(plan.steps[0].effects.len(), 1);
        assert_eq!(plan.steps[0].note, Some("the first attempt is refused"));
        assert_eq!(plan.outcome, Some(Expected::Exit(0)));
    }

    /// `#[track_caller]` has to survive the macro's rewrite of the body, or every step in a
    /// failure message would report the attribute's line instead of its own.
    #[test]
    fn step_records_where_it_was_written() {
        let test = Test::new();
        let line = line!() + 1;
        test.expect_command(crate::step::cmd(0x10));
        let plan = test.plan.borrow();
        assert_eq!(plan.steps[0].at.line, line);
        assert!(plan.steps[0].at.file.ends_with("test.rs"));
    }

    #[test]
    fn never_rule_is_not_an_ordered_step() {
        let test = Test::new();
        test.never_command(crate::step::cmd(0x99));
        let plan = test.plan.borrow();
        assert!(plan.steps.is_empty());
        assert_eq!(plan.forbidden.len(), 1);
    }

    #[test]
    fn last_stated_outcome_is_required() {
        let test = Test::new();
        test.expect_ok().expect_panic(2);
        assert_eq!(test.plan.borrow().outcome, Some(Expected::Panic(2)));
    }
}

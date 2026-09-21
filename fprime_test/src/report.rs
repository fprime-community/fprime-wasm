//! Renders a failing sequence test's panic message.
//!
//! Host-call lines use [`crate::describe::describe_call`], so a call reads the same here as
//! in any other transcript.

use crate::describe::{describe_call, describe_outcome};
use crate::interpreter::{Call, Issues, Kind, Outcome};
use crate::plan::{Context, Expected, Supplied, Walk, found_anywhere};
use crate::step::Matcher;
use fprime_dictionary::Dictionary;

/// The call that would satisfy this step if it happened too early, excluding one an earlier
/// step already matched.
fn misplaced_at(plan: &Context, walk: &Walk, calls: &[Call], index: usize) -> Option<usize> {
    if walk.matched[index].is_some() {
        return None;
    }
    let at = found_anywhere(&plan.steps[index].matcher, calls)?;
    let reached = walk.matched[..index].iter().flatten().max().copied()?;
    // Strictly before, and not the call that got the previous step there.
    (at < reached && !walk.matched.iter().any(|m| *m == Some(at))).then_some(at)
}

/// What the failure was about.
pub(crate) enum Failed {
    /// A step matched nothing after the step before it.
    Step(usize),
    /// A `never` rule matched.
    Never,
    /// Every step matched, but the sequence did not end the way the test required.
    OutcomeOnly,
}

/// The whole message, ready to hand to `panic!`.
pub(crate) fn render(
    sequence: &str,
    module: &std::path::Path,
    limits: &str,
    plan: &Context,
    walk: &Walk,
    calls: &[Call],
    outcome: &Outcome,
    issues: &[Issues],
    dictionary: Option<&Dictionary>,
    failed: Failed,
) -> String {
    let mut blocks = vec![
        head(sequence, module, limits, plan, walk, calls, &failed),
        match &failed {
            Failed::Step(index) => step_detail(plan, walk, calls, *index),
            Failed::Never => never_detail(plan, walk),
            Failed::OutcomeOnly => outcome_detail(plan, outcome),
        },
        transcript(plan, walk, calls, outcome, dictionary, &failed),
    ];
    blocks.extend(supplied(&plan.supplied()));
    blocks.extend(trap_reason(issues, calls, outcome));
    blocks.extend(deadlock(plan, calls, outcome, &failed));
    blocks.extend(hint(plan, walk, calls, dictionary, &failed));
    joined(&blocks)
}

/// Joins blocks with one blank line between them.
fn joined(blocks: &[String]) -> String {
    blocks.join("\n")
}

/// Joins lines with `\n`, ending in one; an empty string is a blank line.
fn lines(lines: Vec<String>) -> String {
    lines.into_iter().map(|line| format!("{line}\n")).collect()
}

/// The one-line verdict, followed by where the module and its limits came from.
fn head(
    sequence: &str,
    module: &std::path::Path,
    limits: &str,
    plan: &Context,
    walk: &Walk,
    calls: &[Call],
    failed: &Failed,
) -> String {
    let headline = match failed {
        Failed::Step(index) => match misplaced_at(plan, walk, calls, *index).is_some() {
            true => format!("sequence `{sequence}`: wrong order"),
            false => format!("sequence `{sequence}`: step missing"),
        },
        Failed::Never => format!("sequence `{sequence}`: forbidden call happened"),
        Failed::OutcomeOnly => format!("sequence `{sequence}`: wrong outcome"),
    };
    lines(vec![
        headline,
        format!("  module  {}", module.display()),
        format!("  limits  {limits}"),
    ])
}

/// The last call's `Warning` issues, when the run trapped — the host's own reason.
fn trap_reason(issues: &[Issues], calls: &[Call], outcome: &Outcome) -> Option<String> {
    if !matches!(outcome, Outcome::Trapped(_)) {
        return None;
    }
    let at = calls.len().checked_sub(1)?;
    let reasons: Vec<&str> = issues
        .iter()
        .filter(|issue| issue.call == at && issue.kind == Kind::Warning)
        .map(|issue| issue.message.as_str())
        .collect();
    if reasons.is_empty() {
        return None;
    }

    let mut out = vec![format!("trap reason, host call {at}")];
    out.extend(reasons.into_iter().map(|reason| format!("  {reason}")));
    Some(lines(out))
}

/// Explains a suspended run: the sequence waiting on a port this test never fed.
fn deadlock(plan: &Context, calls: &[Call], outcome: &Outcome, failed: &Failed) -> Option<String> {
    if !matches!(failed, Failed::Step(_)) || !matches!(outcome, Outcome::Suspended) {
        return None;
    }
    if plan.stopped_at_call.is_some() {
        return Some(lines(vec![
            "  stopped by stop_after_last_step(); nothing after the last matched call was \
             observed."
                .to_string(),
        ]));
    }
    let port = waiting_on(calls)?;

    let (queued, taken) = plan.serial_traffic(port);
    Some(lines(vec![
        format!(
            "  parked on serial port {port}: queued {queued}, read {taken} — nothing left; on \
             board this waits forever."
        ),
        String::new(),
        format!("  fix: t.initial_serial({port}, ..) — another message up front, or"),
        format!(
            "       t.expect_command(..).queues_serial({port}, ..) — one that arrives on a step."
        ),
        "       otherwise the sequence itself is wrong.".to_string(),
    ]))
}

/// The step that stopped the walk, and where it was written.
fn step_detail(plan: &Context, walk: &Walk, calls: &[Call], index: usize) -> String {
    let step = &plan.steps[index];
    let total = plan.steps.len();
    let reached = walk.matched[..index].iter().flatten().max().copied();
    let misplaced = misplaced_at(plan, walk, calls, index);

    let mut out = vec![
        match misplaced.is_some() {
            true => format!("step {} of {total} happened too early", index + 1),
            false => format!("step {} of {total} never happened", index + 1),
        },
        String::new(),
        format!("  {}", step.at),
        format!("    expect  {}", step.matcher.describe()),
    ];
    if let Some(note) = step.note {
        out.push(format!("    because {note}"));
    }

    match misplaced {
        Some(at) => {
            out.push(format!(
                "    no match after host call {}; happened earlier, at host call {at}",
                reached.unwrap_or(0)
            ));
            out.push(String::new());
            out.push(
                "  steps match in order — an earlier call can't satisfy a later step."
                    .to_string(),
            );
        }
        None => {
            out.push(String::new());
            out.push(match (index, reached) {
                (0, _) => "  nothing matched step 1.".to_string(),
                (_, Some(previous)) => format!(
                    "  steps 1-{index} matched (last at host call {previous}); nothing after \
                     matched step {}.",
                    index + 1
                ),
                (_, None) => "  nothing matched this step.".to_string(),
            });
        }
    }
    lines(out)
}

fn never_detail(plan: &Context, walk: &Walk) -> String {
    let fired = walk.fired.iter().filter(|at| at.is_some()).count();
    let mut out = vec![
        format!("{fired} of {} `never` rules fired", plan.forbidden.len()),
        String::new(),
    ];
    for (rule, at) in plan.forbidden.iter().zip(&walk.fired) {
        let Some(at) = at else { continue };
        out.push(format!("  {}", rule.at));
        out.push(format!("    never   {}", rule.matcher.describe()));
        if let Some(note) = rule.note {
            out.push(format!("    because {note}"));
        }
        out.push(format!("    fired   at host call {at}"));
    }
    lines(out)
}

fn outcome_detail(plan: &Context, outcome: &Outcome) -> String {
    let expected = plan.outcome.clone().unwrap_or(Expected::Nominal);
    let mut out = vec![
        "all steps matched; wrong ending".to_string(),
        String::new(),
        format!("    expected  {}", expected.describe()),
        format!("    actually  {}", describe_outcome(outcome)),
    ];

    if expected == Expected::StillRunning && matches!(outcome, Outcome::OutOfInstructions) {
        out.push(String::new());
        out.push(
            "  polls instead of blocking, so it ran out the instruction limit instead of \
             stopping on empty."
                .to_string(),
        );
        out.push("  fix: add `t.stop_after_last_step();`".to_string());
    }
    lines(out)
}

/// Every host call, with the step it satisfied — `??` marks a misplaced call, `!!` a `never`
/// violation.
fn transcript(
    plan: &Context,
    walk: &Walk,
    calls: &[Call],
    outcome: &Outcome,
    dictionary: Option<&Dictionary>,
    failed: &Failed,
) -> String {
    // Which call, if any, gets a marker rather than a step number.
    let misplaced = match failed {
        Failed::Step(index) => misplaced_at(plan, walk, calls, *index),
        _ => None,
    };
    let violations: Vec<usize> = walk.fired.iter().flatten().copied().collect();

    let mut out = vec![
        "what the sequence did".to_string(),
        String::new(),
        "   #  step  host call".to_string(),
        format!("  --  ----  {}", "-".repeat(60)),
    ];
    for (index, call) in calls.iter().enumerate() {
        let marker = if violations.contains(&index) {
            "!!".to_string()
        } else if misplaced == Some(index) {
            "??".to_string()
        } else {
            match walk.matched.iter().position(|at| *at == Some(index)) {
                Some(step) => (step + 1).to_string(),
                None => String::new(),
            }
        };
        out.push(format!(
            "  {index:>2}  {marker:>4}  {}",
            describe_call(call, dictionary)
        ));
    }
    out.push(String::new());
    out.push(format!("  {}", boundary(plan, calls, outcome)));

    if misplaced.is_some() {
        out.push(String::new());
        out.push("  ?? = would match, but came too late.".to_string());
    }
    if !violations.is_empty() {
        out.push(String::new());
        out.push("  !! = matched a `never` rule.".to_string());
    }
    lines(out)
}

/// How the run ended — a `Suspended` run is distinguished as the sequence parking vs. this
/// test cutting it short.
fn boundary(plan: &Context, calls: &[Call], outcome: &Outcome) -> String {
    if !matches!(outcome, Outcome::Suspended) {
        return describe_outcome(outcome);
    }
    if let Some(at) = plan.stopped_at_call {
        return format!("stopped by this test at host call {at}; nothing after was observed");
    }
    match waiting_on(calls) {
        Some(port) => format!("waiting on serial port {port}; on board this would wait forever"),
        None => describe_outcome(outcome),
    }
}

/// The port a parked sequence is waiting on: the last receive that found nothing.
fn waiting_on(calls: &[Call]) -> Option<i32> {
    calls.iter().rev().find_map(|call| match call {
        Call::SerialRecv {
            index, received: 0, ..
        } => Some(*index),
        _ => None,
    })
}

/// What the test fed the sequence.
fn supplied(points: &[Supplied]) -> Option<String> {
    if points.is_empty() {
        return None;
    }
    let width = points.iter().map(|p| p.path.len()).max().unwrap_or(0);
    let mut out = vec!["what this test supplied".to_string()];
    for point in points {
        let reads = match point.reads {
            1 => "read once".to_string(),
            n => format!("read {n} times"),
        };
        out.push(format!(
            "  {:<width$}  {:<18}  {reads}",
            point.path,
            point.shown,
            width = width
        ));
    }
    Some(lines(out))
}

/// The nearest call of the same kind as the failing step, if any.
fn hint(
    plan: &Context,
    walk: &Walk,
    calls: &[Call],
    dictionary: Option<&Dictionary>,
    failed: &Failed,
) -> Option<String> {
    let Failed::Step(index) = failed else {
        return None;
    };
    let step = &plan.steps[*index];
    let from = walk.matched[..*index]
        .iter()
        .flatten()
        .max()
        .map(|at| at + 1)
        .unwrap_or(0);

    let (at, call) = calls
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, call)| same_kind(&step.matcher, call))?;

    Some(lines(vec![
        format!("  fix: nearest match of that kind is host call {at}:"),
        format!("       {}", describe_call(call, dictionary)),
        "       either the sequence or this step is wrong.".to_string(),
    ]))
}

/// Whether a call is the same *kind* as a matcher, ignoring the details.
fn same_kind(matcher: &Matcher, call: &Call) -> bool {
    matches!(
        (matcher, call),
        (Matcher::Command { .. }, Call::Command { .. })
            | (Matcher::Event { .. }, Call::Event { .. })
            | (
                Matcher::Sleep { .. },
                Call::RelativeSleep { .. } | Call::AbsoluteSleep { .. }
            )
            | (Matcher::TelemetryRead { .. }, Call::Telemetry { .. })
            | (Matcher::ParameterRead { .. }, Call::Parameter { .. })
    )
}

/// The message for a run that could not even be attempted; every line indented.
pub(crate) fn could_not_run(sequence: &str, error: &anyhow::Error) -> String {
    let detail = format!("{error:#}")
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("sequence `{sequence}` could not be run\n\n{detail}\n")
}

/// Whether a step or `never` rule survived, given a walk and an outcome.
pub(crate) fn verdict(plan: &Context, walk: &Walk, outcome: &Outcome) -> Option<Failed> {
    if walk.any_fired() {
        return Some(Failed::Never);
    }
    if let Some(index) = walk.first_unmatched() {
        return Some(Failed::Step(index));
    }
    let expected = plan.outcome.clone().unwrap_or(Expected::Nominal);
    match expected.accepts(outcome) {
        true => None,
        false => Some(Failed::OutcomeOnly),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::Step;
    use crate::step::{IntoCommand, Where, cmd, event};

    fn step(matcher: Matcher) -> Step {
        Step::new(
            matcher,
            Where {
                file: "tests/safing.rs",
                line: 12,
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
    fn fired_never_rule_outranks_unmatched_step() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.push_forbidden(step(cmd(0x99).into_matcher()));
        let calls = vec![command(0x99)];
        let walk = crate::plan::walk(&plan.steps, &plan.forbidden, &calls);
        assert!(matches!(
            verdict(&plan, &walk, &Outcome::Exited(0)),
            Some(Failed::Never)
        ));
    }

    #[test]
    fn outcome_failure_reported_only_after_steps_match() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.require_outcome(Expected::Exit(0));

        let missing = crate::plan::walk(&plan.steps, &[], &[]);
        assert!(matches!(
            verdict(&plan, &missing, &Outcome::Panicked(2)),
            Some(Failed::Step(0))
        ));

        let matched = crate::plan::walk(&plan.steps, &[], &[command(0x10)]);
        assert!(matches!(
            verdict(&plan, &matched, &Outcome::Panicked(2)),
            Some(Failed::OutcomeOnly)
        ));
        assert!(verdict(&plan, &matched, &Outcome::Exited(0)).is_none());
    }

    #[test]
    fn default_outcome_requirement_is_nominal() {
        let plan = Context::default();
        let walk = crate::plan::walk(&[], &[], &[]);
        assert!(verdict(&plan, &walk, &Outcome::Returned).is_none());
        assert!(verdict(&plan, &walk, &Outcome::Exited(0)).is_none());
        assert!(matches!(
            verdict(&plan, &walk, &Outcome::Trapped("MemoryOutOfBounds".into())),
            Some(Failed::OutcomeOnly)
        ));
    }

    #[test]
    fn misplaced_step_differs_from_absent_step() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.push_step(step(cmd(0x20).into_matcher()));

        // Present, but before step 1 matched.
        let calls = vec![command(0x20), command(0x10)];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "safing",
            std::path::Path::new("target/safing.wasm"),
            "sequencer.toml",
            &plan,
            &walk,
            &calls,
            &Outcome::Exited(0),
            &[],
            None,
            Failed::Step(1),
        );
        assert!(
            message.contains("wrong order"),
            "should diagnose order:\n{message}"
        );
        assert!(message.contains("happened too early"), "{message}");
        assert!(
            message.contains("??"),
            "the call must be marked:\n{message}"
        );

        // Absent entirely.
        let calls = vec![command(0x10)];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "safing",
            std::path::Path::new("target/safing.wasm"),
            "sequencer.toml",
            &plan,
            &walk,
            &calls,
            &Outcome::Exited(0),
            &[],
            None,
            Failed::Step(1),
        );
        assert!(message.contains("never happened"), "{message}");
        assert!(!message.contains("wrong order"), "{message}");
    }

    #[test]
    fn message_locates_step_and_shows_transcript() {
        let mut plan = Context::default();
        plan.push_step(step(event().containing("safe mode").into_matcher()));
        let calls = vec![
            command(0x10),
            Call::Event {
                severity: 3,
                message: "bus off".into(),
                truncated: false,
            },
        ];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "safing",
            std::path::Path::new("target/safing.wasm"),
            "sequencer.toml",
            &plan,
            &walk,
            &calls,
            &Outcome::Exited(0),
            &[],
            None,
            Failed::Step(0),
        );

        assert!(message.contains("tests/safing.rs:12"), "{message}");
        assert!(
            message.contains("event containing \"safe mode\""),
            "{message}"
        );
        assert!(message.contains("what the sequence did"), "{message}");
        assert!(
            message.contains("event(WARNING_LO) \"bus off\""),
            "{message}"
        );
        assert!(
            message.contains("sequence exited successfully 0"),
            "{message}"
        );
        assert!(message.contains("fix:"), "{message}");
        assert!(message.contains("host call 1"), "{message}");
    }

    #[test]
    fn authors_note_reaches_message() {
        let mut only = step(cmd(0x10).into_matcher());
        only.note = Some("the retry after the first PWR_OFF fails");
        let mut plan = Context::default();
        plan.push_step(only);
        let walk = crate::plan::walk(&plan.steps, &[], &[]);
        let message = render(
            "safing",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &[],
            &Outcome::Exited(0),
            &[],
            None,
            Failed::Step(0),
        );
        assert!(
            message.contains("the retry after the first PWR_OFF fails"),
            "{message}"
        );
    }

    #[test]
    fn call_matched_by_earlier_step_is_not_misplaced() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.push_step(step(cmd(0x10).into_matcher()));

        let calls = vec![command(0x10)];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        assert_eq!(walk.matched, vec![Some(0), None]);
        assert_eq!(
            misplaced_at(&plan, &walk, &calls, 1),
            None,
            "the only matching call belongs to step 1"
        );

        let message = render(
            "worker",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &calls,
            &Outcome::Exited(0),
            &[],
            None,
            Failed::Step(1),
        );
        assert!(message.contains("never happened"), "{message}");
        assert!(!message.contains("wrong order"), "{message}");
        assert!(
            !message.contains("??"),
            "no call should be marked misplaced:\n{message}"
        );
    }

    #[test]
    fn call_before_previous_match_is_misplaced() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.push_step(step(cmd(0x20).into_matcher()));

        let calls = vec![command(0x20), command(0x10)];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        assert_eq!(misplaced_at(&plan, &walk, &calls, 1), Some(0));
    }

    #[test]
    fn boundary_says_who_ended_run() {
        let parked = vec![Call::SerialRecv {
            index: 2,
            blocking: true,
            received: 0,
            status: 1,
        }];

        let plan = Context::default();
        let sequence_parked = boundary(&plan, &parked, &Outcome::Suspended);
        assert!(
            sequence_parked.contains("waiting on serial port 2"),
            "{sequence_parked}"
        );
        assert!(sequence_parked.contains("forever"), "{sequence_parked}");

        let mut cut = Context::default();
        cut.stopped_at_call = Some(4);
        let by_the_test = boundary(&cut, &parked, &Outcome::Suspended);
        assert!(
            by_the_test.contains("stopped by this test"),
            "{by_the_test}"
        );
        assert!(by_the_test.contains("host call 4"), "{by_the_test}");

        assert_eq!(
            boundary(&plan, &[], &Outcome::Exited(0)),
            describe_outcome(&Outcome::Exited(0))
        );
    }

    #[test]
    fn deadlock_names_port_and_traffic() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.queue_serial(0, vec![1]);
        // One queued, one read, then parked.
        crate::interpreter::Script::serial_recv(&mut plan, 0, true);

        let calls = vec![
            Call::SerialRecv {
                index: 0,
                blocking: true,
                received: 4,
                status: 0,
            },
            Call::SerialRecv {
                index: 0,
                blocking: true,
                received: 0,
                status: 1,
            },
        ];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "worker",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &calls,
            &Outcome::Suspended,
            &[],
            None,
            Failed::Step(0),
        );

        assert!(message.contains("parked on serial port 0"), "{message}");
        assert!(message.contains("queued 1"), "{message}");
        assert!(message.contains("read 1"), "{message}");
        assert!(message.contains("initial_serial(0"), "{message}");
        assert!(message.contains("queues_serial(0"), "{message}");
    }

    #[test]
    fn trap_explained_in_hosts_words() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));

        let calls = vec![Call::SerialRecv {
            index: 0,
            blocking: true,
            received: 0,
            status: 0,
        }];
        let issues = vec![
            // Against an earlier call, so not this trap's reason.
            Issues {
                call: 0,
                kind: Kind::Info,
                message: "read as all zeroes".into(),
            },
            Issues {
                call: 0,
                kind: Kind::Warning,
                message: "the supplied message for serial port 0 is 8 bytes".into(),
            },
        ];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "worker",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &calls,
            &Outcome::Trapped("Host".into()),
            &issues,
            None,
            Failed::Step(0),
        );

        assert!(message.contains("trap reason, host call 0"), "{message}");
        assert!(message.contains("is 8 bytes"), "{message}");
        assert!(!message.contains("read as all zeroes"), "{message}");
    }

    #[test]
    fn untrapped_run_has_no_trap_section() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        let calls = vec![command(0x20)];
        let issues = vec![Issues {
            call: 0,
            kind: Kind::Warning,
            message: "serial_send on port 0 needs serialOutMax > 0".into(),
        }];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "worker",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &calls,
            &Outcome::Exited(0),
            &issues,
            None,
            Failed::Step(0),
        );
        assert!(!message.contains("trap reason"), "{message}");
        assert!(!message.contains("serialOutMax"), "{message}");
    }

    #[test]
    fn out_of_fuel_suggests_stop_after_last_step() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.require_outcome(Expected::StillRunning);

        let calls = vec![command(0x10)];
        let walk = crate::plan::walk(&plan.steps, &[], &calls);
        let message = render(
            "poller",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &calls,
            &Outcome::OutOfInstructions,
            &[],
            None,
            Failed::OutcomeOnly,
        );
        assert!(message.contains("instruction limit"), "{message}");
        assert!(message.contains("stop_after_last_step()"), "{message}");
    }

    #[test]
    fn message_shows_what_test_supplied() {
        let mut plan = Context::default();
        plan.push_step(step(cmd(0x10).into_matcher()));
        plan.give_telemetry(
            4,
            vec![vec![0]],
            Supplied {
                path: "Ref.power.BatteryVoltage",
                shown: "21.5".into(),
                reads: 0,
                is_parameter: false,
            },
        );
        let walk = crate::plan::walk(&plan.steps, &[], &[]);
        let message = render(
            "safing",
            std::path::Path::new("m.wasm"),
            "defaults",
            &plan,
            &walk,
            &[],
            &Outcome::Exited(0),
            &[],
            None,
            Failed::Step(0),
        );
        assert!(message.contains("what this test supplied"), "{message}");
        assert!(message.contains("Ref.power.BatteryVoltage"), "{message}");
        assert!(message.contains("21.5"), "{message}");
    }
}

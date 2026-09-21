//! Running one test.
//!
//! Build the plan from the author's body, find the compiled module, run it with the plan
//! installed as the host, then judge what happened. The plan is built first and completely,
//! which is what lets a `.sets_telemetry(..)` take effect *during* the run.

use crate::config;
use crate::interpreter;
use crate::interpreter::Script;
use crate::locate;
use crate::plan;
use crate::project::Project;
use crate::report;
use crate::test::Test;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

/// What `#[fprime_test]` knows at compile time about one test.
pub struct Case {
    /// The sequence this test is about, from `#[fprime_test(sequence = "...")]`.
    pub sequence: &'static str,
    /// The test function's own name, for the failure headline.
    pub test: &'static str,
    /// `CARGO_MANIFEST_DIR` — the sequence crate.
    pub manifest_dir: &'static str,
    /// `FPRIME_DICTIONARY`, emitted by `fprime_build` as a `cargo::rustc-env`.
    pub dictionary: &'static str,
    /// `#[fprime_test(limits = "...")]`: a `sequencer.toml` relative to [`Self::manifest_dir`],
    /// as the author wrote it. `None` is the project's own.
    pub limits: Option<&'static str>,
}

/// Reject a plan that cannot mean anything: `stop_after_last_step` with no steps, or without
/// `expect_still_running`, would pass while observing nothing.
fn coherent(test: &Test) -> anyhow::Result<()> {
    let plan = test.plan.borrow();
    if !plan.stop_after_steps {
        return Ok(());
    }
    if plan.steps.is_empty() {
        anyhow::bail!(
            "this test calls `stop_after_last_step()`, which stops the sequence once every \
             listed step has matched — but it lists no steps, so the run would end at the \
             first host call and the test would assert nothing.\n\
             Add the steps the sequence should reach, or drop `stop_after_last_step()`."
        );
    }
    if plan.outcome != Some(crate::plan::Expected::StillRunning) {
        anyhow::bail!(
            "this test calls `stop_after_last_step()`, which stops the sequence before it can \
             end — so no other ending is reachable and `expect_exit`/`expect_ok` can never \
             hold.\n\
             Use `t.expect_still_running();` for a sequence that never finishes, or drop \
             `stop_after_last_step()` if it does finish."
        );
    }
    Ok(())
}

/// Build the plan, run the sequence, and fail the test if they disagree.
#[track_caller]
pub fn run(case: Case, body: impl FnOnce(Test)) {
    let test = Test::new();
    body(test.clone());

    if let Err(err) = coherent(&test) {
        panic!("{}", report::could_not_run(case.sequence, &err));
    }

    let manifest_dir = Path::new(case.manifest_dir);
    let (module, bytes) = match locate::module(manifest_dir, case.sequence) {
        Ok(found) => found,
        Err(err) => panic!("{}", report::could_not_run(case.sequence, &err)),
    };

    let project = Project::find(manifest_dir);
    let loaded = match case.limits {
        // Named by the attribute, and joined the way the macro joined it to check it exists.
        Some(named) => config::load_named(manifest_dir, Path::new(named)),
        None => config::load(None, project.as_ref().ok()),
    }
    .unwrap_or_else(|err| {
        panic!(
            "{}",
            report::could_not_run(case.sequence, &anyhow::anyhow!("{err:#}"))
        )
    });
    let limits = {
        let plan = test.plan.borrow();
        // A test may hold the sequence to limits of its own, and raise the ceiling on a
        // long one. `t.limits(..)` wins over the attribute: it is the more specific of the two.
        let mut limits = plan.limits.unwrap_or(loaded.limits);
        if let Some(instructions) = plan.max_instructions {
            limits.max_instructions = instructions;
        }
        limits
    };

    // The plan *is* the host for this run. Coerced to `dyn Script` here; the concrete handle
    // stays in `test`, which is how the walk below reads back what the plan observed.
    let script: Rc<RefCell<dyn Script>> = test.plan.clone();
    let outcome = interpreter::run(bytes, &limits, Some(script));
    let report = match outcome {
        Ok(report) => report,
        Err(err) => panic!("{}", report::could_not_run(case.sequence, &err)),
    };

    let dictionary = fprime_dictionary::try_parse(Path::new(case.dictionary)).ok();

    let plan = test.plan.borrow();
    let walk = plan::walk(&plan.steps, &plan.forbidden, &report.recording.calls);
    if let Some(failed) = report::verdict(&plan, &walk, &report.outcome) {
        panic!(
            "{}",
            report::render(
                case.sequence,
                &module,
                &loaded.source.label(),
                &plan,
                &walk,
                &report.recording.calls,
                &report.outcome,
                &report.recording.issues,
                dictionary.as_ref(),
                failed,
            )
        );
    }
}

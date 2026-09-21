//! Driving a loaded module to its end.

use super::load::{Cost, Loaded, load};
use super::{GuestStack, Limits, Outcome, Recording, Report, Stop, host, stack, tracker};
use crate::abi;
use anyhow::{Result, anyhow, bail};
use spacewasm::{InterpreterResult, InterpreterRunner, MemoryKind};

/// Load `bytes`, run it as `Svc::WasmSequencer` would, and report what it cost.
///
/// `script` answers the host calls as the run proceeds; `None` leaves every answer
/// nominal — commands succeed, reads look valid, sleeps return at once.
pub fn run(
    bytes: Vec<u8>,
    limits: &Limits,
    script: Option<host::script::Shared>,
) -> Result<Report> {
    let shared = host::context(limits, script);
    let report = run_inner(bytes, limits, &shared);

    // Wanted even on failure: the calls made before it usually explain it.
    let recording = shared
        .into_recording()
        .map_err(|_| anyhow!("the host module outlived the run"))?;

    report.map(|mut report| {
        report.recording = recording;
        report
    })
}

fn run_inner(bytes: Vec<u8>, limits: &Limits, shared: &host::Ctx) -> Result<Report> {
    let Loaded {
        sizes: _,
        mut engine,
        code,
        module,
        entry,
        declared_memory,
        guest_pool,
    } = load(bytes, limits, shared)?;

    let cost = Cost::of(&code);
    // `CodeBuilder` is done writing once decoded; the interpreter only reads from here.
    let text = code.pages();

    // A `start` function runs before the entry point, as it would on board.
    if let Some(start) = engine.module_start(module) {
        engine
            .invoke(start, &[])
            .map_err(|err| anyhow!("could not invoke the start function: {err:?}"))?;
        match spacewasm::Interpreter.run(text, &mut engine, usize::MAX) {
            InterpreterResult::Finished => {}
            other => bail!("the module's start function did not complete: {other:?}"),
        }
    }

    // Dropped before invoking: `memory.grow` needs the `Rc<Memory>` uniquely held, so a
    // clone held across the run would trap any module that grows.
    let stack_region = {
        let memory = match &engine
            .store
            .modules()
            .last()
            .expect("module was pushed")
            .memory
        {
            Some(MemoryKind::Owned(memory)) => Some(memory.clone()),
            _ => None,
        };
        memory
            .as_ref()
            .and_then(|memory| stack::paint(memory, &engine, declared_memory))
    };

    engine
        .invoke(entry, &[])
        .map_err(|err| anyhow!("could not invoke `{}`: {err:?}", abi::ENTRY_POINT))?;

    let slice = limits.stack_sample.max(1);
    let mut peak_operand_stack = engine.sp;
    let mut instructions = 0u64;
    let outcome = loop {
        match spacewasm::Interpreter.run(text, &mut engine, slice) {
            InterpreterResult::OutOfFuel => {
                peak_operand_stack = peak_operand_stack.max(engine.sp);
                instructions += slice as u64;
                if instructions >= limits.max_instructions {
                    break Outcome::OutOfInstructions;
                }
            }
            InterpreterResult::Finished => {
                peak_operand_stack = peak_operand_stack.max(engine.sp);
                // `exit`/`panic` stop via a trap; `stop` distinguishes them from a real fault.
                break match shared.borrow().stop {
                    Some(Stop::Exit(code)) => Outcome::Exited(code),
                    Some(Stop::Panic(code)) => Outcome::Panicked(code),
                    None => Outcome::Returned,
                };
            }
            InterpreterResult::Trap(reason) => {
                // Sampled before unwinding resets `sp` to zero.
                break match shared.borrow().stop {
                    Some(Stop::Exit(code)) => Outcome::Exited(code),
                    Some(Stop::Panic(code)) => Outcome::Panicked(code),
                    None => Outcome::Trapped(format!("{reason:?}")),
                };
            }
            InterpreterResult::Pause => break Outcome::Suspended,
        }
    };

    // Re-acquired now growth (if any) is done; held earlier it would have blocked `grow`.
    let guest_stack = stack_region.map(|reserved| {
        let used = match &engine
            .store
            .modules()
            .last()
            .expect("module was pushed")
            .memory
        {
            Some(MemoryKind::Owned(memory)) => stack::used(memory, reserved),
            _ => 0,
        };
        GuestStack { reserved, used }
    });

    Ok(Report {
        outcome,
        usage: tracker().usage(),
        code_pages: cost.code_pages,
        code_words: cost.code_words,
        code_capacity: cost.code_capacity,
        instructions,
        peak_operand_stack,
        // Peak of the two: `GuestPool` sees nothing until memory is non-empty.
        guest_memory: declared_memory.max(guest_pool.peak() as u64),
        declared_memory,
        refused_grows: guest_pool.refused(),
        guest_stack,
        // Filled in by `run` once the host module has been dropped.
        recording: Recording::default(),
    })
}

//! Decoding, instantiating and driving one module.

use super::link::link_error;
use super::{
    GuestStack, Limits, Outcome, Recording, Report, Responses, Stop, alloc, host, stack, stream,
    tracker,
};
use crate::abi;
use crate::wasm;
use anyhow::{Context, Result, anyhow, bail};
use spacewasm::{
    CodeBuilder, CompilerOptions, Engine, ExportDesc, InterpreterResult, InterpreterRunner,
    MemoryKind, ModuleRef, Ref, WasmRef,
};

/// Validation limits, matching `spacewasm_c_api/include/config.rs`, which is the
/// configuration `WasmSequencer` builds against. A module that needs more than these
/// will not validate on board either.
const MAX_CONTROL_FRAMES: usize = 64;
const MAX_STACK_DEPTH: usize = 256;

/// A code page holds 256 16-bit words of resolved instructions.
const CODE_PAGE_WORDS: usize = 256;

/// Decode, instantiate and run `bytes`, then report what it cost.
///
/// `static_view` is the same module as read by [`crate::wasm`]; it supplies the
/// `-zstack-size` region and the import names, which `spacewasm`'s own decode does not
/// keep.
pub fn run(
    bytes: Vec<u8>,
    static_view: &wasm::Module,
    limits: &Limits,
    responses: Responses,
) -> Result<Report> {
    // Clears the counters and installs a page allocator of the configured size, so
    // consecutive modules are each measured on their own. Asserts nothing is still
    // allocated, which holds because every interpreter object from the previous call
    // was dropped when it returned.
    tracker().configure(limits.page_size);

    let shared = host::recording(responses);
    let report = run_inner(bytes, static_view, limits, &shared);

    // The recording is wanted even when the run failed part-way: the host calls made
    // before the failure are usually what explains it.
    let recording = std::rc::Rc::try_unwrap(shared)
        .map_err(|_| anyhow!("the host module outlived the run"))?
        .into_inner();

    report.map(|mut report| {
        report.recording = recording;
        report
    })
}

fn run_inner(
    bytes: Vec<u8>,
    static_view: &wasm::Module,
    limits: &Limits,
    shared: &host::Shared,
) -> Result<Report> {
    let mut code = CodeBuilder::new(CompilerOptions {
        // `WasmSequencerHelpers.cpp` sets this true: growth is allowed, and is then
        // bounded at run time by the guest bump pool, which returns -1 to the guest
        // rather than trapping. (A stale comment in `WasmSequencer.hpp` claims modules
        // are compiled with growth disabled; the code disagrees.) Compiling it out
        // here would fail a module that loads on board.
        allow_memory_grow: true,
        // Bounded as on board. `None` here would let a module load that fails at load
        // time with ERR_POSSIBLE_BACKPATCH_CYCLE on the sequencer.
        max_backpatch_iterations: Some(abi::MAX_BACKPATCH_ITERATIONS),
        max_code_pages: limits.max_code_pages,
    })
    .map_err(|err| anyhow!("could not allocate the code page table: {err:?}"))?;

    let mut engine = Engine::new(
        limits.stack_size,
        usize::from(limits.max_guest_modules),
        spacewasm::Vec::from_array([host::module(shared)])
            .map_err(|err| anyhow!("could not register the host module: {err:?}"))?,
    )
    .map_err(|err| anyhow!("could not create the interpreter: {err:?}"))?;

    // Guest linear memory comes from a pool the size of `Config::guestMemorySize`, so
    // a `memory.grow` past it is refused here as it would be on board.
    let guest_pool = std::rc::Rc::new(alloc::GuestPool::new(
        usize::try_from(limits.guest_memory).unwrap_or(usize::MAX),
    ));

    let mut source = stream::Bytes::new(bytes);
    let module = spacewasm::Module::new::<MAX_CONTROL_FRAMES, MAX_STACK_DEPTH>(
        "sequence",
        &mut source,
        &mut engine.store,
        &mut code,
        spacewasm::Rc::new(alloc::Guest::new(std::rc::Rc::clone(&guest_pool)))
            .map_err(|err| anyhow!("could not allocate the memory allocator: {err:?}"))?
            .into_wasm_memory_allocator(),
    )
    .map_err(|err| link_error(err, static_view))?;

    let declared_memory = match &module.memory {
        Some(MemoryKind::Owned(memory)) => {
            let page_size = memory.mem_type().page_size.size() as u64;
            u64::from(memory.size()) * page_size
        }
        // A sequence that touches no memory declares none; nothing to size.
        _ => 0,
    };

    let module_ref = engine
        .push_module(module)
        .map_err(|err| anyhow!("could not instantiate the module: {err:?}"))?;

    // Borrowed for the rest of the run: `CodeBuilder` is done being written to once the
    // module is decoded, and the interpreter only reads the pages.
    let text = code.pages();

    // A `start` function runs before the entry point, as it would on board.
    if let Some(start) = engine.module_start(module_ref) {
        engine
            .invoke(start, &[])
            .map_err(|err| anyhow!("could not invoke the start function: {err:?}"))?;
        match spacewasm::Interpreter.run(text, &mut engine, usize::MAX) {
            InterpreterResult::Finished => {}
            other => bail!("the module's start function did not complete: {other:?}"),
        }
    }

    let entry = entry_point(&engine)?;

    // Poison before invoking so the fill is not mistaken for stack use, and measure the
    // region from the module's own `__stack_pointer`.
    //
    // The reference is taken and dropped inside this block on purpose. `memory.grow`
    // needs the `Rc<Memory>` uniquely held — the interpreter drops its own reference and
    // then asks for `get_mut` — so a clone held across the run traps any module that
    // grows, with `MemoryRefNotUnique`.
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
                // `exit` and `panic` stop the interpreter through a trap, so a recorded
                // stop is what distinguishes them from a real fault.
                break match shared.borrow().stop {
                    Some(Stop::Exit(code)) => Outcome::Exited(code),
                    Some(Stop::Panic(code)) => Outcome::Panicked(code),
                    None => Outcome::Returned,
                };
            }
            InterpreterResult::Trap(reason) => {
                // Sampled before the trap: the interpreter resets `sp` when it unwinds,
                // so reading it afterwards would report zero.
                break match shared.borrow().stop {
                    Some(Stop::Exit(code)) => Outcome::Exited(code),
                    Some(Stop::Panic(code)) => Outcome::Panicked(code),
                    None => Outcome::Trapped(format!("{reason:?}")),
                };
            }
            InterpreterResult::Pause => break Outcome::Suspended,
        }
    };

    // Re-acquired now the run is over: growth may have moved the allocation, and holding
    // a reference across the run would have blocked it.
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

    let code_pages = text.len();
    let code_words = if code_pages == 0 {
        0
    } else {
        (code_pages - 1) * CODE_PAGE_WORDS + code.offset()
    };

    Ok(Report {
        outcome,
        usage: tracker().usage(),
        code_pages,
        code_words,
        code_capacity: code_pages * CODE_PAGE_WORDS,
        instructions,
        peak_operand_stack,
        // The peak, so growth counts toward the budget. `GuestPool` only sees an
        // allocation once the memory is non-empty, so a module that declares memory and
        // never grows reports its declared size here.
        guest_memory: declared_memory.max(guest_pool.peak() as u64),
        declared_memory,
        refused_grows: guest_pool.refused(),
        guest_stack,
        // Filled in by `run` once the host module has been dropped.
        recording: Recording::default(),
    })
}

/// Resolve the `main` export that `WasmSequencerController` invokes.
fn entry_point(engine: &Engine) -> Result<WasmRef> {
    let module = engine.store.modules().last().expect("module was pushed");
    let export = module
        .exports
        .iter()
        .find(|export| export.name == abi::ENTRY_POINT)
        .with_context(|| {
            format!(
                "the module exports no `{}`. A sequence needs `#[fprime_main]` on its entry \
                 point, which exports it under that name",
                abi::ENTRY_POINT
            )
        })?;
    let ExportDesc::Func(index) = export.desc else {
        bail!(
            "the module exports `{}`, but not as a function",
            abi::ENTRY_POINT
        );
    };
    let Some(Ref::Module(index)) = module.get_func_ref(index) else {
        bail!("`{}` does not resolve to a function", abi::ENTRY_POINT);
    };
    Ok(WasmRef {
        module: ModuleRef(0),
        index,
    })
}

//! Decoding and instantiating one module — everything up to the first instruction.
//!
//! This is all `fprime-wasm verify` does: a module that loads here is one the on-board
//! interpreter will accept, sized against the same `Config`. [`super::run`] carries the
//! same [`Loaded`] on into the entry point.

use super::link::{link_error, unsupported_error};
use super::{Limits, alloc, host, tracker};
use crate::abi;
use crate::wasm;
use anyhow::{Context, Result, anyhow, bail};
use spacewasm::{
    CodeBuilder, CompilerOptions, Engine, ExportDesc, MemoryKind, ModuleRef, Ref, WasmRef,
};

/// The `DecoderStack` `WasmSequencer` validates against: `spacewasm_c_api/include/config.rs`
/// up to 0.7.1, and the same lengths passed to `spacewasm_load_module` since.
const DECODER: spacewasm_tools::DecoderLimits = spacewasm_tools::DecoderLimits {
    max_control_frames: 64,
    max_stack_depth: 256,
};

/// A code page holds 256 16-bit words of resolved instructions.
const CODE_PAGE_WORDS: usize = 256;

/// A module decoded, instantiated and ready to invoke. Nothing has run yet.
pub struct Loaded {
    /// The static read of the binary: section sizes, imports, exports.
    pub sizes: wasm::Sizes,
    pub engine: Engine,
    /// The resolved instructions. Read-only from here: only the decoder writes pages.
    pub code: CodeBuilder,
    /// The instantiated module, for [`Engine::module_start`].
    pub module: ModuleRef,
    /// The `main` export `WasmSequencerController` invokes.
    pub entry: WasmRef,
    /// Linear memory the module declares, in bytes.
    pub declared_memory: u64,
    /// The pool linear memory is served from, so growth can be sized afterwards.
    pub guest_pool: std::rc::Rc<alloc::GuestPool>,
}

/// How much of the code page table a module took up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Code pages the module compiled to, against [`Limits::max_code_pages`].
    pub code_pages: usize,
    /// 16-bit instruction words used across those pages, and the words they hold.
    pub code_words: usize,
    pub code_capacity: usize,
}

impl Cost {
    pub fn of(code: &CodeBuilder) -> Cost {
        let code_pages = code.pages().len();
        Cost {
            code_pages,
            code_words: match code_pages {
                0 => 0,
                pages => (pages - 1) * CODE_PAGE_WORDS + code.offset(),
            },
            code_capacity: code_pages * CODE_PAGE_WORDS,
        }
    }
}

/// What loading one module measured. No code ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Validated {
    pub sizes: wasm::Sizes,
    pub cost: Cost,
    /// Linear memory the module declares, in bytes. Growth needs a run.
    pub declared_memory: u64,
    /// Interpreter heap the load consumed — the code pages and the module's own
    /// structures, which is nearly all of it.
    pub usage: alloc::Usage,
}

/// Decode `bytes` as the on-board interpreter would and record, for every compiled
/// instruction, its first IR word and the module offset of its opcode.
///
/// IR words count from the module's first. On board other modules may have compiled
/// before it, so a PC from there must first be made relative to where its code starts.
pub fn ir_map(bytes: &[u8], limits: &Limits) -> Result<spacewasm_tools::IrMap> {
    // Configured before anything allocates, as for a load: the replay decodes through the
    // same process-global allocator.
    tracker().configure(limits.page_size);
    let shared = host::context(limits, None);
    let hosts = spacewasm::Vec::from_array([host::module(&shared)])
        .map_err(|err| anyhow!("could not register the host module: {err:?}"))?;
    spacewasm_tools::ir_map(bytes, compiler_options(limits), hosts, DECODER)
        .map_err(|err| anyhow!("{err}"))
}

/// What `WasmSequencerHelpers.cpp` builds its `spacewasm_compiler_options_t` from.
fn compiler_options(limits: &Limits) -> CompilerOptions {
    CompilerOptions::new(
        // Growth is bounded by the guest pool instead.
        true,
        // `None` would accept modules that fail on board with ERR_POSSIBLE_BACKPATCH_CYCLE.
        Some(abi::MAX_BACKPATCH_ITERATIONS),
        limits.max_code_pages,
    )
}

/// Load `bytes`, measure what that cost, and throw the module away.
///
/// This is `fprime-wasm verify`: everything the on-board interpreter checks before the
/// first instruction, and nothing that needs one.
pub fn validate(bytes: Vec<u8>, limits: &Limits) -> Result<Validated> {
    // Nominal host, never called: the module still has to link against it to decode.
    let shared = host::context(limits, None);
    let loaded = load(bytes, limits, &shared)?;
    let validated = Validated {
        sizes: loaded.sizes,
        cost: Cost::of(&loaded.code),
        declared_memory: loaded.declared_memory,
        // Read while the module is still alive; the next `load` resets it.
        usage: tracker().usage(),
    };
    Ok(validated)
}

/// Decode and instantiate `bytes`, with `shared` standing in for `Svc::WasmSequencer`.
///
/// Resets the interpreter heap first, so nothing from a previous module is still live —
/// which is why only one load may be in flight at a time.
pub fn load(bytes: Vec<u8>, limits: &Limits, shared: &host::Ctx) -> Result<Loaded> {
    // Read before `bytes` is consumed: the names `spacewasm` discards are what a link
    // failure has to name.
    let static_view = wasm::read(&bytes).map_err(|err| anyhow!("{err:#}"))?;
    // Only looked at if the decoder refuses the module, but `bytes` is consumed by then.
    // An older interpreter's refusal has to be found first: this decoder would accept it.
    let unsupported = match limits.fprime.older_interpreter() {
        Some(release) => match spacewasm_tools::find_unsupported_in(&bytes, release) {
            Some(unsupported) => return Err(unsupported_error(&unsupported, limits)),
            None => None,
        },
        None => spacewasm_tools::find_unsupported(&bytes),
    };

    tracker().configure(limits.page_size);

    let mut code = CodeBuilder::new(compiler_options(limits))
        .map_err(|err| anyhow!("could not allocate the code page table: {err:?}"))?;

    let mut engine = Engine::new(
        limits.stack_size,
        usize::from(limits.max_guest_modules),
        spacewasm::Vec::from_array([host::module(shared)])
            .map_err(|err| anyhow!("could not register the host module: {err:?}"))?,
    )
    .map_err(|err| anyhow!("could not create the interpreter: {err:?}"))?;

    // Sized to `Config::guestMemorySize`; growth past it is refused as on board.
    let guest_pool = std::rc::Rc::new(alloc::GuestPool::new(
        usize::try_from(limits.guest_memory).unwrap_or(usize::MAX),
    ));

    let mut source = super::stream::Bytes::new(bytes);
    let memory = spacewasm::Rc::new(alloc::Guest::new(std::rc::Rc::clone(&guest_pool)))
        .map_err(|err| anyhow!("could not allocate the memory allocator: {err:?}"))?
        .into_wasm_memory_allocator();
    let module = DECODER
        .with_stack(|stack| {
            spacewasm::Module::new(
                "sequence",
                &mut source,
                &mut engine.store,
                &mut code,
                stack,
                memory,
            )
        })
        .map_err(|err| link_error(err, &static_view, unsupported))?;

    let declared_memory = match &module.memory {
        Some(MemoryKind::Owned(memory)) => {
            let page_size = memory.mem_type().page_size.size() as u64;
            u64::from(memory.size()) * page_size
        }
        // A sequence that touches no memory declares none; nothing to size.
        _ => 0,
    };

    let module = engine
        .push_module(module)
        .map_err(|err| anyhow!("could not instantiate the module: {err:?}"))?;

    let entry = entry_point(&engine)?;

    Ok(Loaded {
        sizes: static_view.sizes,
        engine,
        code,
        module,
        entry,
        declared_memory,
        guest_pool,
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

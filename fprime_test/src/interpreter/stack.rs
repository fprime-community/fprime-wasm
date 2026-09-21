//! The guest stack high-water mark.
//!
//! `rustc` links `wasm32v1-none` stack-first, so the stack occupies the bottom of
//! linear memory and grows down from the initial `__stack_pointer`, with the data
//! segments above it. That makes `[0, __stack_pointer)` pure scratch — `.bss` lives
//! above the data, never below — so it can be poisoned before the run and the
//! untouched bytes counted afterwards.

use spacewasm::{Engine, Memory};

/// Byte written over the guest's stack region before the run, so the bytes it did not
/// touch can be told from the bytes it did. Any non-zero value works; this one is
/// conventional and unlikely to be a value the sequence would store.
const POISON: u8 = 0xA5;

/// Fill the guest's stack region with [`POISON`], returning its size.
///
/// Returns `None` when the module declares no mutable `i32` global, which means the
/// linker found no need for a stack pointer and the sequence keeps everything in the
/// operand stack.
pub(super) fn paint(memory: &Memory, engine: &Engine, guest_memory: u64) -> Option<usize> {
    let module = engine.store.modules().last().expect("module was pushed");
    // Read before the run: these globals are mutable and `__stack_pointer` is exactly
    // the one the sequence decrements.
    let top = module
        .globals
        .iter()
        .filter_map(|global| match global.value() {
            spacewasm::Value::I32(value) if global.type_.mutable && value > 0 => {
                usize::try_from(value).ok()
            }
            _ => None,
        })
        .max()?;

    if top as u64 > guest_memory {
        return None;
    }
    let slice = memory.get_slice();
    // If anything down here is already non-zero, a data segment claimed part of the
    // region and the stack-first assumption does not hold. Leave it alone.
    if slice.len() < top || slice[..top].iter().any(|byte| *byte != 0) {
        return None;
    }
    for address in 0..top {
        memory
            .store_u8(address, POISON)
            .expect("address is within the slice checked above");
    }
    Some(top)
}

/// How far into a poisoned stack region the sequence wrote.
///
/// The stack grows down from `reserved`, so the untouched bytes are the ones still
/// holding [`POISON`] at the bottom, and the high-water mark is whatever is left above
/// them.
pub(super) fn used(memory: &Memory, reserved: usize) -> usize {
    let slice = memory.get_slice();
    let untouched = slice[..reserved.min(slice.len())]
        .iter()
        .take_while(|byte| **byte == POISON)
        .count();
    reserved.saturating_sub(untouched)
}

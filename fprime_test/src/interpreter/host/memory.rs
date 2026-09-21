//! Reading guest memory and reading arguments off the operand stack.
//!
//! A pointer the guest got wrong traps the guest, exactly as an out-of-bounds host
//! read would on board, and never fails `verify` itself.

use spacewasm::{Engine, HostFunctionBreak, Value};

/// `spacewasm` validates arity and value types against the registered signature
/// before dispatching, so a mismatch here is a bug in [`crate::abi::FUNCTIONS`]
/// rather than anything the guest did.
pub(super) fn i32_at(params: &[Value], index: usize) -> i32 {
    match params.get(index) {
        Some(Value::I32(v)) => *v,
        other => panic!("expected an i32 at parameter {index}, found {other:?}"),
    }
}

pub(super) fn i64_at(params: &[Value], index: usize) -> i64 {
    match params.get(index) {
        Some(Value::I64(v)) => *v,
        other => panic!("expected an i64 at parameter {index}, found {other:?}"),
    }
}

/// Read `len` bytes of guest memory.
pub(super) fn load(state: &Engine, ptr: i32, len: i32) -> Result<Vec<u8>, HostFunctionBreak> {
    let (Ok(ptr), Ok(len)) = (usize::try_from(ptr), usize::try_from(len)) else {
        return Err(HostFunctionBreak::Trap);
    };
    state
        .memory
        .load(ptr, len)
        .map(<[u8]>::to_vec)
        .map_err(|_| HostFunctionBreak::Trap)
}

/// Write `data` into guest memory at `ptr`.
pub(super) fn store(state: &Engine, ptr: i32, data: &[u8]) -> Result<(), HostFunctionBreak> {
    let Ok(ptr) = usize::try_from(ptr) else {
        return Err(HostFunctionBreak::Trap);
    };
    state
        .memory
        .store(ptr, data)
        .map_err(|_| HostFunctionBreak::Trap)
}

/// Fill `len` bytes at `ptr` with zeroes. A zero-filled `Fw::Time` is a valid
/// serialised time whatever `Fw::TimeValue`'s width is in the dictionary, which is
/// what lets `time` and `tlm` work without knowing the deployment's layout.
pub(super) fn zero(state: &Engine, ptr: i32, len: i32) -> Result<(), HostFunctionBreak> {
    let Ok(len) = usize::try_from(len) else {
        return Err(HostFunctionBreak::Trap);
    };
    store(state, ptr, &vec![0u8; len])
}

/// Write a serialised value into a guest buffer of exactly `len` bytes.
///
/// A canned value shorter than the buffer is zero-padded on the right and one longer
/// is rejected, matching the host: `WasmSequencer` traps rather than overrun a guest
/// buffer.
pub(super) fn store_value(
    state: &Engine,
    ptr: i32,
    len: i32,
    value: &[u8],
) -> Result<(), HostFunctionBreak> {
    let Ok(len) = usize::try_from(len) else {
        return Err(HostFunctionBreak::Trap);
    };
    if value.len() > len {
        return Err(HostFunctionBreak::Trap);
    }
    let mut buffer = vec![0u8; len];
    buffer[..value.len()].copy_from_slice(value);
    store(state, ptr, &buffer)
}

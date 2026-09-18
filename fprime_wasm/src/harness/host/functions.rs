//! The twelve functions `WasmSequencer` registers.
//!
//! Each is registered with the signature [`crate::abi::FUNCTIONS`] gives it, so a
//! module that links against this host module links against the on-board one. The
//! checks that trap the guest are in [`super::traps`]; divergences the guest cannot
//! see are recorded through [`super::record`].

use super::memory::{i32_at, i64_at, load, store, store_value, zero};
use super::traps::{bad_id, bad_port, bad_time_len, sleep_too_long};
use super::{Call, Shared, Stop};
use crate::abi;
use spacewasm::{Engine, HostFunction, HostFunctionBreak, HostFunctionResult, HostModule, Value};
use std::ops::ControlFlow;
use std::rc::Rc;

/// Register one function under the name and signature the ABI gives it, so a
/// handler cannot silently disagree with the contract.
macro_rules! host_function {
    ($shared:expr, $name:literal, $handler:expr) => {{
        let function =
            abi::function($name).unwrap_or_else(|| panic!("{} is not in abi::FUNCTIONS", $name));
        let shared = Rc::clone($shared);
        HostFunction::new(
            function.name,
            function.params.into(),
            function.returns.into(),
            move |state: &mut Engine, params: &[Value]| -> HostFunctionResult {
                #[allow(clippy::redundant_closure_call)]
                $handler(&shared, state, params)
            },
        )
    }};
}

/// Build the host module. The returned module borrows `shared`, so the recording can
/// be read once the run is over.
pub fn module(shared: &Shared) -> HostModule {
    HostModule {
        name: abi::MODULE.into(),
        globals: spacewasm::vec![],
        functions: spacewasm::vec![
            host_function!(
                shared,
                "exit",
                |shared: &Shared, _: &mut Engine, params: &[Value]| {
                    let code = i32_at(params, 0);
                    let mut recording = shared.borrow_mut();
                    recording.push(Call::Exit { code });
                    recording.stop = Some(Stop::Exit(code));
                    // `exit` is `-> !`: the guest never resumes. A break is how a
                    // host function stops the interpreter.
                    ControlFlow::Break(HostFunctionBreak::Trap)
                }
            ),
            host_function!(
                shared,
                "panic",
                |shared: &Shared, _: &mut Engine, params: &[Value]| {
                    let code = i32_at(params, 0);
                    let mut recording = shared.borrow_mut();
                    recording.push(Call::Panic { code });
                    recording.stop = Some(Stop::Panic(code));
                    ControlFlow::Break(HostFunctionBreak::Trap)
                }
            ),
            host_function!(
                shared,
                "args",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let ptr = i32_at(params, 0);
                    let capacity = i32_at(params, 1);
                    let mut recording = shared.borrow_mut();
                    let args = recording.responses.args.clone();

                    // Documented contract: a buffer smaller than the arguments traps.
                    if capacity < 0 || (capacity as usize) < args.len() {
                        recording.push(Call::Args {
                            capacity: capacity.unsigned_abs(),
                            written: 0,
                        });
                        recording.warn(format!(
                            "the sequence offered a {capacity}-byte buffer for {} bytes of \
                         arguments; the sequencer traps rather than truncate",
                            args.len()
                        ));
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }

                    if let Err(brk) = store(state, ptr, &args) {
                        return ControlFlow::Break(brk);
                    }
                    recording.push(Call::Args {
                        capacity: capacity.unsigned_abs(),
                        written: args.len() as u32,
                    });
                    ControlFlow::Continue(Some(Value::I32(args.len() as i32)))
                }
            ),
            host_function!(
                shared,
                "time",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let ptr = i32_at(params, 0);
                    let len = i32_at(params, 1);
                    let mut recording = shared.borrow_mut();
                    recording.push(Call::Time {
                        len: len.unsigned_abs(),
                    });
                    if let Some(message) = bad_time_len(len, "time") {
                        recording.warn(message);
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    drop(recording);
                    if let Err(brk) = zero(state, ptr, len) {
                        return ControlFlow::Break(brk);
                    }
                    ControlFlow::Continue(None)
                }
            ),
            host_function!(
                shared,
                "tlm",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let id = i64_at(params, 0);
                    let time_ptr = i32_at(params, 1);
                    let time_len = i32_at(params, 2);
                    let value_ptr = i32_at(params, 3);
                    let value_len = i32_at(params, 4);

                    let mut recording = shared.borrow_mut();
                    if let Some(message) =
                        bad_time_len(time_len, "tlm").or_else(|| bad_id(id, "telemetry channel"))
                    {
                        recording.push(Call::Telemetry {
                            id,
                            value_len: value_len.unsigned_abs(),
                            status: abi::TLM_VALID,
                        });
                        recording.warn(message);
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    if let Err(brk) = zero(state, time_ptr, time_len) {
                        return ControlFlow::Break(brk);
                    }
                    let canned = recording.responses.telemetry.get(&id).cloned();
                    let write = match &canned {
                        Some(value) => store_value(state, value_ptr, value_len, value),
                        None => zero(state, value_ptr, value_len),
                    };
                    if let Err(brk) = write {
                        if canned.is_some() {
                            recording.warn(format!(
                                "the supplied value for telemetry channel {id} does not fit the \
                             {value_len}-byte buffer the sequence provided"
                            ));
                        }
                        return ControlFlow::Break(brk);
                    }

                    recording.push(Call::Telemetry {
                        id,
                        value_len: value_len.unsigned_abs(),
                        status: abi::TLM_VALID,
                    });
                    if canned.is_none() {
                        recording.inform(format!(
                            "telemetry channel {id} read as all zeroes; supply --tlm {id}=<hex> to \
                         drive the sequence down another branch"
                        ));
                    }
                    ControlFlow::Continue(Some(Value::I32(abi::TLM_VALID)))
                }
            ),
            host_function!(
                shared,
                "prm",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let id = i64_at(params, 0);
                    let value_ptr = i32_at(params, 1);
                    let value_len = i32_at(params, 2);

                    let mut recording = shared.borrow_mut();
                    if let Some(message) = bad_id(id, "parameter") {
                        recording.push(Call::Parameter {
                            id,
                            value_len: value_len.unsigned_abs(),
                            status: abi::PARAM_VALID,
                        });
                        recording.warn(message);
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    let canned = recording.responses.parameters.get(&id).cloned();
                    let write = match &canned {
                        Some(value) => store_value(state, value_ptr, value_len, value),
                        None => zero(state, value_ptr, value_len),
                    };
                    if let Err(brk) = write {
                        if canned.is_some() {
                            recording.warn(format!(
                                "the supplied value for parameter {id} does not fit the \
                             {value_len}-byte buffer the sequence provided"
                            ));
                        }
                        return ControlFlow::Break(brk);
                    }

                    recording.push(Call::Parameter {
                        id,
                        value_len: value_len.unsigned_abs(),
                        status: abi::PARAM_VALID,
                    });
                    if canned.is_none() {
                        recording.inform(format!(
                        "parameter {id} read as all zeroes; supply --prm {id}=<hex> to drive the \
                         sequence down another branch"
                    ));
                    }
                    ControlFlow::Continue(Some(Value::I32(abi::PARAM_VALID)))
                }
            ),
            host_function!(
                shared,
                "cmd",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let ptr = i32_at(params, 0);
                    let len = i32_at(params, 1);
                    let payload = match load(state, ptr, len) {
                        Ok(payload) => payload,
                        Err(brk) => return ControlFlow::Break(brk),
                    };

                    let mut recording = shared.borrow_mut();
                    if payload.len() as u64 > u64::from(abi::CMD_MAX_PAYLOAD) {
                        recording.push(Call::Command {
                            opcode: 0,
                            payload,
                            response: abi::CMD_RESPONSE_OK,
                        });
                        recording.warn(format!(
                            "a command of {len} bytes exceeds the {} the Com buffer can carry; \
                             the sequencer traps the guest",
                            abi::CMD_MAX_PAYLOAD
                        ));
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    if payload.len() < abi::OPCODE_BYTES {
                        recording.push(Call::Command {
                            opcode: 0,
                            payload,
                            response: abi::CMD_RESPONSE_OK,
                        });
                        recording.warn(format!(
                            "a command was dispatched with {len} bytes, too few for the \
                         {}-byte opcode; the dispatcher would reject it",
                            abi::OPCODE_BYTES
                        ));
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }

                    // F Prime serialises big-endian, opcode first.
                    let opcode = u32::from_be_bytes(
                        payload[..abi::OPCODE_BYTES]
                            .try_into()
                            .expect("checked above"),
                    );
                    recording.push(Call::Command {
                        opcode,
                        payload: payload[abi::OPCODE_BYTES..].to_vec(),
                        response: abi::CMD_RESPONSE_OK,
                    });
                    ControlFlow::Continue(Some(Value::I32(abi::CMD_RESPONSE_OK)))
                }
            ),
            host_function!(
                shared,
                "event",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let severity = i32_at(params, 0);
                    let ptr = i32_at(params, 1);
                    let len = i32_at(params, 2);
                    let bytes = match load(state, ptr, len) {
                        Ok(bytes) => bytes,
                        Err(brk) => return ControlFlow::Break(brk),
                    };

                    let mut recording = shared.borrow_mut();
                    let limit = recording.responses.event_message_max;
                    let truncated = bytes.len() > limit;
                    let kept = &bytes[..bytes.len().min(limit)];
                    let message = String::from_utf8_lossy(kept).into_owned();

                    recording.push(Call::Event {
                        severity,
                        message: message.clone(),
                        truncated,
                    });
                    if truncated {
                        recording.warn(format!(
                            "an event message of {} bytes is truncated to {limit}; the guest is \
                         not told",
                            bytes.len()
                        ));
                    }
                    let classified = abi::severity(severity);
                    if classified.is_rejected() {
                        recording.warn(format!(
                            "an event asked for severity {severity} ({}), which the sequencer \
                         replaces with HostFunctionInvalidSeverity; the event is lost and the \
                         guest continues",
                            classified.name()
                        ));
                    }
                    ControlFlow::Continue(None)
                }
            ),
            host_function!(
                shared,
                "rsleep",
                |shared: &Shared, _: &mut Engine, params: &[Value]| {
                    let us = i64_at(params, 0) as u64;
                    let mut recording = shared.borrow_mut();
                    recording.push(Call::RelativeSleep { us });
                    if let Some(message) = sleep_too_long(us, "relative") {
                        recording.warn(message);
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    ControlFlow::Continue(None)
                }
            ),
            host_function!(
                shared,
                "asleep",
                |shared: &Shared, _: &mut Engine, params: &[Value]| {
                    let us = i64_at(params, 0) as u64;
                    let mut recording = shared.borrow_mut();
                    recording.push(Call::AbsoluteSleep { us });
                    // `wasmAsleep` applies the same bound as `wasmRsleep`.
                    if let Some(message) = sleep_too_long(us, "absolute") {
                        recording.warn(message);
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    ControlFlow::Continue(None)
                }
            ),
            host_function!(
                shared,
                "serial_send",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let index = i32_at(params, 0);
                    let ptr = i32_at(params, 1);
                    let len = i32_at(params, 2);
                    {
                        let mut recording = shared.borrow_mut();
                        let ports = recording.responses.serial_ports;
                        if let Some(message) = bad_port(index, ports, "serial_send") {
                            recording.push(Call::SerialSend {
                                index,
                                len: len.unsigned_abs(),
                            });
                            recording.warn(message);
                            return ControlFlow::Break(HostFunctionBreak::Trap);
                        }
                    }
                    // Read it even though it is not kept, so a bad pointer traps here
                    // exactly as it would when the host copies the payload out.
                    if let Err(brk) = load(state, ptr, len) {
                        return ControlFlow::Break(brk);
                    }
                    let mut recording = shared.borrow_mut();
                    recording.push(Call::SerialSend {
                        index,
                        len: len.unsigned_abs(),
                    });
                    recording.warn(format!(
                        "serial_send on port {index} needs serialOutMax > 0 and the serialOut port \
                     connected, or the sequencer traps the guest"
                    ));
                    ControlFlow::Continue(None)
                }
            ),
            host_function!(
                shared,
                "serial_recv",
                |shared: &Shared, state: &mut Engine, params: &[Value]| {
                    let index = i32_at(params, 0);
                    let data_ptr = i32_at(params, 1);
                    let data_size = i32_at(params, 2);
                    let actual_size_ptr = i32_at(params, 3);
                    let raw_block = i32_at(params, 4);
                    let blocking = raw_block == abi::BLOCKING;

                    let mut recording = shared.borrow_mut();
                    let ports = recording.responses.serial_ports;
                    // `Svc::BlockState` has exactly two values; anything else traps
                    // rather than being read as non-blocking.
                    let bad_block = (raw_block != abi::BLOCKING && raw_block != 1).then(|| {
                        format!(
                            "serial_recv was given block_type {raw_block}; only 0 (blocking) and \
                             1 (non-blocking) are valid and the sequencer traps on anything else"
                        )
                    });
                    if let Some(message) = bad_port(index, ports, "serial_recv").or(bad_block) {
                        recording.push(Call::SerialRecv {
                            index,
                            blocking,
                            received: 0,
                            status: abi::QUEUE_EMPTY,
                        });
                        recording.warn(message);
                        return ControlFlow::Break(HostFunctionBreak::Trap);
                    }
                    let drained = recording
                        .serial_drained
                        .get(&index)
                        .copied()
                        .unwrap_or(false);
                    let pending = if drained {
                        None
                    } else {
                        recording.responses.serial.get(&index).cloned()
                    };

                    match pending {
                        Some(message) => {
                            if message.len() > data_size.max(0) as usize {
                                recording.push(Call::SerialRecv {
                                    index,
                                    blocking,
                                    received: 0,
                                    status: abi::QUEUE_OK,
                                });
                                recording.warn(format!(
                                "the supplied message for serial port {index} is {} bytes, more \
                                 than the {data_size}-byte buffer the sequence provided; the \
                                 sequencer traps rather than overrun it",
                                message.len()
                            ));
                                return ControlFlow::Break(HostFunctionBreak::Trap);
                            }
                            if let Err(brk) = store(state, data_ptr, &message) {
                                return ControlFlow::Break(brk);
                            }
                            // The written length is little-endian, unlike F Prime's
                            // own serialisation.
                            let written = (message.len() as u32).to_le_bytes();
                            if let Err(brk) = store(state, actual_size_ptr, &written) {
                                return ControlFlow::Break(brk);
                            }
                            recording.serial_drained.insert(index, true);
                            recording.push(Call::SerialRecv {
                                index,
                                blocking,
                                received: message.len() as u32,
                                status: abi::QUEUE_OK,
                            });
                            ControlFlow::Continue(Some(Value::I32(abi::QUEUE_OK)))
                        }
                        None => {
                            if let Err(brk) = store(state, actual_size_ptr, &0u32.to_le_bytes()) {
                                return ControlFlow::Break(brk);
                            }
                            recording.push(Call::SerialRecv {
                                index,
                                blocking,
                                received: 0,
                                status: abi::QUEUE_EMPTY,
                            });
                            if blocking {
                                recording.warn(format!(
                                "the sequence blocks on serial port {index} with nothing queued; \
                                 on board it would wait indefinitely. Supply --serial \
                                 {index}=<hex> to deliver a message"
                            ));
                                // Pause is what the sequencer does while a host call
                                // is outstanding, and it stops the run without
                                // claiming the guest failed.
                                return ControlFlow::Break(HostFunctionBreak::Pause);
                            }
                            ControlFlow::Continue(Some(Value::I32(abi::QUEUE_EMPTY)))
                        }
                    }
                }
            ),
        ],
        memory: spacewasm::Vec::zero(),
        table: spacewasm::Vec::zero(),
    }
}

// The tests that build a real `HostModule` live in `tests/harness.rs`: constructing
// one allocates through `spacewasm`'s process-global, single-threaded allocator,
// which the parallel unit-test harness cannot safely share.

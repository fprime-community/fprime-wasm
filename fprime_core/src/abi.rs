#[link(wasm_import_module = "fprime_v1")]
unsafe extern "C" {
    /// Exit the runtime given a status.
    /// This function should not return and should stop the WASM runtime
    ///
    /// # Arguments
    ///
    /// * `code`: Exit code signaling status
    ///
    /// returns: ! Never returns
    pub(crate) unsafe fn exit(code: i32) -> !;

    /// Exit the current Wasm program with a failure.
    /// This function does not return.
    ///
    /// # Arguments
    ///
    /// * `code`: Arbitrary code to indicate source of the panic
    pub(crate) unsafe fn panic(code: i32) -> !;

    /// Get the sequence arguments this sequence was invoked with
    /// This function will write the arguments provided from the invoke/run
    /// into the guest memory.
    ///
    /// # Arguments
    ///
    /// * `destination_ptr`: Guest memory address to argument buffer
    /// * `destination_size`: Length of the argument buffer. This size must be greater than
    ///   or equal to the length of arguments passed to the sequence otherwise the
    ///   interpreter will trap
    ///
    /// returns: u32 (number of bytes written to `destination_ptr`)
    #[allow(dead_code)]
    pub(crate) unsafe fn args(destination_ptr: u32, destination_size: u32) -> u32;

    /// Read the current F´ system time into guest memory
    ///
    /// The host serializes an Fw::Time into the guest buffer.
    ///
    /// # Arguments
    ///
    /// * `time_ptr`: Guest memory address to write the serialized time
    /// * `time_size`: Size allocated for time_ptr, must equal Fw::Time::SERIALIZED_SIZE
    pub(crate) unsafe fn time(time_ptr: u32, time_size: u32);

    /// Read a telemetry channel value and write it to the specified memory addresses
    ///
    /// # Arguments
    ///
    /// * `id`: Channel ID to read
    /// * `time_ptr`: Guest memory address to write channel update time
    /// * `time_size`: Size allocated for time_ptr, should be Fw::Time::SERIALIZED_SIZE
    /// * `value_ptr`: Guest memory address to channel's value
    /// * `value_size`: Size allocated for value_ptr
    ///
    /// returns: i32 (Fw::TlmValid)
    pub(crate) unsafe fn tlm(
        id: i64,
        time_ptr: u32,
        time_size: u32,
        value_ptr: u32,
        value_size: u32,
    ) -> i32;

    /// Read a parameter value and write it to the specified memory addresses
    ///
    /// # Arguments
    ///
    /// * `id`: Parameter ID to read
    /// * `value_ptr`: Guest memory address to parameter's value
    /// * `value_size`: Size allocated for value_ptr
    ///
    /// returns: i32 (Fw::ParamValid). Value bytes are written when the parameter is
    /// present. Unlike telemetry validity, this is a four-state encoding where a valid
    /// parameter is 1, not 0.
    pub(crate) unsafe fn prm(id: i64, value_ptr: u32, value_size: u32) -> i32;

    /// Dispatch a command, blocking call.
    ///
    /// Command must be encoded with the FwOpcodeType + arguments.
    /// A FORMAT_ERROR will be returned if the format is not valid!
    ///
    /// # Arguments
    ///
    /// * `buf_ptr`: Guest memory address to encoded command
    /// * `buf_size`: Size allocated for value_ptr
    ///
    /// returns: i32 (Fw::CmdResponse)
    pub(crate) unsafe fn cmd(buf_ptr: u32, buf_size: u32) -> i32;

    /// Emit an event from the current WasmSequencer component at a given severity level
    ///
    /// # Arguments
    ///
    /// * `severity`: Event severity level to emit
    /// * `msg_ptr`: Guest memory address to event message string
    /// * `msg_size`: Size allocated for value_ptr
    pub(crate) unsafe fn event(severity: i32, msg_ptr: u32, msg_size: u32);

    /// Pause the runtime for a specified time
    ///
    /// # Arguments
    ///
    /// * `us`: Microseconds to pause the runtime for
    pub(crate) unsafe fn rsleep(us: u64);

    /// Pause the runtime until a specified time
    ///
    /// # Arguments
    ///
    /// * `us`: Microseconds from system epoch to pause until
    pub(crate) unsafe fn asleep(us: u64);

    /// Invoke a serial port
    /// If the port is not connected, the module will panic/trap
    ///
    /// # Arguments
    ///
    /// * `index`: Port index to emit on the serialOut on
    /// * `data_ptr`: Pointer to the data to send on the output port
    /// * `data_size`: Length of the data to send on the output port
    pub(crate) unsafe fn serial_send(index: i32, data_ptr: u32, data_size: u32);

    /// Receive a message from a serial input queue
    ///
    /// # Arguments
    ///
    /// * `index`: Port number/queue index to receive message from
    /// * `data_ptr`: Pointer to the destination to write serial messages into
    /// * `data_size`: Size of the data_ptr memory. If a message is larger than this size,
    ///   trap the wasm module
    /// * `actual_size_ptr`: On message receive, number of bytes received will be written
    ///   here (little endian)
    /// * `block_type`: Whether or not to block for a message when the queue on this index
    ///   is empty (FprimeBlockingType: 0 = blocking, 1 = non-blocking)
    ///
    /// returns: i32 (FprimeQueueStatus). Status on whether or not a message was received
    /// (blocking always returns OK = 0; EMPTY = 1 for a non-blocking call on an empty
    /// queue).
    pub(crate) unsafe fn serial_recv(
        index: i32,
        data_ptr: u32,
        data_size: u32,
        actual_size_ptr: u32,
        block_type: i32,
    ) -> i32;
}

/// Host-build stand-ins for the `fprime_v1` imports; every one aborts.
#[cfg(not(target_family = "wasm"))]
#[allow(unused)]
mod host {
    fn off_target(function: &str) -> ! {
        panic!(
            "fprime_v1.{function} was called in a host build. A sequence runs on the \
             interpreter, not natively: load the module with `fprime_test` instead of \
             calling into the generated API directly."
        )
    }

    pub(crate) unsafe fn exit(_code: i32) -> ! {
        off_target("exit")
    }

    pub(crate) unsafe fn panic(_code: i32) -> ! {
        off_target("panic")
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn args(_destination_ptr: u32, _destination_size: u32) -> u32 {
        off_target("args")
    }

    pub(crate) unsafe fn time(_time_ptr: u32, _time_size: u32) {
        off_target("time")
    }

    pub(crate) unsafe fn tlm(
        _id: i64,
        _time_ptr: u32,
        _time_size: u32,
        _value_ptr: u32,
        _value_size: u32,
    ) -> i32 {
        off_target("tlm")
    }

    pub(crate) unsafe fn prm(_id: i64, _value_ptr: u32, _value_size: u32) -> i32 {
        off_target("prm")
    }

    pub(crate) unsafe fn cmd(_buf_ptr: u32, _buf_size: u32) -> i32 {
        off_target("cmd")
    }

    pub(crate) unsafe fn event(_severity: i32, _msg_ptr: u32, _msg_size: u32) {
        off_target("event")
    }

    pub(crate) unsafe fn rsleep(_us: u64) {
        off_target("rsleep")
    }

    pub(crate) unsafe fn asleep(_us: u64) {
        off_target("asleep")
    }

    pub(crate) unsafe fn serial_send(_index: i32, _data_ptr: u32, _data_size: u32) {
        off_target("serial_send")
    }

    pub(crate) unsafe fn serial_recv(
        _index: i32,
        _data_ptr: u32,
        _data_size: u32,
        _actual_size_ptr: u32,
        _block_type: i32,
    ) -> i32 {
        off_target("serial_recv")
    }
}

#[allow(unused)]
#[cfg(not(target_family = "wasm"))]
pub(crate) use host::*;

use crate::{EventSeverity, abi, message, messagef_impl};

#[derive(Copy, Clone, Debug)]
#[repr(i32)]
pub enum PanicCode {
    /// Code reserved for Rust global panic handler (abort)
    RustPanic = 0,
    /// A host function returned an invalid response
    InvalidStatus = 1,
    /// A command failed
    CmdFailed = 2,
    /// Telemetry value is invalid
    TlmInvalid = 3,
    /// Parameter value is not initialized
    PrmUninit = 4,
    /// Parameter value is not initialized
    PrmInvalid = 5,
    /// A serialized enum held a value with no corresponding variant
    InvalidEnum = 6,
    /// A serialized value did not fit its destination buffer
    Truncated = 7,
    /// Attempted to compare times with different bases
    TimeBaseIncomparable = 8,
    /// Attempted to compare times with different contexts
    TimeContextIncomparable = 9,
}

/// Exit the runtime due to a system/response failure
///
/// There are a fixed set of failure codes reserved. If the program needs to
/// exit with failure use [exit] with a non-zero status.
pub fn panic(code: PanicCode) -> ! {
    unsafe {
        abi::panic(code as i32);
    }
}

/// Exit the runtime given a status.
/// This function should not return and should stop the WASM runtime
///
/// # Arguments
///
/// * `code`: Exit code signaling status
///
/// returns: ! Never returns
pub fn exit(code: i32) -> ! {
    unsafe { abi::exit(code) }
}

pub fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    // Only emit error messages in debug mode
    // This keeps the release builds 600B smaller
    if cfg!(debug_assertions) {
        if let Some(msg) = info.message().as_str() {
            message(EventSeverity::WarningHi, msg);
        } else {
            message(EventSeverity::WarningHi, "rust panic")
        }

        if let Some(loc) = info.location() {
            messagef_impl(EventSeverity::ActivityLo, format_args!("{}", loc));
        }
    }

    panic(PanicCode::RustPanic);
}

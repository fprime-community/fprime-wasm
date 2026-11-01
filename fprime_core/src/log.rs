use crate::{String, abi};
use core::fmt::Arguments;

#[macro_export]
macro_rules! format {
    ($size:expr, $($arg:tt)+) => {
        fprime_core::heapless::string::format::<$size, u32>(format_args!($($arg)+)).unwrap()
    };
}

#[derive(Copy, Clone, Debug)]
#[repr(i32)]
pub enum EventSeverity {
    WarningHi = 2,
    WarningLow = 3,
    ActivityHigh = 5,
    ActivityLo = 6,
    Diagnostic = 7,
}

/// Emit a message via the F Prime event system
///
/// # Arguments
///
/// * `msg`: message string to emit via F Prime event
///
/// returns: ()
pub fn message(severity: EventSeverity, msg: &str) {
    let ptr = msg.as_ptr() as u32;
    let len = msg.len() as u32;
    unsafe { abi::event(severity as i32, ptr, len) }
}

#[macro_export]
macro_rules! messagef {
    ($sev:ident, $($arg:tt)+) => {
        fprime_core::messagef_impl(EventSeverity::$sev, format_args!($($arg)+));
    };
}

#[inline]
pub fn messagef_impl(severity: EventSeverity, args: Arguments<'_>) {
    let s: String<120> = heapless::string::format(args).unwrap();
    message(severity, &s)
}

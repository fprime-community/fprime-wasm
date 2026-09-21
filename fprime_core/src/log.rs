use crate::abi;
use core::fmt::Arguments;

/// Stack-backed String type
/// The capacity specifies the
pub type String<const N: usize> = heapless::String<N, u16>;

pub trait StrTruncate<const N: usize> {
    fn truncate(s: &str) -> Self;
}

impl<const N: usize> StrTruncate<N> for String<N> {
    fn truncate(s: &str) -> String<N> {
        let mut out: heapless::Vec<u8, N, u16> = heapless::Vec::new();
        let n = core::cmp::min(s.len(), N);
        unsafe {
            // Avoid `copy_nonoverlapping` since that induces a memcpy and without
            // bulk-memory operations in spacewasm it would cost a large amout of `.wasm` size.
            out.set_len(n);
            for (i, c) in s.get_unchecked(..n).as_bytes().iter().enumerate() {
                *out.get_unchecked_mut(i) = *c;
            }
            heapless::String::from_utf8_unchecked(out)
        }
    }
}

#[macro_export]
macro_rules! print_message {
    ($sev:expr, $($arg:tt)+) => {
        fprime_core::messagef($sev, format_args!($($arg)+));
    };
}

#[macro_export]
macro_rules! format {
    ($size:expr, $($arg:tt)+) => {
        fprime_core::heapless::string::format::<$size, u16>(format_args!($($arg)+)).unwrap()
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

/// Emit a message via the F Prime event system.
/// The message should be serialized into a UTF-8 buffer
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

#[inline]
pub fn messagef(severity: EventSeverity, args: Arguments<'_>) {
    let s: String<120> = heapless::string::format(args).unwrap();
    message(severity, &s)
}

//! Loading and running a sequence the way `Svc::WasmSequencer` would, one module at a time.
//!
//! [`Limits`] configures both. Loading decodes and instantiates a module and reports
//! [`Validated`], which is as far as `fprime-wasm verify` goes; running carries on into the
//! entry point and reports a [`Report`]. [`host`] is the `fprime_v1` module the guest calls
//! into.
//!
//! `spacewasm`'s allocator is process-global (`spacewasm::global_allocator!`), so two modules
//! decoding at once would corrupt it rather than fail cleanly. [`run`] and [`validate`] each
//! take a lock for their whole duration; libtest's threads queue here instead of running
//! concurrently.

mod alloc;
pub mod host;
mod limits;
mod link;
mod load;
mod report;
mod run;
mod stack;
mod stream;

use anyhow::Result;
use std::sync::{Mutex, MutexGuard};

pub use alloc::Usage;
pub use host::{Call, Issues, Kind, Recording, Script, Stop};
pub use limits::Limits;
pub use load::{Cost, Validated};
pub use report::{GuestStack, Outcome, Report};

spacewasm::global_allocator!(alloc::Tracker, alloc::Tracker::new());

/// The interpreter heap `spacewasm` allocates through.
///
/// # Safety
///
/// Sound because a load is single-threaded and never replaces the allocator while a
/// reference is live.
fn tracker() -> &'static alloc::Tracker {
    // SAFETY: single-threaded, never replaced while borrowed.
    unsafe { &*GLOBAL_ALLOCATOR }
}

/// Held for the duration of one load. The guarded value is `()`: what is protected is
/// process-global state inside `spacewasm`, not anything this crate owns.
static INTERPRETER: Mutex<()> = Mutex::new(());

/// Take the interpreter lock, recovering from a poisoned one — a panicking test must not
/// cascade a lock-poisoning error onto every test after it.
fn lock() -> MutexGuard<'static, ()> {
    INTERPRETER.lock().unwrap_or_else(|poisoned| {
        INTERPRETER.clear_poison();
        poisoned.into_inner()
    })
}

/// Decode and run one module, and report what it did.
///
/// `script` is the test's plan, consulted at every host call. `None` runs the sequence
/// against a nominal host.
pub fn run(
    bytes: Vec<u8>,
    limits: &Limits,
    script: Option<host::script::Shared>,
) -> Result<Report> {
    let _guard = lock();
    run::run(bytes, limits, script)
}

/// Decode and instantiate one module without running it, and report what that cost.
///
/// What `fprime-wasm verify` does: a module that gets this far is one the on-board
/// interpreter will load.
pub fn validate(bytes: Vec<u8>, limits: &Limits) -> Result<Validated> {
    let _guard = lock();
    load::validate(bytes, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_lock_is_recovered() {
        let poisoned = std::panic::catch_unwind(|| {
            let _guard = lock();
            panic!("a test failing its assertions while running a sequence");
        });
        assert!(poisoned.is_err(), "the panic should have escaped");
        assert!(
            INTERPRETER.is_poisoned(),
            "and it should have poisoned the lock"
        );

        // The whole point: this must not panic.
        let _guard = lock();
        assert!(
            !INTERPRETER.is_poisoned(),
            "taking the lock should have cleared the poison"
        );
    }
}

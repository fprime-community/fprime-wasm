//! Running a sequence the way `Svc::WasmSequencer` would, and measuring it.
//!
//! Every number here maps onto something a deployment configures.
//! `WasmSequencer::configure` takes a `WasmSequencer::Config` and a memory
//! allocator; too small and the sequence fails on board, too large and the memory is
//! wasted. So the harness stands up the same interpreter with the same limits and
//! reports what the module actually needed.
//!
//! * [`Limits`] — the configuration a module is measured against
//! * [`run`] — decode, instantiate, drive the interpreter, and [`Report`] the cost
//! * [`host`] — the `fprime_v1` module the guest calls into
//! * [`alloc`] — where the interpreter's memory comes from, and the figures read
//!   back off it
//! * [`stream`] — handing the module to the streaming decoder
//! * `link` — turning a decode failure into something a sequence author can act on
//! * `stack` — the guest stack high-water mark, measured by poisoning the region

pub mod alloc;
pub mod host;
mod limits;
mod link;
mod report;
mod run;
mod stack;
pub mod stream;

pub use alloc::Usage;
pub use host::{Call, Issues, Kind, Recording, Responses, Stop};
pub use limits::Limits;
pub use report::{GuestStack, Outcome, Report};
pub use run::run;

spacewasm::global_allocator!(alloc::Tracker, alloc::Tracker::new());

/// The interpreter heap `spacewasm` allocates through.
///
/// # Safety of the accessor
///
/// The macro above stores the allocator in `static mut` globals. Handing out a shared
/// reference is sound here because `verify` is single-threaded and never replaces the
/// allocator while a reference is live — [`alloc::Tracker::configure`] takes `&self`
/// and asserts nothing is allocated.
fn tracker() -> &'static alloc::Tracker {
    // SAFETY: single-threaded; see the note above.
    unsafe { &*GLOBAL_ALLOCATOR }
}

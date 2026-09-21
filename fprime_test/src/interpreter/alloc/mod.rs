//! Where the interpreter's memory comes from, and the numbers `verify` reads off it.
//!
//! Two separate budgets, and this module keeps them separate:
//!
//! * [`Tracker`] wraps the same `PageAllocator` `WasmSequencer` hands `spacewasm` for
//!   its own working memory, and records what passed through as a [`Usage`];
//! * [`GuestPool`] stands in for the bump pool guest linear memory is served from,
//!   bounded by `Config::guestMemorySize`.
//!
//! [`System`] backs both with the host allocator.
//!
//! # Single-threaded
//!
//! `spacewasm::global_allocator!` reaches [`Tracker`] through `static mut` globals
//! with no synchronisation, which is sound only on one thread. `verify` is
//! single-threaded and does not allocate from inside a host call, which is the
//! contract that macro documents.

mod guest;
mod system;
mod tracker;

pub use guest::{Guest, GuestPool};
pub use system::System;
pub use tracker::{Tracker, Usage};

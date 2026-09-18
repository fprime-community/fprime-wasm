//! The `fprime_v1` host module, standing in for `Svc::WasmSequencer`.
//!
//! The behaviour is a nominal dry run: commands succeed, telemetry and parameters
//! read as valid, sleeps return at once. That is the path a sequence is written for,
//! and the path whose resource use a deployment has to size.
//!
//! * `functions` — the twelve functions [`crate::abi::FUNCTIONS`] lists, each
//!   registered with the signature the C++ registers, so a module that links here
//!   links on board
//! * `traps` — the argument checks that mirror `WasmSequencerHost.cpp`: where the
//!   sequencer traps the guest, so does this
//! * `record` — the calls made, and divergences the guest cannot see (a forbidden
//!   severity, a truncated message), which are recorded rather than ignored
//! * `responses` — canned inputs, so a sequence that branches on telemetry can be
//!   driven down a chosen path
//! * `memory` — reading and writing guest memory without ever failing `verify` itself

mod functions;
mod memory;
mod record;
mod responses;
mod traps;

pub use functions::module;
pub use record::{Call, Issues, Kind, Recording, Stop};
pub use responses::Responses;

use std::cell::RefCell;
use std::rc::Rc;

/// Shared handle to the recording, held by every host closure.
pub type Shared = Rc<RefCell<Recording>>;

pub fn recording(responses: Responses) -> Shared {
    Rc::new(RefCell::new(Recording::new(responses)))
}

//! The `fprime_v1` host module: nominal by default — commands succeed, reads look valid,
//! sleeps return at once — and whatever the installed [`Script`] says instead.
//!
//! * `functions` — the twelve functions [`crate::abi::FUNCTIONS`] lists
//! * `traps` — argument checks mirroring `WasmSequencerHost.cpp`
//! * `record` — calls made and divergences the guest cannot see
//! * `script` — the answers a test supplies as the run proceeds
//! * `memory` — guest memory access

mod functions;
mod memory;
mod record;
pub mod script;
mod traps;

pub use functions::module;
pub use record::{Call, Issues, Kind, Recording, Stop};
pub use script::Script;

use super::Limits;
use std::cell::{RefCell, RefMut};
use std::rc::Rc;

/// Shared handle to the recording, held by every host closure.
pub type Shared = Rc<RefCell<Recording>>;

/// Recording and script live in separate cells: a handler can hold the recording borrowed
/// while consulting the script without a `BorrowMutError`.
#[derive(Clone)]
pub struct Ctx {
    recording: Shared,
    /// `None` when nothing is driving the run: every answer stays nominal.
    script: Option<script::Shared>,
}

impl Ctx {
    /// The recording, mutably.
    pub fn borrow_mut(&self) -> RefMut<'_, Recording> {
        self.recording.borrow_mut()
    }

    /// The recording, read-only.
    pub fn borrow(&self) -> std::cell::Ref<'_, Recording> {
        self.recording.borrow()
    }

    /// Ask the script, if one is installed.
    pub fn ask<T>(&self, question: impl FnOnce(&mut dyn Script) -> T) -> Option<T> {
        let script = self.script.as_ref()?;
        Some(question(&mut *script.borrow_mut()))
    }

    /// Unwrap the recording once the run is over.
    pub fn into_recording(self) -> Result<Recording, Self> {
        let Ctx { recording, script } = self;
        Rc::try_unwrap(recording)
            .map(RefCell::into_inner)
            .map_err(|recording| Ctx { recording, script })
    }
}

pub fn context(limits: &Limits, script: Option<script::Shared>) -> Ctx {
    Ctx {
        recording: Rc::new(RefCell::new(Recording::new(limits))),
        script,
    }
}

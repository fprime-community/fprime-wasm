//! The configuration a module is loaded and run under.

use crate::abi;

/// The limits a deployment sets: what `fprime-wasm verify` sizes a module against, and
/// what a sequence test runs it under.
///
/// Defaults are the on-board defaults: `WasmSequencer::Config`'s member initialisers
/// and `WasmSequencerSpacewasmConfig.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// `WASM_SEQ_SPACEWASM_PAGE_SIZE`. Bounds the largest single allocation.
    pub page_size: usize,
    /// `Config::heapPages`, in units of `page_size`.
    pub heap_pages: u32,
    /// `Config::guestMemorySize`, in bytes.
    pub guest_memory: u64,
    /// `Config::stackSize`, in 32-bit words.
    pub stack_size: usize,
    /// `Config::maxCodePages`.
    pub max_code_pages: usize,
    /// `Config::maxGuestModules`.
    pub max_guest_modules: u8,
    /// `Wasm.GUEST_EVENT_MESSAGE_SIZE`: the cap a guest event message is truncated to.
    pub event_message_max: usize,
    /// `Wasm.MAX_SERIAL_{IN,OUT}_PORTS`; an index outside it traps.
    pub serial_ports: i32,
    /// Upper bound on instructions executed, so a sequence that loops forever stops
    /// the tool instead of hanging it.
    pub max_instructions: u64,
    /// Instructions between operand-stack samples. One is exact; larger is faster but
    /// can miss a brief peak.
    pub stack_sample: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            page_size: 8192,
            heap_pages: 8,
            guest_memory: 8192,
            stack_size: 1024,
            max_code_pages: 256,
            max_guest_modules: 8,
            event_message_max: abi::EVENT_MESSAGE_MAX,
            serial_ports: abi::MAX_SERIAL_PORTS,
            max_instructions: 10_000_000,
            stack_sample: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_board_defaults() {
        let limits = Limits::default();
        // WasmSequencerSpacewasmConfig.h
        assert_eq!(limits.page_size, 8192);
        // WasmSequencer::Config member initialisers
        assert_eq!(limits.heap_pages, 8);
        assert_eq!(limits.guest_memory, 8192);
        assert_eq!(limits.stack_size, 1024);
        assert_eq!(limits.max_code_pages, 256);
        assert_eq!(limits.max_guest_modules, 8);
        // Build-time constants, from the ABI rather than the component's config.
        assert_eq!(limits.event_message_max, abi::EVENT_MESSAGE_MAX);
        assert_eq!(limits.serial_ports, abi::MAX_SERIAL_PORTS);
        // The binding cap is the event's own string width, not FW_LOG_STRING_MAX_SIZE.
        assert_eq!(abi::EVENT_MESSAGE_MAX, 128);
    }
}

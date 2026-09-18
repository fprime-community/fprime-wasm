//! The sequencer's own argument checks, each returning why it would trap.
//!
//! These are what `WasmSequencerHost.cpp` verifies before pausing the interpreter.
//! Without them `verify` passes a sequence that traps on board.

use crate::abi;

/// `time` and `tlm` require exactly `Fw::Time::SERIALIZED_SIZE`; too large traps as
/// readily as too small.
pub(super) fn bad_time_len(len: i32, function: &str) -> Option<String> {
    (len.unsigned_abs() != abi::TIME_SERIALIZED_SIZE).then(|| {
        format!(
            "{function} was given a {len}-byte time buffer; Fw::Time serialises to exactly {} \
             bytes and the sequencer traps on any other size",
            abi::TIME_SERIALIZED_SIZE
        )
    })
}

/// A channel or parameter id outside `FwIdType` would alias a valid one if
/// truncated, so the sequencer rejects it.
pub(super) fn bad_id(id: i64, what: &str) -> Option<String> {
    (!(0..=abi::MAX_ID).contains(&id)).then(|| {
        format!(
            "{what} id {id} does not fit FwIdType (0..={}); the sequencer traps the guest",
            abi::MAX_ID
        )
    })
}

/// A sleep whose seconds part would overflow the U32 fields of `Fw::Time`.
pub(super) fn sleep_too_long(us: u64, kind: &str) -> Option<String> {
    (us / 1_000_000 > u64::from(u32::MAX)).then(|| {
        format!("an {kind} sleep of {us} us exceeds what Fw::Time can hold; the sequencer traps the guest")
    })
}

/// A serial port index outside the configured range.
pub(super) fn bad_port(index: i32, ports: i32, function: &str) -> Option<String> {
    (index < 0 || index >= ports).then(|| {
        format!("{function} used port {index}, outside the {ports} configured; the sequencer traps the guest")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_buffer_must_be_exactly_the_serialised_size() {
        let exact = abi::TIME_SERIALIZED_SIZE as i32;
        assert!(bad_time_len(exact, "time").is_none());
        // Too large traps as readily as too small; that surprised me reading the C++.
        let over = bad_time_len(exact + 1, "time").expect("too large should be rejected");
        assert!(over.contains("exactly"), "{over}");
        assert!(bad_time_len(exact - 1, "time").is_some());
        assert!(bad_time_len(0, "time").is_some());
    }

    #[test]
    fn an_id_must_fit_fwidtype() {
        assert!(bad_id(0, "telemetry channel").is_none());
        assert!(bad_id(abi::MAX_ID, "telemetry channel").is_none());
        // Truncating either of these would alias a real channel.
        assert!(bad_id(-1, "telemetry channel").is_some());
        assert!(bad_id(abi::MAX_ID + 1, "telemetry channel").is_some());
    }

    #[test]
    fn a_sleep_must_fit_the_u32_seconds_field() {
        assert!(sleep_too_long(0, "relative").is_none());
        assert!(sleep_too_long(1_000_000, "relative").is_none());
        assert!(sleep_too_long(u64::from(u32::MAX) * 1_000_000, "relative").is_none());
        assert!(sleep_too_long(u64::MAX, "absolute").is_some());
        // The bound applies to `asleep` too, which this harness used to miss.
        let message = sleep_too_long(u64::MAX, "absolute").expect("rejected");
        assert!(message.contains("absolute"), "{message}");
    }

    #[test]
    fn a_serial_port_index_must_be_in_range() {
        let ports = abi::MAX_SERIAL_PORTS;
        assert!(bad_port(0, ports, "serial_send").is_none());
        assert!(bad_port(ports - 1, ports, "serial_send").is_none());
        assert!(bad_port(ports, ports, "serial_send").is_some());
        assert!(bad_port(-1, ports, "serial_recv").is_some());
    }
}

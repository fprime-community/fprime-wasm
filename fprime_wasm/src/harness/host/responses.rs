//! Canned inputs, so a sequence can be driven down a chosen path.

use crate::abi;
use std::collections::BTreeMap;

/// What the host answers with, from the command line, so a sequence that branches
/// on telemetry or waits on a serial port need not always see zeroes.
#[derive(Debug, Clone, Default)]
pub struct Responses {
    /// Sequence arguments, as `fprime_v1.args` would deliver them.
    pub args: Vec<u8>,
    /// Telemetry channel id to serialised value.
    pub telemetry: BTreeMap<i64, Vec<u8>>,
    /// Parameter id to serialised value.
    pub parameters: BTreeMap<i64, Vec<u8>>,
    /// Serial port index to the message a `serial_recv` receives. Each port delivers
    /// its message once; later reads see an empty queue.
    pub serial: BTreeMap<i32, Vec<u8>>,
    /// Effective cap on a guest event message for the target deployment.
    pub event_message_max: usize,
    /// `Wasm.MAX_SERIAL_{IN,OUT}_PORTS`; an index outside it traps.
    pub serial_ports: i32,
}

impl Responses {
    /// Nothing supplied, and the two caps at their on-board values — so a `verify`
    /// with no flags answers the question for a stock sequencer.
    pub fn new() -> Self {
        Responses {
            event_message_max: abi::EVENT_MESSAGE_MAX,
            serial_ports: abi::MAX_SERIAL_PORTS,
            ..Responses::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_default_to_the_on_board_configuration() {
        let responses = Responses::new();
        assert_eq!(responses.event_message_max, abi::EVENT_MESSAGE_MAX);
        assert_eq!(responses.serial_ports, abi::MAX_SERIAL_PORTS);
        // The binding cap is the event's own string width, not FW_LOG_STRING_MAX_SIZE.
        assert_eq!(abi::EVENT_MESSAGE_MAX, 128);
    }
}

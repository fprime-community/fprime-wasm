use std::cell::RefCell;
use std::rc::Rc;

pub trait Script {
    /// The `Fw::CmdResponse` for a command; `None` leaves the nominal OK. `payload` excludes
    /// the opcode.
    fn command(&mut self, opcode: u32, payload: &[u8]) -> Option<i32> {
        let _ = (opcode, payload);
        None
    }

    /// A telemetry channel's value, asked once per read; `None` reads all zeroes.
    fn telemetry(&mut self, id: i64) -> Option<Vec<u8>> {
        let _ = id;
        None
    }

    /// The same, for a parameter.
    fn parameter(&mut self, id: i64) -> Option<Vec<u8>> {
        let _ = id;
        None
    }

    /// The guest emitted an event. Observation only: the host cannot refuse one.
    fn event(&mut self, severity: i32, message: &str) {
        let _ = (severity, message);
    }

    /// The guest asked to sleep (`absolute` distinguishes `asleep` from `rsleep`). Nothing
    /// actually waits.
    fn sleep(&mut self, us: u64, absolute: bool) {
        let _ = (us, absolute);
    }

    /// A queued message for `serial_recv`. `blocking`: `Queue::block_recv` vs `Queue::recv`.
    fn serial_recv(&mut self, port: i32, blocking: bool) -> Option<Vec<u8>> {
        let _ = (port, blocking);
        None
    }

    /// The guest sent on a serial output port. Observation only.
    fn serial_send(&mut self, port: i32, data: &[u8]) {
        let _ = (port, data);
    }

    /// The sequence's invocation arguments; `None` means it was invoked with none.
    fn args(&mut self) -> Option<Vec<u8>> {
        None
    }

    /// Whether the run should stop now, ending it as
    /// [`Outcome::Suspended`](super::super::Outcome).
    /// For a sequence that never blocks on its own.
    fn should_stop(&mut self) -> bool {
        false
    }

    /// Microseconds since the F Prime epoch, for `time`. `None` reports all zeroes.
    fn now_us(&mut self) -> Option<u64> {
        None
    }
}

/// A script, shared between the host closures and whoever installed it.
pub type Shared = Rc<RefCell<dyn Script>>;

/// Wrap a script for installation.
pub fn shared<S: Script + 'static>(script: S) -> Shared {
    Rc::new(RefCell::new(script))
}

/// `Fw::TimeValue`, big-endian: `timeBase: u16`, `timeContext: u8`, `seconds: u32`,
/// `useconds: u32`. `timeBase` stays constant — `PartialOrd for TimeValue` panics the guest
/// if two readings differ.
pub(super) fn serialize_time(us: u64) -> [u8; crate::abi::TIME_SERIALIZED_SIZE as usize] {
    let seconds = (us / 1_000_000) as u32;
    let useconds = (us % 1_000_000) as u32;

    let mut out = [0u8; crate::abi::TIME_SERIALIZED_SIZE as usize];
    // out[0..2] is timeBase, out[2] is timeContext: both left zero.
    out[3..7].copy_from_slice(&seconds.to_be_bytes());
    out[7..11].copy_from_slice(&useconds.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct OnlyCommands {
        seen: Vec<u32>,
    }

    impl Script for OnlyCommands {
        fn command(&mut self, opcode: u32, _payload: &[u8]) -> Option<i32> {
            self.seen.push(opcode);
            Some(4)
        }
    }

    #[test]
    fn default_is_decline() {
        let mut script = OnlyCommands::default();
        assert_eq!(script.telemetry(9), None);
        assert_eq!(script.parameter(9), None);
        assert_eq!(script.serial_recv(0, false), None);
        assert_eq!(script.now_us(), None);
        // These two return nothing but must still be callable without an override.
        script.event(5, "hello");
        script.sleep(1_000, false);
    }

    #[test]
    fn override_is_observed() {
        let mut script = OnlyCommands::default();
        assert_eq!(script.command(0x1000000, &[]), Some(4));
        assert_eq!(script.command(0x1000001, &[1, 2]), Some(4));
        assert_eq!(script.seen, vec![0x1000000, 0x1000001]);
    }

    #[test]
    fn clock_serialises_as_fw_timevalue() {
        assert_eq!(
            serialize_time(3_500_000),
            [
                0x00, 0x00, // timeBase: TB_NONE
                0x00, // timeContext
                0x00, 0x00, 0x00, 0x03, // seconds
                0x00, 0x07, 0xa1, 0x20, // useconds = 500_000
            ]
        );
        assert_eq!(serialize_time(0), [0u8; 11]);
        // Sub-second only, so the split is not just an integer division that happens to work.
        assert_eq!(&serialize_time(999_999)[3..7], &[0, 0, 0, 0]);
        assert_eq!(&serialize_time(999_999)[7..11], &999_999u32.to_be_bytes());
    }

    #[test]
    fn time_base_is_constant() {
        for us in [0, 1, 1_000_000, u64::from(u32::MAX) * 1_000_000] {
            assert_eq!(&serialize_time(us)[..3], &[0, 0, 0], "at {us} us");
        }
    }

    #[test]
    fn shared_script_reachable_from_both_sides() {
        let script = shared(OnlyCommands::default());
        let held = Rc::clone(&script);
        assert_eq!(held.borrow_mut().command(7, &[]), Some(4));
        // The installer's handle sees what the host did.
        assert_eq!(
            script
                .borrow_mut()
                .command(8, &[])
                .expect("the script answers"),
            4
        );
    }
}

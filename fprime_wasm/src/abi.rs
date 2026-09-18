//! The canonical `fprime_v1` host interface.
//!
//! The contract between `fprime_core`'s guest-side `extern` block and
//! `Svc::WasmSequencer::hostFprimeV1`.

/// Host module name. `WasmSequencer` registers exactly one, so an import from any
/// other module cannot resolve on board.
pub const MODULE: &str = "fprime_v1";

/// What the sequencer invokes. `#[fprime_main]` exports the entry point under this
/// name and `WasmSequencerController` looks it up by it.
pub const ENTRY_POINT: &str = "main";

/// One host function's name and signature.
pub struct Function {
    pub name: &'static str,
    pub params: &'static str,
    pub returns: &'static str,
}

/// Every function `WasmSequencer` registers
#[rustfmt::skip]
pub const FUNCTIONS: &[Function] = &[
    Function { name: "exit", params: "i", returns: "" },
    Function { name: "panic", params: "i", returns: "" },
    Function { name: "args", params: "ii", returns: "i" },
    Function { name: "time", params: "ii", returns: "" },
    Function { name: "tlm", params: "Iiiii", returns: "i" },
    Function { name: "prm", params: "Iii", returns: "i" },
    Function { name: "cmd", params: "ii", returns: "i" },
    Function { name: "event", params: "iii", returns: "" },
    Function { name: "rsleep", params: "I", returns: "" },
    Function { name: "asleep", params: "I", returns: "" },
    Function { name: "serial_send", params: "iii", returns: "" },
    Function { name: "serial_recv", params: "iiiii", returns: "i" },
];

pub fn function(name: &str) -> Option<&'static Function> {
    FUNCTIONS.iter().find(|f| f.name == name)
}

/// `Fw::CmdResponse::OK`. `FailMode::Verified` panics the guest on anything else, so
/// a nominal dry run has to return this.
pub const CMD_RESPONSE_OK: i32 = 0;

/// `Fw::TlmValid::VALID` — zero, where `ParamValid`'s valid is one. `fprime_core::tlm`
/// panics on any non-zero status.
pub const TLM_VALID: i32 = 0;

/// `Fw::ParamValid::VALID`. `fprime_core::prm` accepts this and `DEFAULT` (3), and
/// panics on `UNINIT` (0) and `INVALID` (2).
pub const PARAM_VALID: i32 = 1;

/// `FprimeQueueStatus::OK`.
pub const QUEUE_OK: i32 = 0;

/// `FprimeQueueStatus::EMPTY`: what a non-blocking `serial_recv` gets from an empty
/// queue. `fprime_core::Queue::recv` maps it to `None`.
pub const QUEUE_EMPTY: i32 = 1;

/// `FprimeBlockingType::BLOCKING`, the `block_type` argument to `serial_recv`.
pub const BLOCKING: i32 = 0;

/// Width of the `FwOpcodeType` prefix on an encoded command, big-endian as F Prime
/// serialises everything.
pub const OPCODE_BYTES: usize = 4;

/// Effective cap on a guest event message. Two limits apply on board and the smaller
/// binds: `wasmEvent` clamps to `FW_LOG_STRING_MAX_SIZE` (200), then the event's own
/// `string size Wasm.GUEST_EVENT_MESSAGE_SIZE` (128) truncates again. Either way the
/// guest is not told.
pub const EVENT_MESSAGE_MAX: usize = 128;

/// `Fw::Time::SERIALIZED_SIZE`: `FwTimeBaseStoreType` (U16) + `FwTimeContextStoreType`
/// (U8) + seconds + useconds. `time` and `tlm` trap on any other length, not just a
/// shorter one.
pub const TIME_SERIALIZED_SIZE: u32 = 2 + 1 + 4 + 4;

/// Largest payload `cmd` accepts: `FW_COM_BUFFER_MAX_SIZE` (512) less the
/// `FwPacketDescriptorType` (U32) prefix.
pub const CMD_MAX_PAYLOAD: u32 = 512 - 4;

/// `FwChanIdType` and `FwPrmIdType` are both `FwIdType` = U32. An id outside this
/// range would alias a valid one if truncated, so the sequencer traps.
pub const MAX_ID: i64 = u32::MAX as i64;

/// `Wasm.MAX_SERIAL_IN_PORTS` / `MAX_SERIAL_OUT_PORTS`. An index outside it traps.
pub const MAX_SERIAL_PORTS: i32 = 5;

/// `WasmSequencerConfig::MAX_BACKPATCH_ITERATIONS`. Bounds control-flow backpatch
/// resolution; a module past it fails to load with `ERR_POSSIBLE_BACKPATCH_CYCLE`.
pub const MAX_BACKPATCH_ITERATIONS: u32 = 32768;

/// How `WasmSequencer` treats a severity the guest asked `event` for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Dispatched as an event at this severity.
    Allowed(&'static str),
    /// Recognised, but not for a guest: `FATAL` would let untrusted code reach the
    /// `FatalHandler`, and `COMMAND` belongs to the command dispatcher.
    Forbidden(&'static str),
    /// Not a `Fw::LogSeverity` value at all.
    Unknown,
}

impl Severity {
    /// True when the sequencer would emit `HostFunctionInvalidSeverity` instead of the
    /// event asked for. The guest is not trapped, so this is silent from inside the
    /// sequence.
    pub fn is_rejected(self) -> bool {
        !matches!(self, Severity::Allowed(_))
    }

    pub fn name(self) -> &'static str {
        match self {
            Severity::Allowed(name) | Severity::Forbidden(name) => name,
            Severity::Unknown => "?",
        }
    }
}

/// Classify a raw `severity`. `Fw::LogSeverity` is one-based, `FATAL = 1` through
/// `DIAGNOSTIC = 7`; zero is not a value.
pub fn severity(raw: i32) -> Severity {
    match raw {
        1 => Severity::Forbidden("FATAL"),
        2 => Severity::Allowed("WARNING_HI"),
        3 => Severity::Allowed("WARNING_LO"),
        4 => Severity::Forbidden("COMMAND"),
        5 => Severity::Allowed("ACTIVITY_HI"),
        6 => Severity::Allowed("ACTIVITY_LO"),
        7 => Severity::Allowed("DIAGNOSTIC"),
        _ => Severity::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_function_has_a_well_formed_signature() {
        for f in FUNCTIONS {
            for c in f.params.chars().chain(f.returns.chars()) {
                assert!(
                    matches!(c, 'i' | 'I' | 'f' | 'd'),
                    "{}: {c:?} is not a spacewasm value type",
                    f.name
                );
            }
            assert!(
                f.returns.len() <= 1,
                "{}: spacewasm host functions return at most one value",
                f.name
            );
        }
    }

    #[test]
    fn function_names_are_unique() {
        let mut names: Vec<_> = FUNCTIONS.iter().map(|f| f.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate host function name");
    }

    #[test]
    fn looks_up_functions_by_name() {
        assert_eq!(function("tlm").expect("tlm is registered").params, "Iiiii");
        assert!(function("no_such_host_function").is_none());
    }

    /// One-based, and the guest's `EventSeverity` matches. Off by one here mislabels
    /// every event.
    #[test]
    fn classifies_severities_one_based() {
        assert_eq!(severity(2), Severity::Allowed("WARNING_HI"));
        assert_eq!(severity(5), Severity::Allowed("ACTIVITY_HI"));
        assert_eq!(severity(7), Severity::Allowed("DIAGNOSTIC"));
    }

    /// These get `HostFunctionInvalidSeverity` on board, not the event the guest
    /// wrote, so they must not read as allowed.
    #[test]
    fn rejects_forbidden_and_out_of_range_severities() {
        assert_eq!(severity(1), Severity::Forbidden("FATAL"));
        assert_eq!(severity(4), Severity::Forbidden("COMMAND"));
        assert_eq!(severity(0), Severity::Unknown);
        assert_eq!(severity(8), Severity::Unknown);
        assert_eq!(severity(-1), Severity::Unknown);

        for raw in [1, 4, 0, 8, -1] {
            assert!(severity(raw).is_rejected(), "severity({raw})");
        }
        for raw in [2, 3, 5, 6, 7] {
            assert!(!severity(raw).is_rejected(), "severity({raw})");
        }
    }

    /// Every severity `fprime_core::EventSeverity` can produce must be one the
    /// sequencer dispatches, or the guest API offers a level that silently fails.
    #[test]
    fn every_guest_severity_is_allowed() {
        // fprime_core::EventSeverity: WarningHi, WarningLow, ActivityHigh,
        // ActivityLo, Diagnostic.
        for raw in [2, 3, 5, 6, 7] {
            assert!(
                matches!(severity(raw), Severity::Allowed(_)),
                "fprime_core can emit severity {raw}, but the sequencer rejects it"
            );
        }
    }
}

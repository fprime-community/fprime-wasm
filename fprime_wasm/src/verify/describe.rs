//! Turning recorded values into words: outcomes, host calls, dictionary names.

use crate::abi;
use crate::harness::{self, Outcome};
use fprime_dictionary::Dictionary;

pub(crate) fn describe_outcome(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Exited(0) => "sequence exited successfully 0".into(),
        Outcome::Exited(code) => format!("the sequence exited with status {code}"),
        Outcome::Returned => "the sequence returned from `main`".into(),
        Outcome::Panicked(code) => format!("panic code = {code}"),
        Outcome::Trapped(reason) => format!("the interpreter trapped: {reason}"),
        Outcome::Suspended => {
            "the sequence suspended waiting on a host call that never completed".into()
        }
        Outcome::OutOfInstructions => {
            "the sequence was still running at the instruction limit; raise --max-instructions"
                .into()
        }
    }
}

/// One word, for a summary cell.
pub(super) fn outcome_word(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Exited(0) => "exited",
        Outcome::Exited(_) => "exit code",
        Outcome::Returned => "ok",
        Outcome::Panicked(_) => "panicked",
        Outcome::Trapped(_) => "trapped",
        Outcome::Suspended => "suspended",
        Outcome::OutOfInstructions => "no fuel",
    }
}

/// One host call, as a line of trace.
pub(crate) fn describe_call(call: &harness::Call, dictionary: Option<&Dictionary>) -> String {
    use harness::Call;
    match call {
        Call::Exit { code } => format!("exit({code})"),
        Call::Panic { code } => format!("panic({code})"),
        Call::Args { capacity, written } => {
            format!("args -> {written} of {capacity} bytes")
        }
        Call::Time { len } => format!("time -> {len} bytes"),
        Call::Telemetry {
            id,
            value_len,
            status,
        } => format!(
            "tlm({id}{}) -> status {status}, {value_len} bytes",
            named(channel_name(dictionary, *id))
        ),
        Call::Parameter {
            id,
            value_len,
            status,
        } => format!(
            "prm({id}{}) -> status {status}, {value_len} bytes",
            named(parameter_name(dictionary, *id))
        ),
        Call::Command {
            opcode,
            payload,
            response,
        } => format!(
            "cmd({opcode:#010x}{}) -> response {response}, {} byte payload{}",
            named(command_name(dictionary, *opcode)),
            payload.len(),
            if payload.is_empty() {
                String::new()
            } else {
                format!(" [{}]", hex(payload))
            }
        ),
        Call::Event {
            severity,
            message,
            truncated,
        } => format!(
            "event({}) {message:?}{}",
            abi::severity(*severity).name(),
            if *truncated { " (truncated)" } else { "" }
        ),
        Call::RelativeSleep { us } => format!("rsleep({us} us)"),
        Call::AbsoluteSleep { us } => format!("asleep({us} us)"),
        Call::SerialSend { index, len } => format!("serial_send(port {index}, {len} bytes)"),
        Call::SerialRecv {
            index,
            blocking,
            received,
            status,
        } => format!(
            "serial_recv(port {index}, {}) -> status {status}, {received} bytes",
            if *blocking {
                "blocking"
            } else {
                "non-blocking"
            }
        ),
    }
}

fn named(name: Option<String>) -> String {
    name.map(|name| format!(" {name}")).unwrap_or_default()
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Dictionary opcodes are `u64`; a serialised `FwOpcodeType` is the low 32 bits.
pub(crate) fn command_name(dictionary: Option<&Dictionary>, opcode: u32) -> Option<String> {
    dictionary?
        .commands
        .iter()
        .find(|command| command.opcode == u64::from(opcode))
        .map(|command| command.name.clone())
}

pub(crate) fn channel_name(dictionary: Option<&Dictionary>, id: i64) -> Option<String> {
    let id = u64::try_from(id).ok()?;
    dictionary?
        .telemetry_channels
        .iter()
        .find(|channel| channel.id == id)
        .map(|channel| channel.name.clone())
}

pub(crate) fn parameter_name(dictionary: Option<&Dictionary>, id: i64) -> Option<String> {
    let id = u64::try_from(id).ok()?;
    dictionary?
        .parameters
        .iter()
        .find(|parameter| parameter.id == id)
        .map(|parameter| parameter.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sequence that returns from `main` without calling `exit` is fine;
    /// `#[fprime_main]` does not require one.
    #[test]
    fn describes_each_outcome_distinctly() {
        let described: Vec<String> = [
            Outcome::Exited(0),
            Outcome::Exited(3),
            Outcome::Returned,
            Outcome::Panicked(7),
            Outcome::Trapped("MemoryOutOfBounds".into()),
            Outcome::Suspended,
            Outcome::OutOfInstructions,
        ]
        .iter()
        .map(describe_outcome)
        .collect();

        let mut unique = described.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), described.len(), "descriptions collide");
        assert!(described[3].contains('7'), "{}", described[3]);
        assert!(
            described[4].contains("MemoryOutOfBounds"),
            "{}",
            described[4]
        );
    }

    /// The summary cell has room for one word, and two outcomes that mean
    /// different things must not share it.
    #[test]
    fn every_outcome_has_a_distinct_summary_word() {
        let outcomes = [
            Outcome::Exited(0),
            Outcome::Exited(3),
            Outcome::Returned,
            Outcome::Panicked(1),
            Outcome::Trapped("x".into()),
            Outcome::Suspended,
            Outcome::OutOfInstructions,
        ];
        let mut words: Vec<&str> = outcomes.iter().map(outcome_word).collect();
        let count = words.len();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), count, "two outcomes render as the same word");
    }

    #[test]
    fn formats_a_payload_as_hex() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "00 0f ff");
        assert_eq!(hex(&[]), "");
    }

    /// Without a dictionary the trace still has to be readable; names are an
    /// enrichment, not a requirement.
    #[test]
    fn describes_calls_without_a_dictionary() {
        use harness::Call;
        let lines: Vec<String> = [
            Call::Command {
                opcode: 0x10007002,
                payload: vec![0xab],
                response: 0,
            },
            Call::Event {
                severity: 5,
                message: "hello".into(),
                truncated: false,
            },
            Call::Event {
                severity: 1,
                message: "nope".into(),
                truncated: true,
            },
            Call::SerialRecv {
                index: 2,
                blocking: true,
                received: 0,
                status: 1,
            },
            Call::Telemetry {
                id: 12,
                value_len: 4,
                status: 0,
            },
        ]
        .iter()
        .map(|call| describe_call(call, None))
        .collect();

        assert_eq!(
            lines[0],
            "cmd(0x10007002) -> response 0, 1 byte payload [ab]"
        );
        assert_eq!(lines[1], "event(ACTIVITY_HI) \"hello\"");
        // A forbidden severity must still be named, so the note above it makes sense.
        assert_eq!(lines[2], "event(FATAL) \"nope\" (truncated)");
        assert_eq!(
            lines[3],
            "serial_recv(port 2, blocking) -> status 1, 0 bytes"
        );
        assert_eq!(lines[4], "tlm(12) -> status 0, 4 bytes");
    }

    #[test]
    fn a_missing_dictionary_yields_no_names() {
        assert_eq!(command_name(None, 0x10007002), None);
        assert_eq!(channel_name(None, 4), None);
        assert_eq!(parameter_name(None, 4), None);
    }

    /// Ids come off the ABI as `i64` and the dictionary stores them as `u64`; a
    /// negative id is not a lookup, and must not wrap into a valid one.
    #[test]
    fn a_negative_id_is_not_looked_up() {
        assert_eq!(channel_name(None, -1), None);
        assert_eq!(parameter_name(None, -1), None);
    }

    #[test]
    fn names_an_absent_value_as_nothing() {
        assert_eq!(named(None), "");
        assert_eq!(named(Some("CMD_NO_OP".into())), " CMD_NO_OP");
    }
}

//! Running a module: what a sequence test does.
//!
//! Every test goes through [`fprime_test::interpreter`], which holds the lock that keeps two
//! modules out of `spacewasm`'s single-threaded allocator at once — so libtest's threads
//! queue here rather than running concurrently.

mod fixture;

use fprime_test::interpreter::{Kind, Limits, Outcome};

#[test]
fn nominal_module_returns() {
    let bytes = fixture::module("nominal");
    let declared = fixture::declared_memory(&bytes);
    let report = fixture::run("nominal");

    assert_eq!(report.outcome, Outcome::Returned);
    assert!(report.outcome.is_nominal());
    assert_eq!(report.guest_memory, declared);
    assert_eq!(report.declared_memory, declared);
}

/// How close the sequence came to overflowing `-zstack-size`.
///
/// `reader` is the fixture that spills to the guest stack: it buffers the channel's time and
/// value there. The exact high-water mark is the optimiser's to choose, so only the reserved
/// size — pinned by `-zstack-size=512` in `.cargo/config.toml` — is asserted exactly.
#[test]
fn guest_stack_high_water_mark() {
    let report = fixture::run("reader");

    let stack = report
        .guest_stack
        .expect("a module with a stack pointer has a stack region");
    assert_eq!(stack.reserved, 512, "the region is [0, __stack_pointer)");
    assert!(stack.used > 0, "the sequence spills to linear memory");
    assert!(stack.used <= stack.reserved, "{stack:?}");
    assert_eq!(stack.headroom(), stack.reserved - stack.used);
}

/// `rust-lld` only emits `__stack_pointer` when a function needs one. `nominal` needs
/// nothing, so there is no guest stack to report — which is not the same as zero headroom.
#[test]
fn no_stack_pointer_omits_guest_stack() {
    let report = fixture::run("nominal");
    assert!(
        report.guest_stack.is_none(),
        "no stack pointer means no guest stack to report: {:?}",
        report.guest_stack
    );
}

/// Opcode is a big-endian `FwOpcodeType` prefix, stripped from the recorded payload.
#[test]
fn dispatched_command_records_opcode() {
    let report = fixture::run("dispatch");

    // The same descriptor the sequence encoded through, so the test states the wire form
    // once rather than twice.
    const LEN: usize = interp::Konst::Ref::wasmSeq::LOAD__size("helloworld");
    const BUFFER: [u8; LEN] = interp::Konst::Ref::wasmSeq::LOAD__encode::<LEN>("helloworld");
    let expected = interp::Desc::Ref::wasmSeq::LOAD.with(&BUFFER);

    let commands: Vec<(u32, Vec<u8>)> = report
        .recording
        .commands()
        .map(|(opcode, payload)| (opcode, payload.to_vec()))
        .collect();
    assert_eq!(
        commands,
        vec![(expected.opcode(), expected.args().to_vec())],
        "the opcode must be read big-endian and stripped from the payload"
    );
    assert_eq!(report.outcome, Outcome::Returned);
}

#[test]
fn trap_reported() {
    let report = fixture::run("trapping");
    assert!(
        matches!(report.outcome, Outcome::Trapped(_)),
        "expected a trap, got {:?}",
        report.outcome
    );
}

#[test]
fn endless_sequence_stops_at_instruction_limit() {
    let limits = Limits {
        max_instructions: 5_000,
        ..Limits::default()
    };
    let report = fixture::run_with("forever", &limits);

    assert_eq!(report.outcome, Outcome::OutOfInstructions);
    assert!(report.instructions >= 5_000);
    assert!(!report.outcome.is_nominal());
}

/// Peak operand stack is only knowable by running, which is why `verify` does not report it.
#[test]
fn operand_stack_peak_measured_by_running() {
    let report = fixture::run("dispatch");
    assert!(report.peak_operand_stack > 0, "{report:?}");
}

/// A telemetry read with nothing supplied reads zero, and says so.
#[test]
fn unset_channel_reads_zero_and_informs() {
    let report = fixture::run("reader");
    let channel = interp::Desc::CdhCore::events::EventsDropped;

    assert_eq!(
        report.recording.telemetry_read(),
        vec![i64::from(channel.id)]
    );
    let notes: Vec<&str> = report
        .recording
        .issues_of(Kind::Info)
        .map(|issue| issue.message.as_str())
        .collect();
    assert!(
        notes.iter().any(|note| note.contains("read as all zeroes")),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|note| note.contains("initial_telemetry")),
        "the note must name what to do about it: {notes:?}"
    );
}

/// `memory.grow` is allowed; a module using it must still load and run.
#[test]
fn growth_counts_toward_peak_memory() {
    let bytes = fixture::module("growing");
    let declared = fixture::declared_memory(&bytes);
    let report = fixture::run("growing");

    assert_eq!(report.outcome, Outcome::Returned);
    assert_eq!(report.declared_memory, declared);
    // One-byte pages, so the grow is a byte count.
    assert_eq!(
        report.guest_memory,
        declared + interp::GROW_PAGES as u64,
        "the peak must see the grown size"
    );
    assert_eq!(report.refused_grows, 0);
}

/// A grow past the pool is refused, not trapped.
#[test]
fn growth_past_pool_refused_without_trap() {
    let bytes = fixture::module("hungry");
    let declared = fixture::declared_memory(&bytes);
    let report = fixture::run("hungry");

    assert_eq!(
        report.outcome,
        Outcome::Returned,
        "a refused grow must not trap the guest"
    );
    assert_eq!(report.refused_grows, 1);
    assert_eq!(
        report.guest_memory, declared,
        "the refusal held the peak at the declared size"
    );
}

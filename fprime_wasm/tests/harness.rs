//! End-to-end tests for the `verify` harness.
//!
//! Everything here shares one process-global, single-threaded `spacewasm`
//! allocator, so it is deliberately a *single* `#[test]`. Splitting it up would
//! let the test harness run two interpreters concurrently through that allocator,
//! which is unsound — the failure would be a crash or a wrong measurement, not a
//! clean assertion. Each step is a named function so a failure still says which
//! behaviour broke.

use fprime_wasm::abi;
use fprime_wasm::harness::{Limits, Outcome, Responses};
use fprime_wasm::verify;
use fprime_wasm::wasm;
use std::path::PathBuf;

/// A hand-assembled sequence: imports `fprime_v1.cmd`, declares 941 bytes of
/// one-byte-page memory with a 512-byte stack region, and exports `main`.
///
/// Built by hand rather than compiled so the tests do not need a Wasm toolchain,
/// and so the exact shape under test — the custom page size, the stack region, the
/// import — is visible here rather than an artefact of a build.
mod fixture {
    /// LEB128-encode an unsigned value.
    fn uleb(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return out;
            }
            out.push(byte | 0x80);
        }
    }

    /// LEB128-encode a signed value, as `i32.const` operands are encoded.
    fn sleb(mut value: i64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            let sign = byte & 0x40 != 0;
            if (value == 0 && !sign) || (value == -1 && sign) {
                out.push(byte);
                return out;
            }
            out.push(byte | 0x80);
        }
    }

    fn section(id: u8, payload: Vec<u8>) -> Vec<u8> {
        let mut out = vec![id];
        out.extend(uleb(payload.len() as u64));
        out.extend(payload);
        out
    }

    /// Prefix a vector's element count.
    fn vector(count: usize, body: Vec<u8>) -> Vec<u8> {
        let mut out = uleb(count as u64);
        out.extend(body);
        out
    }

    fn name(text: &str) -> Vec<u8> {
        let mut out = uleb(text.len() as u64);
        out.extend(text.as_bytes());
        out
    }

    /// How `main` should behave.
    pub enum Body {
        /// Return immediately.
        Empty,
        /// Dispatch one command with this opcode and payload, then return.
        Command { opcode: u32, payload: Vec<u8> },
        /// Store to an address well past the end of linear memory.
        OutOfBounds,
        /// `loop br 0`, i.e. never terminate.
        Forever,
        /// Read a telemetry channel, then return. Nothing supplies a value, so the
        /// host zero-fills it and records that as an informational note.
        ReadTelemetry { id: i64 },
        /// `memory.grow` by `pages`, keeping the result. `spacewasm` writes -1 when
        /// the pool refuses, and does not trap.
        Grow { pages: i64 },
    }

    /// Assemble a module. `memory_bytes` is the declared linear memory with
    /// one-byte pages; `stack_pointer` adds a mutable `i32` global initialised to
    /// that value, which is what `verify` reads as the guest stack region.
    pub fn module(body: &Body, memory_bytes: u64, stack_pointer: Option<i32>) -> Vec<u8> {
        let mut out = b"\0asm\x01\x00\x00\x00".to_vec();

        // Types: 0 = () -> (), 1 = (i32, i32) -> i32 (`cmd`),
        // 2 = (i64, i32, i32, i32, i32) -> i32 (`tlm`).
        out.extend(section(
            1,
            vector(
                3,
                [
                    vec![0x60, 0x00, 0x00],
                    vec![0x60, 0x02, 0x7F, 0x7F, 0x01, 0x7F],
                    vec![0x60, 0x05, 0x7E, 0x7F, 0x7F, 0x7F, 0x7F, 0x01, 0x7F],
                ]
                .concat(),
            ),
        ));

        // Import only the host function this body calls, so a body that needs none
        // produces a module with no imports at all and exercises that path too.
        let import_of = match body {
            Body::Command { .. } => Some(("cmd", 1u64)),
            Body::ReadTelemetry { .. } => Some(("tlm", 2u64)),
            Body::Empty | Body::OutOfBounds | Body::Forever | Body::Grow { .. } => None,
        };
        if let Some((function, type_index)) = import_of {
            let mut import = name("fprime_v1");
            import.extend(name(function));
            import.push(0x00); // func
            import.extend(uleb(type_index));
            out.extend(section(2, vector(1, import)));
        }

        // One local function, `main`, of type 0.
        out.extend(section(3, vector(1, vec![0x00])));

        // Memory with the custom-page-sizes bit set and a page size of 2^0.
        let mut memory = vec![0x08];
        memory.extend(uleb(memory_bytes));
        memory.extend(uleb(0));
        out.extend(section(5, vector(1, memory)));

        if let Some(initial) = stack_pointer {
            let mut global = vec![0x7F, 0x01]; // i32, mutable
            global.push(0x41); // i32.const
            global.extend(sleb(i64::from(initial)));
            global.push(0x0B); // end
            out.extend(section(6, vector(1, global)));
        }

        // Export `main`. Its function index follows any imported functions.
        let main_index = u64::from(import_of.is_some());
        let mut export = name("main");
        export.push(0x00);
        export.extend(uleb(main_index));
        out.extend(section(7, vector(1, export)));

        let instructions = match body {
            Body::Empty => vec![],
            Body::Command { opcode, payload } => {
                // Write the opcode big-endian, then the payload, then dispatch.
                let mut code = Vec::new();
                let bytes: Vec<u8> = opcode
                    .to_be_bytes()
                    .into_iter()
                    .chain(payload.iter().copied())
                    .collect();
                for (offset, byte) in bytes.iter().enumerate() {
                    code.push(0x41);
                    code.extend(sleb(offset as i64));
                    code.push(0x41);
                    code.extend(sleb(i64::from(*byte)));
                    // i32.store8 align=0 offset=0
                    code.extend([0x3A, 0x00, 0x00]);
                }
                code.push(0x41);
                code.extend(sleb(0)); // buffer pointer
                code.push(0x41);
                code.extend(sleb(bytes.len() as i64)); // buffer length
                code.extend([0x10, 0x00]); // call 0 (the imported `cmd`)
                code.push(0x1A); // drop the response
                code
            }
            Body::OutOfBounds => {
                let mut code = Vec::new();
                code.push(0x41);
                code.extend(sleb(i64::from(i32::MAX))); // address
                code.push(0x41);
                code.extend(sleb(1)); // value
                code.extend([0x3A, 0x00, 0x00]); // i32.store8
                code
            }
            // loop; br 0; end — a backwards branch that never exits.
            Body::Forever => vec![0x03, 0x40, 0x0C, 0x00, 0x0B],
            Body::Grow { pages } => {
                let mut code = vec![0x41]; // i32.const
                code.extend(sleb(*pages));
                code.extend([0x40, 0x00]); // memory.grow, memidx 0
                code.push(0x1A); // drop the old size (or -1)
                code
            }
            Body::ReadTelemetry { id } => {
                let mut code = vec![0x42]; // i64.const
                code.extend(sleb(*id));
                for value in [0i64, 11, 16, 4] {
                    // time_ptr, time_len, value_ptr, value_len
                    code.push(0x41); // i32.const
                    code.extend(sleb(value));
                }
                code.extend([0x10, 0x00]); // call 0 (the imported `tlm`)
                code.push(0x1A); // drop the validity status
                code
            }
        };

        let mut function_body = uleb(0); // no locals
        function_body.extend(instructions);
        function_body.push(0x0B); // end
        let mut code_entry = uleb(function_body.len() as u64);
        code_entry.extend(function_body);
        out.extend(section(10, vector(1, code_entry)));

        out
    }
}

/// Write a module to a temporary file, since `verify::verify` takes a path.
fn temporary(bytes: &[u8], label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("fprime-wasm-{}-{label}.wasm", std::process::id()));
    std::fs::write(&path, bytes).expect("could not write the fixture");
    path
}

#[test]
fn the_harness_measures_and_judges_a_module() {
    a_nominal_module_passes_every_budget();
    the_guest_stack_high_water_mark_is_measured();
    a_module_with_no_stack_pointer_reports_no_guest_stack();
    a_dispatched_command_is_recorded_with_its_opcode();
    guest_memory_is_measured_against_the_budget();
    a_trap_is_reported_rather_than_hidden();
    an_endless_sequence_stops_at_the_instruction_limit();
    an_undersized_page_is_reported_as_a_failure();
    a_module_without_a_main_export_is_rejected();
    an_unknown_import_names_itself();
    the_static_view_agrees_with_the_fixture();
    the_table_report_shows_every_budget();
    the_summary_has_one_row_per_module();
    the_summary_omits_the_limits_it_states_once();
    informational_notes_are_held_back_until_asked_for();
    the_json_report_round_trips();
    the_json_summary_counts_unreadable_inputs();
    a_module_that_grows_memory_still_loads();
    growth_counts_toward_the_memory_budget();
    growth_past_the_pool_is_refused_without_trapping();
}

fn run(
    body: &fixture::Body,
    memory: u64,
    stack_pointer: Option<i32>,
    label: &str,
) -> verify::Verified {
    run_with(body, memory, stack_pointer, label, &Limits::default())
}

fn run_with(
    body: &fixture::Body,
    memory: u64,
    stack_pointer: Option<i32>,
    label: &str,
    limits: &Limits,
) -> verify::Verified {
    let path = temporary(&fixture::module(body, memory, stack_pointer), label);
    let checked = verify::verify(&path, limits, Responses::new())
        .unwrap_or_else(|err| panic!("{label} should have been checkable: {err:#}"));
    std::fs::remove_file(&path).ok();
    checked
}

/// The baseline: a module that does nothing must run, and must fit a stock
/// sequencer with room to spare. If this fails, the defaults are wrong.
fn a_nominal_module_passes_every_budget() {
    let checked = run(&fixture::Body::Empty, 941, None, "nominal");

    assert_eq!(checked.report.outcome, Outcome::Returned);
    assert!(
        checked.passed(),
        "a do-nothing sequence should fit the on-board defaults, but: {:?}",
        checked.failures()
    );
    for budget in &checked.budgets {
        assert!(
            budget.fits(),
            "{} needed {} but only {} is configured",
            budget.setting,
            budget.needed,
            budget.configured
        );
    }
    // Every allocation has to come from a page; one that cannot is a hard
    // failure on board and must never be silently absorbed here.
    assert_eq!(checked.report.usage.oversize, 0);
    assert!(
        checked.report.usage.largest > 0,
        "the interpreter must have allocated something"
    );
}

/// The measurement that motivates poisoning the stack region: how close a
/// sequence came to overflowing `-zstack-size`.
fn the_guest_stack_high_water_mark_is_measured() {
    // The command fixture stores its buffer at address 0, inside the stack
    // region, so a known number of bytes get touched.
    let opcode = 0x1000_7002;
    let payload = vec![0xAA, 0xBB];
    let checked = run(
        &fixture::Body::Command {
            opcode,
            payload: payload.clone(),
        },
        941,
        Some(512),
        "stack",
    );

    let stack = checked
        .report
        .guest_stack
        .expect("a module with a stack pointer has a stack region");
    assert_eq!(stack.reserved, 512, "the region is [0, __stack_pointer)");
    // Bytes 0..6 were written: the four-byte opcode and the two-byte payload.
    // The high-water mark is measured from the top of a downward-growing stack,
    // so writing the bottom 6 bytes reads as the whole region being reached.
    assert_eq!(
        stack.used, 512,
        "writing the bottom of the region means the deepest point is its top"
    );
    assert_eq!(stack.headroom(), 0);
}

/// `rustc` emits no `__stack_pointer` when a module never spills to linear
/// memory. That is not a measurement failure, and must not be reported as zero
/// headroom.
fn a_module_with_no_stack_pointer_reports_no_guest_stack() {
    let checked = run(&fixture::Body::Empty, 941, None, "no-stack-pointer");
    assert!(
        checked.report.guest_stack.is_none(),
        "no stack pointer means no guest stack to report"
    );
    assert!(checked.passed());
}

/// The opcode is a big-endian `FwOpcodeType` prefix. Reading it little-endian
/// would name the wrong command in every trace.
fn a_dispatched_command_is_recorded_with_its_opcode() {
    let opcode = 0x1000_7002;
    let payload = vec![0xAA, 0xBB, 0xCC];
    let checked = run(
        &fixture::Body::Command {
            opcode,
            payload: payload.clone(),
        },
        941,
        Some(512),
        "command",
    );

    let commands: Vec<(u32, Vec<u8>)> = checked
        .report
        .recording
        .commands()
        .map(|(opcode, payload)| (opcode, payload.to_vec()))
        .collect();
    assert_eq!(
        commands,
        vec![(opcode, payload)],
        "the opcode must be read big-endian and stripped from the payload"
    );
    assert_eq!(checked.report.outcome, Outcome::Returned);
}

/// `guestMemorySize` is the one budget that comes from the module's declaration
/// rather than from running it, and the custom page size has to be honoured or it
/// is out by 65536x.
fn guest_memory_is_measured_against_the_budget() {
    let checked = run(&fixture::Body::Empty, 941, None, "memory-fits");
    assert_eq!(checked.report.guest_memory, 941);

    let limits = Limits {
        guest_memory: 512,
        ..Limits::default()
    };
    let checked = run_with(&fixture::Body::Empty, 941, None, "memory-over", &limits);
    assert_eq!(checked.report.guest_memory, 941);
    let budget = checked
        .budgets
        .iter()
        .find(|budget| budget.setting == "guestMemorySize")
        .expect("a guestMemorySize budget");
    assert!(!budget.fits());
    assert!(!checked.passed());
    assert!(
        checked
            .failures()
            .iter()
            .any(|failure| failure.contains("guestMemorySize")),
        "{:?}",
        checked.failures()
    );
}

fn a_trap_is_reported_rather_than_hidden() {
    let checked = run(&fixture::Body::OutOfBounds, 941, None, "trap");
    assert!(
        matches!(checked.report.outcome, Outcome::Trapped(_)),
        "expected a trap, got {:?}",
        checked.report.outcome
    );
    assert!(!checked.passed());
    assert!(
        checked
            .failures()
            .iter()
            .any(|failure| failure.contains("trapped")),
        "{:?}",
        checked.failures()
    );
}

/// A sequence that loops forever must stop the tool, not hang it.
fn an_endless_sequence_stops_at_the_instruction_limit() {
    let limits = Limits {
        max_instructions: 5_000,
        ..Limits::default()
    };
    let checked = run_with(&fixture::Body::Forever, 941, None, "forever", &limits);
    assert_eq!(checked.report.outcome, Outcome::OutOfInstructions);
    assert!(checked.report.instructions >= 5_000);
    assert!(!checked.passed());
}

/// An allocation larger than a page can never be served, however many pages
/// there are, so it is a separate verdict from the page count.
fn an_undersized_page_is_reported_as_a_failure() {
    let limits = Limits {
        page_size: 64,
        ..Limits::default()
    };
    let checked = run_with(&fixture::Body::Empty, 941, None, "small-page", &limits);

    assert!(
        checked.required_page_size > 64,
        "the operand stack alone exceeds a 64-byte page"
    );
    assert!(!checked.passed());
    assert!(
        checked
            .failures()
            .iter()
            .any(|failure| failure.contains("PAGE_SIZE")),
        "{:?}",
        checked.failures()
    );
    // The run still had to complete, so that one invocation reports every
    // problem rather than stopping at the first.
    assert!(checked.report.usage.oversize > 0);
    assert_eq!(checked.report.outcome, Outcome::Returned);
}

fn a_module_without_a_main_export_is_rejected() {
    // Strip the export section by rebuilding without it: easier to just take a
    // valid module and rename the export.
    let mut bytes = fixture::module(&fixture::Body::Empty, 941, None);
    let position = bytes
        .windows(4)
        .position(|window| window == b"main")
        .expect("the fixture exports main");
    bytes[position..position + 4].copy_from_slice(b"nope");

    let path = temporary(&bytes, "no-main");
    let err = verify::verify(&path, &Limits::default(), Responses::new())
        .expect_err("a module without `main` cannot be run");
    std::fs::remove_file(&path).ok();

    let message = format!("{err:#}");
    assert!(message.contains(abi::ENTRY_POINT), "{message}");
    assert!(message.contains("fprime_main"), "{message}");
}

/// The diagnostic that the static view exists for: `spacewasm` cannot say which
/// import failed to resolve, so the name has to come from our own read.
fn an_unknown_import_names_itself() {
    let mut bytes = fixture::module(
        &fixture::Body::Command {
            opcode: 1,
            payload: vec![],
        },
        941,
        None,
    );
    // `cmd` is not a name the ABI has, once renamed.
    let position = bytes
        .windows(3)
        .position(|window| window == b"cmd")
        .expect("the fixture imports cmd");
    bytes[position..position + 3].copy_from_slice(b"zzz");

    let path = temporary(&bytes, "bad-import");
    let err = verify::verify(&path, &Limits::default(), Responses::new())
        .expect_err("an unresolvable import cannot be run");
    std::fs::remove_file(&path).ok();

    let message = format!("{err:#}");
    assert!(message.contains("fprime_v1.zzz"), "{message}");
    assert!(message.contains("not part of"), "{message}");
}

/// The static reader and the interpreter have to agree about the fixture, since
/// the reports mix numbers from both.
fn the_static_view_agrees_with_the_fixture() {
    let bytes = fixture::module(
        &fixture::Body::Command {
            opcode: 7,
            payload: vec![1],
        },
        941,
        Some(512),
    );
    let view = wasm::read(&bytes).expect("the fixture should parse");

    assert_eq!(view.sizes.total, bytes.len());
    let memory = view.memory.expect("a memory section");
    assert_eq!(memory.page_size, 1, "the custom page size must be honoured");
    assert_eq!(memory.initial_bytes(), 941);

    assert!(view.export(abi::ENTRY_POINT).is_some());
    let imports: Vec<&str> = view
        .imports_from(abi::MODULE)
        .map(|import| import.name.as_str())
        .collect();
    assert_eq!(imports, vec!["cmd"]);
    assert_eq!(view.imports_outside(abi::MODULE).count(), 0);
}

/// Every budget has to appear in the table, and each has to be labelled with the
/// `WasmSequencer::Config` field it sizes — that mapping is the point of the
/// report, and a renamed column would break it silently.
fn the_table_report_shows_every_budget() {
    let checked = run(&fixture::Body::Empty, 941, Some(512), "render");
    let rendered = verify::detail(&checked, None, false);

    for setting in [
        "guestMemorySize",
        "heapPages",
        "maxCodePages",
        "stackSize",
        "PAGE_SIZE",
    ] {
        assert!(
            rendered.contains(setting),
            "{setting} missing from:\n{rendered}"
        );
    }
    for header in ["Setting", "Needed", "Configured", "Unit", "Used", "Fits"] {
        assert!(
            rendered.contains(header),
            "{header} missing from:\n{rendered}"
        );
    }
    // A table with a misaligned rule is worse than no table.
    let rule = rendered
        .lines()
        .find(|line| line.trim_start().starts_with("---"))
        .expect("a rule under the budget header");
    let header = rendered
        .lines()
        .find(|line| line.contains("Setting"))
        .expect("a budget header");
    assert_eq!(
        rule.trim_end().len(),
        header.trim_end().len(),
        "the rule and its header must be the same width:\n{header}\n{rule}"
    );
    for line in rendered.lines() {
        assert_eq!(line, line.trim_end(), "trailing space in {line:?}");
    }
}

fn the_summary_has_one_row_per_module() {
    let modules = [
        run(&fixture::Body::Empty, 941, None, "sum-a"),
        run(&fixture::Body::Empty, 512, Some(512), "sum-b"),
    ];
    let summary = verify::summary(&modules);
    assert!(summary.contains("Status"), "{summary}");

    // Header, rule, then one row per module.
    assert_eq!(
        summary.lines().count(),
        2 + modules.len(),
        "unexpected summary shape:\n{summary}"
    );
    assert!(summary.contains("sum-a"), "{summary}");
    assert!(summary.contains("sum-b"), "{summary}");
    // The module with no stack pointer reads `-`, not `0/512`: those mean
    // different things.
    let row = summary
        .lines()
        .find(|line| line.contains("sum-a"))
        .expect("a row for sum-a");
    assert!(
        row.contains(" - "),
        "a module with no guest stack should read `-`: {row}"
    );
}

/// The JSON is a wire format, so it has to parse and carry the same figures the
/// table shows.
fn the_json_report_round_trips() {
    let checked = vec![run(
        &fixture::Body::Command {
            opcode: 0x1000_7002,
            payload: vec![0xAA],
        },
        941,
        Some(512),
        "json",
    )];
    let limits = Limits::default();

    let run = verify::report::Run::new(&checked, &[], &limits, None, true);
    let text = serde_json::to_string(&run).expect("serialises");
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");

    assert_eq!(value["schema"], verify::report::SCHEMA);
    assert_eq!(value["limits"]["guest_memory"], limits.guest_memory);
    assert_eq!(value["summary"]["inputs"], 1);
    assert_eq!(value["summary"]["passed"], 1);
    assert_eq!(value["summary"]["failed"], 0);

    let module = &value["modules"][0];
    assert_eq!(module["passed"], true);
    assert_eq!(module["guest"]["memory_bytes"], 941);
    assert_eq!(module["guest"]["stack"]["reserved"], 512);
    assert_eq!(module["outcome"]["kind"], "returned");

    // The opcode is carried in both bases and they must agree.
    let command = &module["commands"][0];
    assert_eq!(command["opcode"], 0x1000_7002_u32);
    assert_eq!(command["opcode_hex"], "0x10007002");
    assert_eq!(command["payload_bytes"], 1);

    // `--trace` was on, so the calls are present and tagged by ABI function.
    let calls = module["calls"].as_array().expect("calls with --trace");
    assert!(
        calls.iter().any(|call| call["function"] == "cmd"),
        "{calls:?}"
    );

    // And without it they are omitted entirely rather than being null.
    let quiet = serde_json::to_value(verify::report::Run::new(
        &checked,
        &[],
        &limits,
        None,
        false,
    ))
    .expect("serialises");
    assert!(quiet["modules"][0].get("calls").is_none());
}

/// The bug this caught: a module that could not be decoded made the exit status
/// non-zero while the JSON summary still reported a clean run, so a CI consumer
/// reading only the summary would disagree with the process it just ran.
fn the_json_summary_counts_unreadable_inputs() {
    let checked = vec![run(&fixture::Body::Empty, 941, None, "counted")];
    let unreadable = vec![(
        PathBuf::from("/tmp/not-a-module.wasm"),
        "not a Wasm module".to_string(),
    )];

    let value = serde_json::to_value(verify::report::Run::new(
        &checked,
        &unreadable,
        &Limits::default(),
        None,
        false,
    ))
    .expect("serialises");

    assert_eq!(value["summary"]["inputs"], 2);
    assert_eq!(value["summary"]["passed"], 1);
    assert_eq!(value["summary"]["failed"], 1);
    assert_eq!(value["summary"]["errors"], 1);
    assert_eq!(value["errors"][0]["path"], "/tmp/not-a-module.wasm");
    // Only measurable modules appear here, so every entry has a full set of
    // figures.
    assert_eq!(value["modules"].as_array().expect("modules").len(), 1);
}

/// The limits are global to a run, so repeating `/8192` on every row carries no
/// information — they are stated once by `limits_line` instead. This is the change
/// that took the 39-module report from 1094 lines to under 50.
fn the_summary_omits_the_limits_it_states_once() {
    let limits = Limits::default();
    let modules = [run(&fixture::Body::Empty, 941, Some(512), "narrow")];

    let line = verify::limits_line(&limits);
    assert!(line.contains(&limits.guest_memory.to_string()), "{line}");
    assert!(line.contains("8 pages"), "{line}");
    assert!(line.contains("1024 words"), "{line}");

    let summary = verify::summary(&modules);
    // The needed figure is there...
    assert!(summary.contains("941"), "{summary}");
    // ...and the configured one is not repeated on the row.
    for row in summary.lines().skip(2) {
        assert!(
            !row.contains("8192"),
            "the row restates a limit already in the header:\n{row}"
        );
    }
}

/// A note about an unsupplied channel reading zero fires on nearly every module,
/// so it must not crowd out the ones that say the sequencer will behave
/// differently from what the guest asked.
fn informational_notes_are_held_back_until_asked_for() {
    // Reading telemetry with no `--tlm` produces exactly that note.
    let checked = [run(
        &fixture::Body::ReadTelemetry { id: 7 },
        941,
        None,
        "info",
    )];

    assert_eq!(
        verify::info_count(&checked),
        1,
        "the zero-fill note should be counted as informational"
    );
    assert_eq!(
        verify::issues(&checked, false),
        "",
        "informational notes must be silent by default"
    );

    let verbose = verify::issues(&checked, true);
    assert!(verbose.contains("read as all zeroes"), "{verbose}");
    assert!(
        verbose.contains("info"),
        "the module should be named: {verbose}"
    );
}

/// `WasmSequencer` sets `allow_memory_grow = true`, so a module containing
/// `memory.grow` loads on board. Compiling it out here would fail a module that
/// works — a false failure, which is worse than a missed one.
fn a_module_that_grows_memory_still_loads() {
    let checked = run(&fixture::Body::Grow { pages: 8 }, 64, None, "grow-loads");
    assert_eq!(
        checked.report.outcome,
        Outcome::Returned,
        "a module using memory.grow must load and run"
    );
}

/// The grown size is what the guest pool has to hold, so the budget is measured
/// against the peak rather than the declaration.
fn growth_counts_toward_the_memory_budget() {
    // Declares 64 one-byte pages, grows by 8 more.
    let checked = run(&fixture::Body::Grow { pages: 8 }, 64, None, "grow-budget");

    assert_eq!(checked.report.declared_memory, 64);
    assert_eq!(
        checked.report.guest_memory, 72,
        "the budget must see the grown size"
    );
    assert_eq!(checked.report.refused_grows, 0);

    let budget = checked
        .budgets
        .iter()
        .find(|budget| budget.setting == "guestMemorySize")
        .expect("a guestMemorySize budget");
    assert_eq!(budget.needed, 72);

    // A module that never grows reports its declared size, not zero.
    let flat = run(&fixture::Body::Empty, 941, None, "grow-none");
    assert_eq!(flat.report.guest_memory, 941);
    assert_eq!(flat.report.declared_memory, 941);
}

/// On board a grow past the pool returns -1 to the guest and logs
/// `MemoryGrowRejected`; it does not trap. An unbounded pool here would instead
/// report a module as fitting when it would not.
fn growth_past_the_pool_is_refused_without_trapping() {
    let limits = Limits {
        guest_memory: 64,
        ..Limits::default()
    };
    let checked = run_with(
        &fixture::Body::Grow { pages: 1_000 },
        64,
        None,
        "grow-refused",
        &limits,
    );

    assert_eq!(
        checked.report.outcome,
        Outcome::Returned,
        "a refused grow must not trap the guest"
    );
    assert_eq!(checked.report.refused_grows, 1);
    // The refusal held the peak at the declared size, so the budget still fits.
    assert_eq!(checked.report.guest_memory, 64);
    assert!(checked.passed(), "{:?}", checked.failures());
}

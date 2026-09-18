//! The `--json` wire format.
//!
//! Separate types from [`crate::verify::Verified`] on purpose: those are shaped for
//! measuring and free to change, this is parsed by something else. Changing it
//! should be deliberate, not a side effect of a refactor.

use crate::harness::{Call, Outcome};
use crate::verify::Verified;
use fprime_dictionary::Dictionary;
use serde::Serialize;

/// Bump when a field is removed or its meaning changes.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Serialize)]
pub struct Run {
    pub schema: u32,
    /// What the modules were measured against.
    pub limits: Limits,
    pub modules: Vec<Module>,
    /// Inputs with no measurements. Kept out of `modules` so every entry there
    /// has a full set of figures, but counted in `summary.failed` so the summary
    /// agrees with the exit status.
    pub errors: Vec<ModuleError>,
    pub summary: Summary,
}

#[derive(Debug, Serialize)]
pub struct ModuleError {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct Limits {
    pub page_size: usize,
    pub heap_pages: u32,
    pub guest_memory: u64,
    pub stack_size: usize,
    pub max_code_pages: usize,
    pub max_guest_modules: u8,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    /// Given, measurable or not.
    pub inputs: usize,
    /// Within every budget.
    pub passed: usize,
    /// Over budget, or not measurable.
    pub failed: usize,
    /// How many of `failed` could not be decoded.
    pub errors: usize,
}

#[derive(Debug, Serialize)]
pub struct Module {
    pub path: String,
    /// Ran nominally and every budget fits.
    pub passed: bool,
    /// Empty when `passed`.
    pub failures: Vec<String>,
    pub outcome: OutcomeJson,
    pub instructions: u64,
    pub sizes: Sizes,
    pub budgets: Vec<Budget>,
    pub page_size: PageSize,
    pub guest: Guest,
    pub interpreter: Interpreter,
    pub commands: Vec<Command>,
    pub telemetry_read: Vec<Id>,
    pub parameters_read: Vec<Id>,
    pub relative_sleep_us: u64,
    /// Behaviour differences the guest cannot see.
    pub notes: Vec<String>,
    /// Only present with `--trace`: a long trace dwarfs the rest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calls: Option<Vec<CallJson>>,
}

#[derive(Debug, Serialize)]
pub struct Sizes {
    pub total: usize,
    pub code: usize,
    pub data: usize,
}

/// Tagged, so a consumer matches on `kind` rather than parsing prose.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutcomeJson {
    Exited { code: i32, description: String },
    Panicked { code: i32, description: String },
    Returned { description: String },
    Trapped { reason: String, description: String },
    Suspended { description: String },
    OutOfInstructions { description: String },
}

impl OutcomeJson {
    fn new(outcome: &Outcome) -> Self {
        let description = crate::verify::describe_outcome(outcome);
        match outcome {
            Outcome::Exited(code) => OutcomeJson::Exited {
                code: *code,
                description,
            },
            Outcome::Panicked(code) => OutcomeJson::Panicked {
                code: *code,
                description,
            },
            Outcome::Returned => OutcomeJson::Returned { description },
            Outcome::Trapped(reason) => OutcomeJson::Trapped {
                reason: reason.clone(),
                description,
            },
            Outcome::Suspended => OutcomeJson::Suspended { description },
            Outcome::OutOfInstructions => OutcomeJson::OutOfInstructions { description },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Budget {
    /// The `Config` field this sizes.
    pub setting: String,
    pub unit: String,
    pub needed: u64,
    pub configured: u64,
    pub fits: bool,
}

/// Apart from the other budgets: a page bounds the largest *single* allocation.
#[derive(Debug, Serialize)]
pub struct PageSize {
    pub required: usize,
    pub configured: usize,
    pub fits: bool,
    pub largest_allocation: usize,
}

#[derive(Debug, Serialize)]
pub struct Guest {
    /// Peak linear memory in bytes, growth included: what the pool must hold.
    pub memory_bytes: u64,
    /// What the module declared before running.
    pub declared_bytes: u64,
    /// `memory.grow` requests the pool refused; the guest saw -1 and continued.
    pub refused_grows: u64,
    /// `null` when no `__stack_pointer` is declared — not the same as unused.
    pub stack: Option<GuestStack>,
}

#[derive(Debug, Serialize)]
pub struct GuestStack {
    pub reserved: usize,
    pub used: usize,
    pub spare: usize,
}

#[derive(Debug, Serialize)]
pub struct Interpreter {
    pub code_pages: usize,
    pub code_words: usize,
    pub code_capacity: usize,
    pub allocations: u64,
    pub peak_bytes: usize,
    pub padding_bytes: u32,
    pub resident_bytes: usize,
    pub peak_pages: u32,
    /// Larger than one page: each is a hard failure on board.
    pub oversize: u64,
}

#[derive(Debug, Serialize)]
pub struct Command {
    pub opcode: u32,
    /// Hex too: that is how an opcode is written in the dictionary, FPP and
    /// ground tools.
    pub opcode_hex: String,
    /// When a dictionary was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub payload_bytes: usize,
}

/// A channel or parameter read.
#[derive(Debug, Serialize)]
pub struct Id {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CallJson {
    /// The `fprime_v1` function called.
    pub function: String,
    /// As the terminal trace shows it.
    pub description: String,
}

impl Run {
    /// Build the report for a whole run.
    pub fn new(
        checked: &[Verified],
        errors: &[(std::path::PathBuf, String)],
        limits: &crate::harness::Limits,
        dictionary: Option<&Dictionary>,
        trace: bool,
    ) -> Self {
        let modules: Vec<Module> = checked
            .iter()
            .map(|checked| Module::new(checked, dictionary, trace))
            .collect();
        let passed = modules.iter().filter(|module| module.passed).count();
        let errors: Vec<ModuleError> = errors
            .iter()
            .map(|(path, error)| ModuleError {
                path: path.to_string_lossy().into_owned(),
                error: error.clone(),
            })
            .collect();

        Run {
            schema: SCHEMA,
            limits: Limits {
                page_size: limits.page_size,
                heap_pages: limits.heap_pages,
                guest_memory: limits.guest_memory,
                stack_size: limits.stack_size,
                max_code_pages: limits.max_code_pages,
                max_guest_modules: limits.max_guest_modules,
            },
            summary: Summary {
                inputs: modules.len() + errors.len(),
                passed,
                failed: (modules.len() - passed) + errors.len(),
                errors: errors.len(),
            },
            modules,
            errors,
        }
    }
}

impl Module {
    fn new(checked: &Verified, dictionary: Option<&Dictionary>, trace: bool) -> Self {
        let report = &checked.report;
        let recording = &report.recording;

        Module {
            // Lossy rather than refusing: a path we could read but cannot render
            // as UTF-8 should still produce a report.
            path: checked.path.to_string_lossy().into_owned(),
            passed: checked.passed(),
            failures: checked.failures(),
            outcome: OutcomeJson::new(&report.outcome),
            instructions: report.instructions,
            sizes: Sizes {
                total: checked.sizes.total,
                code: checked.sizes.code,
                data: checked.sizes.data,
            },
            budgets: checked
                .budgets
                .iter()
                .map(|budget| Budget {
                    setting: budget.setting.to_string(),
                    unit: budget.unit.to_string(),
                    needed: budget.needed,
                    configured: budget.configured,
                    fits: budget.fits(),
                })
                .collect(),
            page_size: PageSize {
                required: checked.required_page_size,
                configured: checked.configured_page_size,
                fits: checked.required_page_size <= checked.configured_page_size,
                largest_allocation: report.usage.largest,
            },
            guest: Guest {
                memory_bytes: report.guest_memory,
                declared_bytes: report.declared_memory,
                refused_grows: report.refused_grows,
                stack: report.guest_stack.map(|stack| GuestStack {
                    reserved: stack.reserved,
                    used: stack.used,
                    spare: stack.headroom(),
                }),
            },
            interpreter: Interpreter {
                code_pages: report.code_pages,
                code_words: report.code_words,
                code_capacity: report.code_capacity,
                allocations: report.usage.allocations,
                peak_bytes: report.usage.peak_live,
                padding_bytes: report.usage.padding,
                resident_bytes: report.usage.resident,
                peak_pages: report.usage.peak_pages,
                oversize: report.usage.oversize,
            },
            commands: recording
                .commands()
                .map(|(opcode, payload)| Command {
                    opcode,
                    opcode_hex: format!("{opcode:#010x}"),
                    name: crate::verify::command_name(dictionary, opcode),
                    payload_bytes: payload.len(),
                })
                .collect(),
            telemetry_read: recording
                .telemetry_read()
                .into_iter()
                .map(|id| Id {
                    id,
                    name: crate::verify::channel_name(dictionary, id),
                })
                .collect(),
            parameters_read: recording
                .parameters_read()
                .into_iter()
                .map(|id| Id {
                    id,
                    name: crate::verify::parameter_name(dictionary, id),
                })
                .collect(),
            relative_sleep_us: recording.relative_sleep_us(),
            notes: recording
                .issues
                .iter()
                .map(|finding| finding.message.clone())
                .collect(),
            calls: trace.then(|| {
                recording
                    .calls
                    .iter()
                    .map(|call| CallJson {
                        function: function_of(call).to_string(),
                        description: crate::verify::describe_call(call, dictionary),
                    })
                    .collect()
            }),
        }
    }
}

/// So a consumer can filter by function without parsing the description.
fn function_of(call: &Call) -> &'static str {
    match call {
        Call::Exit { .. } => "exit",
        Call::Panic { .. } => "panic",
        Call::Args { .. } => "args",
        Call::Time { .. } => "time",
        Call::Telemetry { .. } => "tlm",
        Call::Parameter { .. } => "prm",
        Call::Command { .. } => "cmd",
        Call::Event { .. } => "event",
        Call::RelativeSleep { .. } => "rsleep",
        Call::AbsoluteSleep { .. } => "asleep",
        Call::SerialSend { .. } => "serial_send",
        Call::SerialRecv { .. } => "serial_recv",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi;

    /// Every name here must be one the host module actually registers, or a
    /// consumer filtering on it silently gets nothing.
    #[test]
    fn every_call_maps_to_an_abi_function() {
        let calls = [
            Call::Exit { code: 0 },
            Call::Panic { code: 1 },
            Call::Args {
                capacity: 0,
                written: 0,
            },
            Call::Time { len: 11 },
            Call::Telemetry {
                id: 1,
                value_len: 4,
                status: 0,
            },
            Call::Parameter {
                id: 1,
                value_len: 4,
                status: 1,
            },
            Call::Command {
                opcode: 1,
                payload: vec![],
                response: 0,
            },
            Call::Event {
                severity: 5,
                message: String::new(),
                truncated: false,
            },
            Call::RelativeSleep { us: 1 },
            Call::AbsoluteSleep { us: 1 },
            Call::SerialSend { index: 0, len: 0 },
            Call::SerialRecv {
                index: 0,
                blocking: false,
                received: 0,
                status: 1,
            },
        ];

        // One per registered function, and each name is registered.
        assert_eq!(calls.len(), abi::FUNCTIONS.len());
        for call in &calls {
            let name = function_of(call);
            assert!(
                abi::function(name).is_some(),
                "`{name}` is not a registered host function"
            );
        }

        let mut names: Vec<&str> = calls.iter().map(function_of).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), calls.len(), "two calls map to the same name");
    }

    #[test]
    fn an_outcome_is_tagged_and_carries_its_payload() {
        let json = |outcome: &Outcome| {
            serde_json::to_value(OutcomeJson::new(outcome)).expect("serialises")
        };

        let trapped = json(&Outcome::Trapped("MemoryOutOfBounds".into()));
        assert_eq!(trapped["kind"], "trapped");
        assert_eq!(trapped["reason"], "MemoryOutOfBounds");
        assert!(
            trapped["description"]
                .as_str()
                .expect("prose")
                .contains("trapped")
        );

        let panicked = json(&Outcome::Panicked(7));
        assert_eq!(panicked["kind"], "panicked");
        assert_eq!(panicked["code"], 7);

        assert_eq!(json(&Outcome::Returned)["kind"], "returned");
        assert_eq!(json(&Outcome::Exited(0))["code"], 0);
        assert_eq!(
            json(&Outcome::OutOfInstructions)["kind"],
            "out_of_instructions"
        );
    }

    /// An opcode is written in hex everywhere else in F Prime, so the report
    /// carries both and they have to agree.
    #[test]
    fn a_command_opcode_is_given_in_both_bases() {
        let command = Command {
            opcode: 0x1000_7002,
            opcode_hex: format!("{:#010x}", 0x1000_7002),
            name: None,
            payload_bytes: 0,
        };
        let json = serde_json::to_value(&command).expect("serialises");
        assert_eq!(json["opcode"], 0x1000_7002_u32);
        assert_eq!(json["opcode_hex"], "0x10007002");
        // An absent name is omitted rather than null, so `jq` filters stay simple.
        assert!(json.get("name").is_none());
    }

    #[test]
    fn a_named_command_keeps_its_name() {
        let json = serde_json::to_value(Command {
            opcode: 1,
            opcode_hex: "0x00000001".into(),
            name: Some("CdhCore.cmdDisp.CMD_NO_OP".into()),
            payload_bytes: 3,
        })
        .expect("serialises");
        assert_eq!(json["name"], "CdhCore.cmdDisp.CMD_NO_OP");
    }

    /// A guest with no stack pointer must be `null`, not zero: those mean
    /// different things and a consumer sizing `-zstack-size` needs to tell them
    /// apart.
    #[test]
    fn an_absent_guest_stack_is_null_not_zero() {
        let json = serde_json::to_value(Guest {
            memory_bytes: 941,
            declared_bytes: 941,
            refused_grows: 0,
            stack: None,
        })
        .expect("serialises");
        assert!(json["stack"].is_null());

        let json = serde_json::to_value(Guest {
            memory_bytes: 941,
            declared_bytes: 941,
            refused_grows: 0,
            stack: Some(GuestStack {
                reserved: 512,
                used: 0,
                spare: 512,
            }),
        })
        .expect("serialises");
        assert_eq!(json["stack"]["used"], 0);
    }
}

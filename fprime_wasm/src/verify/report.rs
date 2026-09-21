//! The `--json` wire format, kept separate from [`crate::verify::Verified`].

use crate::verify::Verified;
use serde::Serialize;

/// Bump when a field is removed or its meaning changes.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Serialize)]
pub struct Run {
    pub schema: u32,
    /// What the modules were loaded under.
    pub limits: Limits,
    pub modules: Vec<Module>,
    /// Inputs with no measurements (kept out of `modules`, counted in `summary.failed`).
    pub errors: Vec<ModuleError>,
    pub summary: Summary,
}

#[derive(Debug, Serialize)]
pub struct ModuleError {
    pub path: String,
    pub error: String,
}

/// The deployment's configuration, echoed whole — including the settings only a run can
/// size, so a consumer can diff this against `sequencer.toml`.
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
    /// Given, loadable or not.
    pub inputs: usize,
    /// Within every budget.
    pub passed: usize,
    /// Over budget, or would not load.
    pub failed: usize,
    /// How many of `failed` could not be loaded at all.
    pub errors: usize,
}

#[derive(Debug, Serialize)]
pub struct Module {
    pub path: String,
    /// Loaded, and every budget fits.
    pub passed: bool,
    /// Empty when `passed`.
    pub failures: Vec<String>,
    pub sizes: Sizes,
    pub budgets: Vec<Budget>,
    pub page_size: PageSize,
    pub guest: Guest,
    pub interpreter: Interpreter,
}

#[derive(Debug, Serialize)]
pub struct Sizes {
    pub total: usize,
    pub code: usize,
    pub data: usize,
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

/// A page bounds the largest single allocation.
#[derive(Debug, Serialize)]
pub struct PageSize {
    pub required: usize,
    pub configured: usize,
    pub fits: bool,
    pub largest_allocation: usize,
}

#[derive(Debug, Serialize)]
pub struct Guest {
    /// Linear memory the module declares, which the pool must hold before it grows.
    pub declared_bytes: u64,
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

impl Run {
    /// Build the report for a whole run.
    pub fn new(
        checked: &[Verified],
        errors: &[(std::path::PathBuf, String)],
        limits: &fprime_test::interpreter::Limits,
    ) -> Self {
        let modules: Vec<Module> = checked.iter().map(Module::new).collect();
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
    fn new(checked: &Verified) -> Self {
        let loaded = &checked.loaded;

        Module {
            // Lossy: an unrenderable path still gets a report.
            path: checked.path.to_string_lossy().into_owned(),
            passed: checked.passed(),
            failures: checked.failures(),
            sizes: Sizes {
                total: loaded.sizes.total,
                code: loaded.sizes.code,
                data: loaded.sizes.data,
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
                largest_allocation: loaded.usage.largest,
            },
            guest: Guest {
                declared_bytes: loaded.declared_memory,
            },
            interpreter: Interpreter {
                code_pages: loaded.cost.code_pages,
                code_words: loaded.cost.code_words,
                code_capacity: loaded.cost.code_capacity,
                allocations: loaded.usage.allocations,
                peak_bytes: loaded.usage.peak_live,
                padding_bytes: loaded.usage.padding,
                resident_bytes: loaded.usage.resident,
                peak_pages: loaded.usage.peak_pages,
                oversize: loaded.usage.oversize,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;
    use std::path::PathBuf;

    fn json(checked: &[Verified], errors: &[(PathBuf, String)]) -> serde_json::Value {
        let run = Run::new(
            checked,
            errors,
            &fprime_test::interpreter::Limits::default(),
        );
        serde_json::to_value(&run).expect("serialises")
    }

    #[test]
    fn a_loaded_module_reports_its_figures() {
        let json = json(&[fixture::nominal("safing")], &[]);
        assert_eq!(json["schema"], SCHEMA);
        assert_eq!(json["summary"]["inputs"], 1);
        assert_eq!(json["summary"]["passed"], 1);
        assert_eq!(json["summary"]["failed"], 0);

        let module = &json["modules"][0];
        assert_eq!(module["passed"], true);
        assert_eq!(module["sizes"]["total"], 641);
        assert_eq!(module["guest"]["declared_bytes"], 941);
        assert_eq!(module["interpreter"]["code_pages"], 2);
        assert_eq!(module["page_size"]["largest_allocation"], 4096);
        assert!(module["failures"].as_array().expect("an array").is_empty());
    }

    /// Nothing ran, so nothing may claim to know what the sequence did.
    #[test]
    fn no_run_only_fields_survive() {
        let json = json(&[fixture::nominal("safing")], &[]);
        let module = &json["modules"][0];
        for gone in [
            "outcome",
            "instructions",
            "calls",
            "commands",
            "telemetry_read",
            "parameters_read",
            "relative_sleep_us",
            "notes",
        ] {
            assert!(module.get(gone).is_none(), "`{gone}` is still reported");
        }
        assert!(module["guest"].get("stack").is_none());
        assert!(module["guest"].get("refused_grows").is_none());
    }

    /// A module that would not load has no figures, so it is reported apart from the ones
    /// that do — but it still counts as a failure.
    #[test]
    fn unreadable_inputs_counted_but_not_measured() {
        let json = json(
            &[fixture::nominal("safing")],
            &[(PathBuf::from("broken.wasm"), "not a module".to_string())],
        );
        assert_eq!(json["summary"]["inputs"], 2);
        assert_eq!(json["summary"]["passed"], 1);
        assert_eq!(json["summary"]["failed"], 1);
        assert_eq!(json["summary"]["errors"], 1);
        assert_eq!(json["errors"][0]["path"], "broken.wasm");
        assert_eq!(json["errors"][0]["error"], "not a module");
        assert_eq!(json["modules"].as_array().expect("an array").len(), 1);
    }

    /// The configuration is echoed whole, `stackSize` included: it is what the deployment
    /// set, not something `verify` measured.
    #[test]
    fn limits_echo_config() {
        let json = json(&[], &[]);
        let limits = &json["limits"];
        assert_eq!(limits["page_size"], 8192);
        assert_eq!(limits["stack_size"], 1024);
        assert_eq!(limits["max_guest_modules"], 8);
    }

    #[test]
    fn module_over_budget() {
        let tight = fprime_test::interpreter::Limits {
            max_code_pages: 1,
            ..fprime_test::interpreter::Limits::default()
        };
        let one = fixture::verified("big", fixture::validated(), &tight);
        let json = json(&[one], &[]);
        let module = &json["modules"][0];
        assert_eq!(module["passed"], false);
        let failure = module["failures"][0].as_str().expect("prose");
        assert!(failure.contains("maxCodePages"), "{failure}");
    }
}

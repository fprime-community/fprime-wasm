//! Running a module and sizing it against a configuration.

use super::describe::describe_outcome;
use crate::harness::{self, Limits, Report};
use crate::wasm;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// One sized resource: needed against configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    /// The `WasmSequencer` setting this sizes.
    pub setting: &'static str,
    pub unit: &'static str,
    pub needed: u64,
    pub configured: u64,
}

impl Budget {
    pub fn fits(&self) -> bool {
        self.needed <= self.configured
    }

    /// `None` when nothing is configured, which would divide by zero.
    pub fn utilisation(&self) -> Option<f64> {
        (self.configured > 0).then(|| self.needed as f64 / self.configured as f64)
    }
}

/// Everything `verify` concluded about one module.
#[derive(Debug)]
pub struct Verified {
    pub path: PathBuf,
    pub sizes: wasm::Sizes,
    pub budgets: Vec<Budget>,
    pub report: Report,
    /// A page bounds the largest *single* allocation, so this fails independently
    /// of the page count.
    pub required_page_size: usize,
    pub configured_page_size: usize,
}

impl Verified {
    /// What the process exit status reflects.
    pub fn passed(&self) -> bool {
        self.report.outcome.is_nominal()
            && self.budgets.iter().all(Budget::fits)
            && self.required_page_size <= self.configured_page_size
            && self.report.usage.oversize == 0
    }

    pub fn failures(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if !self.report.outcome.is_nominal() {
            failures.push(describe_outcome(&self.report.outcome));
        }
        if self.required_page_size > self.configured_page_size {
            failures.push(format!(
                "a single allocation of {} bytes cannot be served by a {}-byte page; \
                 WASM_SEQ_SPACEWASM_PAGE_SIZE must be at least {}",
                self.report.usage.largest, self.configured_page_size, self.required_page_size
            ));
        }
        for budget in self.budgets.iter().filter(|budget| !budget.fits()) {
            failures.push(format!(
                "{} needs {} {} but is configured for {}",
                budget.setting, budget.needed, budget.unit, budget.configured
            ));
        }
        failures
    }
}

/// Run and size one module.
pub fn verify(path: &Path, limits: &Limits, responses: harness::Responses) -> Result<Verified> {
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("could not read {}: {err}", path.display()))?;
    let static_view =
        // `{err:#}` rather than `{err}`: for a structural problem the useful part
        // is the cause chain, where `wasmparser` names what it found and where.
        wasm::read(&bytes).map_err(|err| anyhow::anyhow!("{}: {err:#}", path.display()))?;

    let report = harness::run(bytes, &static_view, limits, responses)?;

    let budgets = vec![
        Budget {
            setting: "guestMemorySize",
            unit: "bytes",
            needed: report.guest_memory,
            configured: limits.guest_memory,
        },
        Budget {
            setting: "heapPages",
            unit: "pages",
            needed: u64::from(report.required_heap_pages()),
            configured: u64::from(limits.heap_pages),
        },
        Budget {
            setting: "maxCodePages",
            unit: "pages",
            needed: report.code_pages as u64,
            configured: limits.max_code_pages as u64,
        },
        Budget {
            setting: "stackSize",
            unit: "words",
            needed: report.peak_operand_stack as u64,
            configured: limits.stack_size as u64,
        },
    ];

    Ok(Verified {
        path: path.to_path_buf(),
        sizes: static_view.sizes,
        budgets,
        required_page_size: report.usage.required_page_size(),
        configured_page_size: limits.page_size,
        report,
    })
}

/// `.wasm` files in `directory`, sorted.
pub fn discover(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut modules: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|err| anyhow::anyhow!("could not list {}: {err}", directory.display()))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect();
    modules.sort();
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(needed: u64, configured: u64) -> Budget {
        Budget {
            setting: "guestMemorySize",
            unit: "bytes",
            needed,
            configured,
        }
    }

    #[test]
    fn a_budget_fits_up_to_and_including_its_limit() {
        assert!(budget(0, 8192).fits());
        assert!(budget(8191, 8192).fits());
        assert!(budget(8192, 8192).fits());
        assert!(!budget(8193, 8192).fits());
    }

    #[test]
    fn utilisation_is_a_fraction_and_guards_a_zero_budget() {
        assert_eq!(budget(4096, 8192).utilisation(), Some(0.5));
        assert_eq!(budget(8192, 8192).utilisation(), Some(1.0));
        assert_eq!(budget(1, 0).utilisation(), None);
    }
}

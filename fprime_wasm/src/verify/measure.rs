//! Loading a module and sizing it against a configuration.

use anyhow::Result;
use fprime_test::interpreter;
use fprime_test::interpreter::{Limits, Validated};
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
    pub loaded: Validated,
    pub budgets: Vec<Budget>,
    /// A page bounds the largest *single* allocation, so this fails independently
    /// of the page count.
    pub required_page_size: usize,
    pub configured_page_size: usize,
}

impl Verified {
    /// What the process exit status reflects.
    pub fn passed(&self) -> bool {
        self.budgets.iter().all(Budget::fits)
            && self.required_page_size <= self.configured_page_size
            && self.loaded.usage.oversize == 0
    }

    pub fn failures(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if self.required_page_size > self.configured_page_size {
            failures.push(format!(
                "a single allocation of {} bytes cannot be served by a {}-byte page; \
                 WASM_SEQ_SPACEWASM_PAGE_SIZE must be at least {}",
                self.loaded.usage.largest, self.configured_page_size, self.required_page_size
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

/// Load one module and size it
pub fn verify(path: &Path, limits: &Limits) -> Result<Verified> {
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("could not read {}: {err}", path.display()))?;
    // `{err:#}` surfaces the cause chain.
    let loaded = interpreter::validate(bytes, limits)
        .map_err(|err| anyhow::anyhow!("{}: {err:#}", path.display()))?;

    Ok(Verified {
        path: path.to_path_buf(),
        budgets: budgets(&loaded, limits),
        required_page_size: loaded.usage.required_page_size(),
        configured_page_size: limits.page_size,
        loaded,
    })
}

fn budgets(loaded: &Validated, limits: &Limits) -> Vec<Budget> {
    vec![
        Budget {
            setting: "guestMemorySize",
            unit: "bytes",
            needed: loaded.declared_memory,
            configured: limits.guest_memory,
        },
        Budget {
            setting: "heapPages",
            unit: "pages",
            needed: u64::from(loaded.usage.peak_pages),
            configured: u64::from(limits.heap_pages),
        },
        Budget {
            setting: "maxCodePages",
            unit: "pages",
            needed: loaded.cost.code_pages as u64,
            configured: limits.max_code_pages as u64,
        },
    ]
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
    fn budget_fits_up_to_limit() {
        assert!(budget(0, 8192).fits());
        assert!(budget(8191, 8192).fits());
        assert!(budget(8192, 8192).fits());
        assert!(!budget(8193, 8192).fits());
    }

    #[test]
    fn utilisation_guards_zero_budget() {
        assert_eq!(budget(4096, 8192).utilisation(), Some(0.5));
        assert_eq!(budget(8192, 8192).utilisation(), Some(1.0));
        assert_eq!(budget(1, 0).utilisation(), None);
    }

    /// `stackSize` must not creep back in: it cannot be sized without running.
    #[test]
    fn every_sizeable_setting_is_budgeted() {
        let settings: Vec<&str> = budgets(&super::super::fixture::validated(), &Limits::default())
            .iter()
            .map(|budget| budget.setting)
            .collect();
        assert_eq!(
            settings,
            vec!["guestMemorySize", "heapPages", "maxCodePages"]
        );
    }
}

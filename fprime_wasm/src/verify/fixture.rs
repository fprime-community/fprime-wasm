//! Measurements to render

use super::{Budget, Verified};
use fprime_test::interpreter::{Cost, Limits, Usage, Validated};
use fprime_test::wasm;
use std::path::PathBuf;

/// A module that loaded, with a figure in every field the reports read.
pub fn validated() -> Validated {
    Validated {
        sizes: wasm::Sizes {
            total: 641,
            code: 355,
            data: 96,
        },
        cost: Cost {
            code_pages: 2,
            code_words: 299,
            code_capacity: 512,
        },
        declared_memory: 941,
        usage: Usage {
            largest: 4096,
            peak_live: 9942,
            resident: 9000,
            allocations: 37,
            pages: 2,
            padding: 4,
            page_bytes: 8192,
            peak_pages: 2,
            oversize: 0,
        },
    }
}

/// [`validated`], sized against `limits` and named `stem`.
pub fn verified(stem: &str, loaded: Validated, limits: &Limits) -> Verified {
    Verified {
        path: PathBuf::from(format!("target/wasm32v1-none/release/{stem}.wasm")),
        budgets: vec![
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
        ],
        required_page_size: loaded.usage.required_page_size(),
        configured_page_size: limits.page_size,
        loaded,
    }
}

/// One module, measured against stock limits.
pub fn nominal(stem: &str) -> Verified {
    verified(stem, validated(), &Limits::default())
}

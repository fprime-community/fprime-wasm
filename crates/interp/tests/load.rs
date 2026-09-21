//! Loading a module: what `fprime-wasm verify` does, and no more.
//!
//! Every test goes through [`fprime_test::interpreter`], which holds the lock that keeps two
//! modules out of `spacewasm`'s single-threaded allocator at once — so libtest's threads
//! queue here rather than running concurrently.

mod fixture;

use fprime_test::abi;
use fprime_test::interpreter;
use fprime_test::interpreter::Limits;
use fprime_test::wasm;

#[test]
fn loading_measures_code_and_declared_memory() {
    let bytes = fixture::module("nominal");
    let declared = fixture::declared_memory(&bytes);
    let loaded =
        interpreter::validate(bytes.clone(), &Limits::default()).expect("nominal should load");

    assert_eq!(loaded.sizes.total, bytes.len());
    assert_eq!(
        loaded.declared_memory, declared,
        "the interpreter and the static read must agree on the memory section"
    );
    assert!(loaded.cost.code_pages > 0, "the module compiled to nothing");
    assert!(
        loaded.cost.code_words <= loaded.cost.code_capacity,
        "{:?}",
        loaded.cost
    );
    assert!(
        loaded.usage.largest > 0,
        "the interpreter must have allocated something"
    );
    assert_eq!(loaded.usage.oversize, 0);
}

/// The whole point of the split: a sequence that would trap, or never stop, still loads.
/// Neither could be reported as a pass if loading ran the module.
#[test]
fn loading_does_not_execute() {
    for (sequence, label) in [("trapping", "would trap"), ("forever", "never returns")] {
        let loaded = interpreter::validate(fixture::module(sequence), &Limits::default())
            .unwrap_or_else(|err| panic!("a module that {label} should still load: {err:#}"));
        assert!(loaded.cost.code_pages > 0);
    }
}

/// `guestMemorySize` is sized from the module's own declaration; growth needs a run.
#[test]
fn loading_reports_the_declared_memory_not_the_grown_one() {
    let bytes = fixture::module("growing");
    let declared = fixture::declared_memory(&bytes);
    let loaded = interpreter::validate(bytes, &Limits::default()).expect("growing should load");

    assert_eq!(
        loaded.declared_memory, declared,
        "the `memory.grow` in the body must not show up until the module runs"
    );
}

/// An oversized allocation is reported without running: it happens while decoding.
#[test]
fn undersized_page_seen_at_load() {
    let limits = Limits {
        page_size: 64,
        ..Limits::default()
    };
    let loaded =
        interpreter::validate(fixture::module("nominal"), &limits).expect("nominal should load");

    assert!(
        loaded.usage.required_page_size() > 64,
        "the code page table alone exceeds a 64-byte page"
    );
    assert!(loaded.usage.oversize > 0);
}

/// Static reader and interpreter must agree on a real module.
#[test]
fn static_view_agrees_with_interpreter() {
    let bytes = fixture::module("dispatch");
    let view = wasm::read(&bytes).expect("a linked module should parse");

    assert_eq!(view.sizes.total, bytes.len());
    let memory = view.memory.expect("a memory section");
    assert_eq!(
        memory.page_size, 1,
        "`--page-size=1` is set for the whole workspace"
    );

    assert!(view.export(abi::ENTRY_POINT).is_some());
    let imports: Vec<&str> = view
        .imports_from(abi::MODULE)
        .map(|import| import.name.as_str())
        .collect();
    assert!(
        imports.contains(&"cmd"),
        "a sequence that sends a command imports `cmd`: {imports:?}"
    );
    assert_eq!(
        view.imports_outside(abi::MODULE).count(),
        0,
        "a sequence may import nothing but the host ABI"
    );

    let loaded = interpreter::validate(bytes, &Limits::default()).expect("dispatch should load");
    assert_eq!(loaded.declared_memory, memory.initial_bytes());
}

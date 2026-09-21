use fprime_test::abi;
use fprime_test::interpreter;
use fprime_test::interpreter::Limits;

fn assemble(text: &str) -> Vec<u8> {
    wat::parse_str(text).expect("the fixture should be valid WebAssembly text")
}

/// Exports something, just not under the name `WasmSequencerController` invokes.
#[test]
fn module_without_main_export_rejected() {
    let bytes = assemble(
        r#"(module
             (memory 512 (pagesize 1))
             (func (export "nope")))"#,
    );

    let err = interpreter::validate(bytes, &Limits::default())
        .expect_err("a module without `main` cannot be run");

    let message = format!("{err:#}");
    assert!(message.contains(abi::ENTRY_POINT), "{message}");
    assert!(message.contains("fprime_main"), "{message}");
}

/// Names the unresolved import; `spacewasm` itself can't say which one.
#[test]
fn unknown_import_names_itself() {
    let bytes = assemble(
        r#"(module
             (import "fprime_v1" "zzz" (func))
             (memory 512 (pagesize 1))
             (func (export "main")))"#,
    );

    let err = interpreter::validate(bytes, &Limits::default())
        .expect_err("an unresolvable import cannot be run");

    let message = format!("{err:#}");
    assert!(message.contains("fprime_v1.zzz"), "{message}");
    assert!(message.contains("not part of"), "{message}");
}

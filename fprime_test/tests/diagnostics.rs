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

/// The IR map replays the load against `fprime_v1` itself: a host call resolves and maps
/// to the `call` that made it.
#[test]
fn ir_map_replays_against_the_host_interface() {
    let bytes = assemble(
        r#"(module
             (import "fprime_v1" "exit" (func $exit (param i32)))
             (memory 512 (pagesize 1))
             (func (export "main") i32.const 0 call $exit))"#,
    );
    let call = wasmparser::Parser::new(0)
        .parse_all(&bytes)
        .find_map(|payload| match payload.unwrap() {
            wasmparser::Payload::CodeSectionEntry(body) => {
                let mut reader = body.get_operators_reader().unwrap();
                reader.read_with_offset().unwrap();
                Some(reader.read_with_offset().unwrap().1 as u32)
            }
            _ => None,
        })
        .unwrap();

    let map = interpreter::ir_map(&bytes, &Limits::default()).expect("the module loads");
    // `i32.const 0` is one IR word, so the host `call` starts at word 1.
    assert_eq!(map.wasm_offset(1), Some(call));
}

/// F Prime 4.4.0 flies spacewasm 0.7.1, which has no sign-extension operators. This tool's
/// decoder does, so the older one's refusal has to be found rather than left to it.
#[test]
fn an_older_fprime_refuses_what_its_interpreter_lacks() {
    let bytes = assemble(
        r#"(module
             (memory 512 (pagesize 1))
             (func (export "main") i32.const 1 i32.extend8_s drop))"#,
    );
    interpreter::validate(bytes.clone(), &Limits::default()).expect("the newest loads it");

    let limits = Limits {
        fprime: fprime_test::fprime::Fprime::parse("4.4.0").unwrap(),
        ..Limits::default()
    };
    let err = interpreter::validate(bytes, &limits).expect_err("0.7.1 does not");
    let message = format!("{err:#}");
    assert!(message.contains("i32.extend8_s"), "{message}");
    assert!(
        message.contains("F Prime 4.4.0 (spacewasm 0.7.1)"),
        "{message}"
    );
    assert!(message.contains("--fprime-version 4.4.0"), "{message}");
}

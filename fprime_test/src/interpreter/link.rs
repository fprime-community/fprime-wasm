//! Turning a decode failure into something a sequence author can act on.
//!
//! `spacewasm` resolves imports against the host store as it decodes, and reports a
//! mismatch without saying which import it was — it no longer has the name. The
//! static view from [`crate::wasm`] still does, so the module's imports are compared
//! against the ABI here and whatever is actually wrong leads the message.
//!
//! An instruction the interpreter does not implement is named by `spacewasm_tools`, whose
//! list of proposals is the release's own, rather than by a copy of it here.

use crate::abi;
use crate::wasm;
use anyhow::anyhow;

const WASM_OPT_NOTE: &str = "Note that `wasm-opt` introduces these while optimising even when \
                             the compiler did not emit any, so a module can build clean and \
                             fail only at load time.";

/// An instruction the F Prime in `limits` flies an interpreter too old for.
pub(super) fn unsupported_error(
    unsupported: &spacewasm_tools::Invalid,
    limits: &super::Limits,
) -> anyhow::Error {
    anyhow!(
        "{unsupported}\n\n{} flies it. Build with the `-Ctarget-feature` of a fresh \
         `fprime-wasm init --fprime-version {}`, or set `{}` to the F Prime the sequence \
         will run on.\n\n{WASM_OPT_NOTE}",
        limits.fprime,
        limits.fprime.version,
        crate::fprime::ENV,
    )
}

/// Explain a decode failure, naming the import at fault where there is one.
///
/// `unsupported` is `spacewasm_tools::find_unsupported` of the same module.
pub(super) fn link_error(
    err: impl std::fmt::Debug,
    static_view: &wasm::Module,
    unsupported: Option<spacewasm_tools::Invalid>,
) -> anyhow::Error {
    // An unsupported instruction is not a linking problem and has a specific remedy,
    // so it is reported on its own rather than alongside import checks.
    if let Some(unsupported) = unsupported {
        return anyhow!("{unsupported}\n\n{WASM_OPT_NOTE}\n\n(the decoder reported {err:?})");
    }

    let mut problems = Vec::new();

    for import in static_view.imports_outside(abi::MODULE) {
        problems.push(format!(
            "imports {}.{} ({}), but the sequencer registers only `{}`",
            import.module,
            import.name,
            import.kind,
            abi::MODULE
        ));
    }

    for import in static_view.imports_from(abi::MODULE) {
        let wasm::ImportKind::Func(signature) = &import.kind else {
            problems.push(format!(
                "imports {}.{} as a {}, but the sequencer provides only functions",
                import.module, import.name, import.kind
            ));
            continue;
        };
        match abi::function(&import.name) {
            None => problems.push(format!(
                "imports {}.{}, which is not part of the {} interface",
                import.module,
                import.name,
                abi::MODULE
            )),
            Some(expected)
                if expected.params != signature.params || expected.returns != signature.returns =>
            {
                problems.push(format!(
                    "imports {}.{} as {}, but the sequencer registers it as \"{}\" -> \"{}\"",
                    import.module, import.name, signature, expected.params, expected.returns
                ));
            }
            Some(_) => {}
        }
    }

    if problems.is_empty() {
        return anyhow!("the module did not decode or validate: {err:?}");
    }
    anyhow!(
        "the module will not link against the {} host interface:\n{}\n\n(the decoder reported \
         {err:?})",
        abi::MODULE,
        problems
            .iter()
            .map(|problem| format!("  - the module {problem}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import(module: &str, name: &str, params: &str, returns: &str) -> wasm::Import {
        wasm::Import {
            module: module.into(),
            name: name.into(),
            kind: wasm::ImportKind::Func(wasm::Signature {
                params: params.into(),
                returns: returns.into(),
            }),
        }
    }

    fn view(imports: Vec<wasm::Import>) -> wasm::Module {
        wasm::Module {
            imports,
            ..wasm::Module::default()
        }
    }

    #[test]
    fn names_an_import_from_the_wrong_module() {
        let message = link_error(
            "ValidationError",
            &view(vec![import("env", "clock_ms", "", "I")]),
            None,
        )
        .to_string();
        assert!(message.contains("env.clock_ms"), "{message}");
        assert!(message.contains("fprime_v1"), "{message}");
    }

    #[test]
    fn names_an_import_that_is_not_in_the_interface() {
        let message = link_error(
            "ValidationError",
            &view(vec![import("fprime_v1", "telemetry", "iiiii", "i")]),
            None,
        )
        .to_string();
        assert!(message.contains("fprime_v1.telemetry"), "{message}");
        assert!(message.contains("not part of"), "{message}");
    }

    /// The failure mode that motivated this: a guest built against an older ABI whose
    /// `tlm` took an `i32` id. The signature has to be spelled out or the author has
    /// nothing to go on.
    #[test]
    fn names_a_signature_mismatch_with_both_signatures() {
        let message = link_error(
            "FunctionImportTypeMismatch",
            &view(vec![import("fprime_v1", "tlm", "iiiii", "i")]),
            None,
        )
        .to_string();
        assert!(message.contains("fprime_v1.tlm"), "{message}");
        assert!(message.contains("\"iiiii\" -> \"i\""), "{message}");
        assert!(message.contains("\"Iiiii\" -> \"i\""), "{message}");
    }

    /// Imports that all check out mean the failure was something else; the decoder's
    /// own error must still come through rather than being replaced by a confident,
    /// wrong explanation.
    #[test]
    fn falls_back_to_the_decoder_error_when_imports_are_fine() {
        let message = link_error(
            "PossibleBackpatchCycle",
            &view(vec![import("fprime_v1", "cmd", "ii", "i")]),
            None,
        )
        .to_string();
        assert!(message.contains("PossibleBackpatchCycle"), "{message}");
        assert!(!message.contains("will not link"), "{message}");
    }

    /// An unsupported instruction takes priority, but an import problem in the same
    /// module must still be reported when there is no opcode to explain.
    #[test]
    fn an_unrelated_decoder_error_still_falls_through_to_imports() {
        let message = link_error(
            "FunctionImportTypeMismatch",
            &view(vec![import("env", "clock_ms", "", "I")]),
            None,
        )
        .to_string();
        assert!(message.contains("env.clock_ms"), "{message}");
    }

    /// The failure this replaced: `wasm-opt` emitted `memory.fill` and the only diagnostic
    /// was a bare opcode number, which says nothing about what to change.
    #[test]
    fn names_an_unsupported_instruction_and_its_remedy() {
        let module = wat::parse_str(
            r#"(module (memory 1) (func (export "main")
                 i32.const 0 i32.const 0 i32.const 1 memory.fill))"#,
        )
        .unwrap();
        let message = link_error(
            "InvalidFcOpcode(11)",
            &view(vec![]),
            spacewasm_tools::find_unsupported(&module),
        )
        .to_string();
        assert!(message.contains("memory.fill"), "{message}");
        assert!(message.contains("bulk-memory"), "{message}");
        assert!(
            message.contains("-Ctarget-feature=-bulk-memory"),
            "{message}"
        );
        // The cause is worth naming: the compiler did not emit this.
        assert!(message.contains("wasm-opt"), "{message}");
    }

    #[test]
    fn reports_a_non_function_import() {
        let message = link_error(
            "ValidationError",
            &view(vec![wasm::Import {
                module: abi::MODULE.into(),
                name: "memory".into(),
                kind: wasm::ImportKind::Memory,
            }]),
            None,
        )
        .to_string();
        assert!(message.contains("only functions"), "{message}");
    }
}

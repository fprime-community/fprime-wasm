//! Turning a decode failure into something a sequence author can act on.
//!
//! `spacewasm` resolves imports against the host store as it decodes, and reports a
//! mismatch without saying which import it was — it no longer has the name. The
//! static view from [`crate::wasm`] still does, so the module's imports are compared
//! against the ABI here and whatever is actually wrong leads the message.

use crate::abi;
use crate::wasm;
use anyhow::anyhow;

/// A post-MVP instruction `spacewasm` does not implement, named with the proposal it
/// belongs to and the flag that keeps it out of a build.
///
/// `spacewasm` implements WebAssembly 1.0 plus `mutable-globals` and
/// `custom-page-sizes`, and reports anything else as a bare `InvalidOpcode(n)`. That
/// number is not something a sequence author can act on, and the instruction usually
/// is not theirs: `rustc` targets `wasm32v1-none` correctly, and it is `wasm-opt` that
/// introduces these while optimising, so the module builds and fails only at load
/// time.
struct Unsupported {
    instruction: &'static str,
    proposal: &'static str,
    remedy: &'static str,
}

fn unsupported_opcode(opcode: u32) -> Option<Unsupported> {
    let sign_ext = |instruction| {
        Some(Unsupported {
            instruction,
            proposal: "sign-extension-ops",
            remedy: "`-Ctarget-feature=-sign-ext` in rustflags, and `--mvp-features` (plus \
                     `--enable-mutable-globals --enable-custom-page-sizes`) if the module is run \
                     through wasm-opt",
        })
    };
    match opcode {
        0xC0 => sign_ext("i32.extend8_s"),
        0xC1 => sign_ext("i32.extend16_s"),
        0xC2 => sign_ext("i64.extend8_s"),
        0xC3 => sign_ext("i64.extend16_s"),
        0xC4 => sign_ext("i64.extend32_s"),
        0xD0..=0xD2 => Some(Unsupported {
            instruction: "a reference-type instruction",
            proposal: "reference-types",
            remedy: "`-Ctarget-feature=-reference-types` in rustflags",
        }),
        0xFC => Some(Unsupported {
            instruction: "a bulk-memory or saturating-truncation instruction",
            proposal: "bulk-memory-operations / non-trapping float-to-int",
            remedy: "`-Ctarget-feature=-bulk-memory,-nontrapping-fptoint` in rustflags",
        }),
        0xFD => Some(Unsupported {
            instruction: "a SIMD instruction",
            proposal: "fixed-width-simd",
            remedy: "`-Ctarget-feature=-simd128` in rustflags",
        }),
        0xFE => Some(Unsupported {
            instruction: "an atomic instruction",
            proposal: "threads",
            remedy: "`-Ctarget-feature=-atomics` in rustflags",
        }),
        _ => None,
    }
}

/// Pull the opcode out of `InvalidOpcode(n)` in a decoder error's `Debug` form.
///
/// Matching on the text rather than the error type because `spacewasm` nests the
/// opcode inside `ParseError { err: SectionDecodeError { err: InvalidOpcode(n) } }`
/// with private fields, so there is nothing to destructure.
fn invalid_opcode(rendered: &str) -> Option<u32> {
    let rest = rendered.split("InvalidOpcode(").nth(1)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Explain a decode failure, naming the import at fault where there is one.
pub(super) fn link_error(err: impl std::fmt::Debug, static_view: &wasm::Module) -> anyhow::Error {
    let rendered = format!("{err:?}");

    // An unsupported instruction is not a linking problem and has a specific remedy,
    // so it is reported on its own rather than alongside import checks.
    if let Some(opcode) = invalid_opcode(&rendered)
        && let Some(unsupported) = unsupported_opcode(opcode)
    {
        return anyhow!(
            "the module uses {} (opcode {opcode:#04x}), from the {} proposal, which the on-board \
             interpreter does not implement: it is WebAssembly 1.0 plus mutable-globals and \
             custom-page-sizes only.\n\nKeep it out with {}.\n\nNote that `wasm-opt` introduces \
             these while optimising even when the compiler did not emit any, so a module can build \
             clean and fail only at load time.\n\n(the decoder reported {rendered})",
            unsupported.instruction,
            unsupported.proposal,
            unsupported.remedy,
        );
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
        )
        .to_string();
        assert!(message.contains("PossibleBackpatchCycle"), "{message}");
        assert!(!message.contains("will not link"), "{message}");
    }

    /// The real failure this replaced: `wasm-opt` lowered a shift-and-mask to
    /// `i64.extend16_s` and the only diagnostic was `InvalidOpcode(195)`, which says
    /// nothing about what to change.
    #[test]
    fn explains_an_unsupported_instruction_and_how_to_avoid_it() {
        let message = link_error(
            "ParseError { offset: 492, err: SectionDecodeError { section: Some(Code), \
             err: InvalidOpcode(195) } }",
            &view(vec![]),
        )
        .to_string();

        assert!(message.contains("i64.extend16_s"), "{message}");
        assert!(message.contains("sign-extension-ops"), "{message}");
        assert!(message.contains("-sign-ext"), "{message}");
        assert!(message.contains("mvp-features"), "{message}");
        // The cause is worth naming: the compiler did not emit this.
        assert!(message.contains("wasm-opt"), "{message}");
    }

    #[test]
    fn maps_each_unsupported_opcode_family() {
        for (opcode, expected) in [
            (0xC0, "i32.extend8_s"),
            (0xC4, "i64.extend32_s"),
            (0xFD, "SIMD"),
            (0xFC, "bulk-memory"),
            (0xFE, "atomic"),
        ] {
            let found = unsupported_opcode(opcode)
                .unwrap_or_else(|| panic!("{opcode:#04x} should be recognised"));
            assert!(
                found.instruction.contains(expected),
                "{opcode:#04x}: {} does not mention {expected}",
                found.instruction
            );
        }
        // A plain MVP opcode is not a feature problem, and must not be reported as one.
        assert!(unsupported_opcode(0x20).is_none(), "local.get is MVP");
        assert!(unsupported_opcode(0x0B).is_none(), "end is MVP");
    }

    #[test]
    fn extracts_the_opcode_from_a_decoder_error() {
        assert_eq!(
            invalid_opcode("SectionDecodeError { err: InvalidOpcode(195) }"),
            Some(195)
        );
        assert_eq!(invalid_opcode("InvalidOpcode(0)"), Some(0));
        assert_eq!(invalid_opcode("MemoryTooLarge"), None);
        // Malformed rather than absent: must not panic or invent a value.
        assert_eq!(invalid_opcode("InvalidOpcode(abc)"), None);
        assert_eq!(invalid_opcode("InvalidOpcode("), None);
    }

    /// An unsupported instruction takes priority, but an import problem in the same
    /// module must still be reported when there is no opcode to explain.
    #[test]
    fn an_unrelated_decoder_error_still_falls_through_to_imports() {
        let message = link_error(
            "FunctionImportTypeMismatch",
            &view(vec![import("env", "clock_ms", "", "I")]),
        )
        .to_string();
        assert!(message.contains("env.clock_ms"), "{message}");
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
        )
        .to_string();
        assert!(message.contains("only functions"), "{message}");
    }
}

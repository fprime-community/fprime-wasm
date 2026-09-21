use crate::infer_path::test_support::{const_encode_block, render_block, rewrite_block};
use crate::{command, parameter, telemetry};
use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use quote::quote;

/// Compare an expansion against the code it is expected to produce.
fn assert_expands(actual: syn::Result<TokenStream>, expected: TokenStream) {
    let actual = actual.expect("expansion failed");
    assert_eq!(expected.to_string(), actual.to_string())
}

/// The rendered message of an expansion that is expected to fail.
fn error(actual: syn::Result<TokenStream>) -> String {
    actual.expect_err("expansion succeeded").to_string()
}

#[test]
fn command_without_arguments() {
    assert_expands(
        command::command(
            quote! { opcode = 0x1000000 },
            quote! {
                /// No-op command
                pub fn CMD_NO_OP(&self) -> super::Defs::Fw::CmdResponse {}
            },
        ),
        quote! {
            /// No-op command
            pub fn CMD_NO_OP(&self) -> super::Defs::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut __SCRATCH) as *mut u8;
                    let len = __SCRATCH_SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                let mut __offset: usize = 0;
                let __opcode: FwOpcodeType = 0x1000000;
                __opcode.serialize_to(__encoded, &mut __offset);

                unsafe { command(__encoded.get_unchecked(0..__offset)) }
            }
        },
    )
}

#[test]
fn command_serializes_arguments_in_order() {
    assert_expands(
        command::command(
            quote! { opcode = 0x1000002 },
            quote! {
                pub fn CMD_TEST_CMD_1(
                    &self,
                    arg1: i32,
                    arg2: crate::Defs::Ref::DpDemo::DpReqType,
                ) -> super::Defs::Fw::CmdResponse {}
            },
        ),
        quote! {
            pub fn CMD_TEST_CMD_1(
                &self,
                arg1: i32,
                arg2: crate::Defs::Ref::DpDemo::DpReqType,
            ) -> super::Defs::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut __SCRATCH) as *mut u8;
                    let len = __SCRATCH_SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                let mut __offset: usize = 0;
                let __opcode: FwOpcodeType = 0x1000002;
                __opcode.serialize_to(__encoded, &mut __offset);
                arg1.serialize_to(__encoded, &mut __offset);
                arg2.serialize_to(__encoded, &mut __offset);

                unsafe { command(__encoded.get_unchecked(0..__offset)) }
            }
        },
    )
}

/// A `String<N>` argument is the wire type, callers pass a truncated `&str`
#[test]
fn command_truncates_string_arguments() {
    assert_expands(
        command::command(
            quote! { opcode = 0x1000001 },
            quote! {
                pub fn CMD_NO_OP_STRING(&self, arg1: String<40>) -> super::Defs::Fw::CmdResponse {}
            },
        ),
        quote! {
            #[inline(always)]
            pub fn CMD_NO_OP_STRING(&self, arg1: &str) -> super::Defs::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut __SCRATCH) as *mut u8;
                    let len = __SCRATCH_SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                let mut __offset: usize = 0;
                let __opcode: FwOpcodeType = 0x1000001;
                __opcode.serialize_to(__encoded, &mut __offset);
                serialize_str::<40>(arg1, __encoded, &mut __offset);

                unsafe { command(__encoded.get_unchecked(0..__offset)) }
            }
        },
    )
}

/// A modeled `String` type keeps its `crate::Defs::` path, not mistaken for `fprime_core::String<N>`.
#[test]
fn command_passes_modeled_string_by_value() {
    assert_expands(
        command::command(
            quote! { opcode = 0x1 },
            quote! {
                pub fn CMD(&self, arg1: crate::Defs::Fw::String) -> super::Defs::Fw::CmdResponse {}
            },
        ),
        quote! {
            pub fn CMD(&self, arg1: crate::Defs::Fw::String) -> super::Defs::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut __SCRATCH) as *mut u8;
                    let len = __SCRATCH_SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                let mut __offset: usize = 0;
                let __opcode: FwOpcodeType = 0x1;
                __opcode.serialize_to(__encoded, &mut __offset);
                arg1.serialize_to(__encoded, &mut __offset);

                unsafe { command(__encoded.get_unchecked(0..__offset)) }
            }
        },
    )
}

#[test]
fn telemetry_channel() {
    assert_expands(
        telemetry::telemetry(
            quote! { id = 0x1000000 },
            quote! {
                /// Number of commands dispatched
                pub fn CommandsDispatched(&self) -> (u32, super::Defs::Fw::TimeValue) {}
            },
        ),
        quote! {
            /// Number of commands dispatched
            pub fn CommandsDispatched(&self) -> (u32, super::Defs::Fw::TimeValue) {
                const { assert!(<u32 as Serializable>::SIZE <= __SCRATCH_SIZE) };
                const {
                    assert!(<super::Defs::Fw::TimeValue as Serializable>::SIZE <= __TIME_SIZE)
                };

                let __time = unsafe {
                    let ptr = (&raw mut __TIME) as *mut u8;
                    let len = <super::Defs::Fw::TimeValue as Serializable>::SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                let __value = unsafe {
                    let ptr = (&raw mut __SCRATCH) as *mut u8;
                    let len = <u32 as Serializable>::SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                unsafe {
                    telemetry(0x1000000, __time, __value)
                };

                (
                    <u32 as Serializable>::deserialize(__value),
                    <super::Defs::Fw::TimeValue as Serializable>::deserialize(__time)
                )
            }
        },
    )
}

#[test]
fn parameter() {
    assert_expands(
        parameter::parameter(
            quote! { id = 0x10022000 },
            quote! {
                /// A choice
                pub fn CHOICE_PRM(&self) -> crate::Defs::Ref::Choice {}
            },
        ),
        quote! {
            /// A choice
            pub fn CHOICE_PRM(&self) -> crate::Defs::Ref::Choice {
                let mut __value: [u8; <crate::Defs::Ref::Choice as Serializable>::SIZE] = unsafe {
                    #[allow(invalid_value)]
                    core::mem::MaybeUninit::uninit().assume_init()
                };

                unsafe {
                    parameter(0x10022000, &mut __value)
                };

                <crate::Defs::Ref::Choice as Serializable>::deserialize(&__value)
            }
        },
    )
}

#[test]
fn rejects_declared_body() {
    assert_eq!(
        "expected an empty body, the implementation is generated from the signature",
        error(parameter::parameter(
            quote! { id = 0x1 },
            quote! { pub fn PRM(&self) -> u32 { 0 } },
        ))
    )
}

#[test]
fn rejects_missing_return_type() {
    assert_eq!(
        "expected a return type naming the modeled type",
        error(parameter::parameter(
            quote! { id = 0x1 },
            quote! { pub fn PRM(&self) {} },
        ))
    )
}

#[test]
fn rejects_channel_without_time() {
    assert_eq!(
        "expected a `(value, time)` return type",
        error(telemetry::telemetry(
            quote! { id = 0x1 },
            quote! { pub fn CHANNEL(&self) -> u32 {} },
        ))
    )
}

#[test]
fn rejects_missing_identifier() {
    assert_eq!(
        "expected `opcode = <value>`",
        error(command::command(
            quote! {},
            quote! { pub fn CMD(&self) -> super::Defs::Fw::CmdResponse {} },
        ))
    );

    assert_eq!(
        "expected `id`, found `opcode`",
        error(parameter::parameter(
            quote! { opcode = 0x1 },
            quote! { pub fn PRM(&self) -> u32 {} },
        ))
    )
}

/// Apply the DSL to a sequence body.
fn rewrite(body: &str) -> String {
    rewrite_block(body).expect("failed to rewrite body")
}

/// The body the DSL is expected to produce, written as ordinary Rust.
fn expect(body: &str) -> String {
    render_block(body).expect("failed to parse expected body")
}

#[test]
fn qualifies_by_formal_parameter_type() {
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);"),
        expect(
            "Ref.dpDemo.Dp(
                 crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE
             );"
        )
    )
}

/// `DpReqType` is declared by both `Ref.DpDemo` and `Ref.SignalGen`. receiver decides
/// which enum `IMMEDIATE` belongs to.
#[test]
fn same_constant_resolves_per_receiver() {
    assert_eq!(
        rewrite("Ref.SG1.Dp(IMMEDIATE, 0, 1);"),
        expect("Ref.SG1.Dp(crate::Defs::Ref::SignalGen::DpReqType::IMMEDIATE, 0, 1);")
    )
}

/// `ACTIVITY_HI` is shared by two severity enums and `DISABLED` by four
/// `Enabled` style enums; only the parameter type tells them apart.
#[test]
fn qualifies_ambiguous_constants() {
    assert_eq!(
        rewrite("CdhCore.events.SET_EVENT_FILTER(ACTIVITY_HI, DISABLED);"),
        expect(
            "CdhCore.events.SET_EVENT_FILTER(
                 crate::Defs::Svc::EventManager::FilterSeverity::ACTIVITY_HI,
                 crate::Defs::Svc::EventManager::Enabled::DISABLED
             );"
        )
    )
}

/// `RED` and `BLUE` are declared by both `Ref.Choice` and `Ref.DpDemo.ColorEnum`.
#[test]
fn qualifies_array_elements() {
    assert_eq!(
        rewrite("Ref.typeDemo.CHOICES([RED, BLUE]);"),
        expect(
            "Ref.typeDemo.CHOICES([crate::Defs::Ref::Choice::RED, crate::Defs::Ref::Choice::BLUE]);"
        )
    )
}

#[test]
fn qualifies_struct_literal_and_members() {
    assert_eq!(
        rewrite("Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: TWO });"),
        expect(
            "Ref.typeDemo.CHOICE_PAIR(crate::Defs::Ref::ChoicePair {
                 firstChoice: crate::Defs::Ref::Choice::RED,
                 secondChoice: crate::Defs::Ref::Choice::TWO
             });"
        )
    )
}

#[test]
fn qualifies_chained_and_nested_commands() {
    assert_eq!(
        rewrite(
            "Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE).check();
             if ready { Ref.dpDemo.SelectColor(BLUE); }"
        ),
        expect(
            "Ref.dpDemo
                 .Dp(
                     crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE,
                     0,
                     crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE
                 )
                 .check();
             if ready { Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::BLUE); }"
        )
    )
}

/// Qualifying an argument's name must not drop the argument's own attributes (e.g. `#[cfg]`).
#[test]
fn keeps_argument_attributes() {
    assert_eq!(
        rewrite("Ref.dpDemo.SelectColor(#[cfg(feature = \"red\")] RED);"),
        expect(
            "Ref.dpDemo.SelectColor(#[cfg(feature = \"red\")] crate::Defs::Ref::DpDemo::ColorEnum::RED);"
        )
    )
}

/// A name carrying the completion marker must resolve like the name without it, and keep
/// the marker.
#[test]
fn completion_marker_resolves() {
    // A finished constant with the cursor in it: qualified, marker preserved.
    assert_eq!(
        rewrite("Ref.dpDemo.SelectColor(REraCompletionMarkerD);"),
        expect(
            "Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::REraCompletionMarkerD);"
        )
    );

    // Half typed, so not a constant: left alone.
    let half_typed = "Ref.dpDemo.Dp(IMMEDIATE, 2, PROC_raCompletionMarker);";
    assert_eq!(
        rewrite(half_typed),
        expect(
            "Ref.dpDemo.Dp(crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE, 2, PROC_raCompletionMarker);"
        )
    );

    // Nothing typed yet: left alone too.
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(IMMEDIATE, 2, raCompletionMarker);"),
        expect(
            "Ref.dpDemo.Dp(crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE, 2, raCompletionMarker);"
        )
    )
}

/// A call with too few arguments keeps the arguments it does have qualified.
#[test]
fn short_call_qualifies_partially() {
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(IMMEDIATE, 2);"),
        expect("Ref.dpDemo.Dp(crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE, 2);")
    )
}

/// Qualifies around a syntax error (a missing `;`), since the DSL works on tokens rather
/// than a parsed body.
#[test]
fn qualifies_across_syntax_error() {
    assert_eq!(
        rewrite(
            "CdhCore.events.SET_EVENT_FILTER(ACTIVITY_HI, DISABLED)
             Ref.dpDemo.SelectColor(GREEN);"
        ),
        expect(
            "CdhCore.events.SET_EVENT_FILTER(
                 crate::Defs::Svc::EventManager::FilterSeverity::ACTIVITY_HI,
                 crate::Defs::Svc::EventManager::Enabled::DISABLED
             )
             Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::GREEN);"
        )
    )
}

/// A deleted argument leaves its slot empty rather than shifting later arguments onto
/// earlier parameters.
#[test]
fn gap_keeps_argument_positions() {
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(, 0, PROC_TYPE_NONE);"),
        expect("Ref.dpDemo.Dp(, 0, crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE);")
    )
}

/// An argument only resolves if it parses as an expression; two names merged by a deleted
/// comma don't, so both stay unqualified.
#[test]
fn non_expression_argument_untouched() {
    assert_eq!(
        rewrite(
            "Ref.dpDemo.Dp(IMMEDIATE 0, PROC_TYPE_NONE);
             Ref.dpDemo.SelectColor(GREEN);"
        ),
        expect(
            "Ref.dpDemo.Dp(IMMEDIATE 0, PROC_TYPE_NONE);
             Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::GREEN);"
        )
    )
}

/// A command is qualified wherever it sits, including nested in a non-command's argument.
#[test]
fn qualifies_nested_command() {
    assert_eq!(
        rewrite("record(Ref.dpDemo.SelectColor(BLUE));"),
        expect("record(Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::BLUE));")
    )
}

/// A chain merely *ending* in a command's name is not that command (the shape a trailing
/// `.` leaves behind).
#[test]
fn chain_ending_in_command_untouched() {
    let absorbed = "Ref.Ref.dpDemo.SelectColor(GREEN);";
    assert_eq!(rewrite(absorbed), expect(absorbed));

    // Nor a command's name with something in front of it.
    let extended = "topology.Ref.dpDemo.SelectColor(GREEN);";
    assert_eq!(rewrite(extended), expect(extended));
}

/// The instance may be written as a qualified path, of which the dictionary knows
/// only the final segment.
#[test]
fn qualifies_through_qualified_instance() {
    assert_eq!(
        rewrite("crate::Ref.dpDemo.SelectColor(BLUE);"),
        expect("crate::Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::BLUE);")
    )
}

/// Everything that isn't a command argument (attributes, visibility, signature) passes
/// through unchanged.
#[test]
fn leaves_non_argument_tokens_alone() {
    assert_eq!(
        rewrite(
            "#[inline]
             pub fn sequence(retries: u32) -> u32 {
                 Ref.dpDemo.SelectColor(BLUE);
                 retries
             }"
        ),
        expect(
            "#[inline]
             pub fn sequence(retries: u32) -> u32 {
                 Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::BLUE);
                 retries
             }"
        )
    )
}

/// The completion marker must survive even when the body doesn't parse.
#[test]
fn completion_marker_resolves_in_broken_body() {
    assert_eq!(
        rewrite(
            "Ref.dpDemo.SelectColor(BLraCompletionMarkerUE)
             Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);"
        ),
        expect(
            "Ref.dpDemo.SelectColor(crate::Defs::Ref::DpDemo::ColorEnum::BLraCompletionMarkerUE)
             Ref.dpDemo.Dp(
                 crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE
             );"
        )
    )
}

#[test]
fn passes_through_unknown_names() {
    // A constant of the user's own in an enum slot, a local in a numeric slot
    // and an already qualified path
    let untouched = "Ref.dpDemo.Dp(MY_REQ_TYPE, priority, ProcType::PROC_TYPE_NONE);";
    assert_eq!(rewrite(untouched), expect(untouched));

    // An instance the dictionary does not know
    let unknown_command = "Ref.notAThing.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);";
    assert_eq!(rewrite(unknown_command), expect(unknown_command));

    // Outside a command argument there is no expected type to resolve against
    let outside_a_command = "let request = IMMEDIATE;";
    assert_eq!(rewrite(outside_a_command), expect(outside_a_command));

    // Non enum parameters are never rewritten
    let string_argument = "Ref.wasmSeq.LOAD(\"helloworld\");";
    assert_eq!(rewrite(string_argument), expect(string_argument));
}

fn const_encode(body: &str) -> String {
    const_encode_block(body).expect("failed to rewrite body")
}

/// The `const` buffer a const-encoded call is expected to expand to.
fn encoded(command: &str, args: &str) -> String {
    let parse =
        |source: String| syn::parse_str::<TokenStream>(&source).expect("expectation should parse");

    let path = command.replace('.', "::");
    let size = parse(format!("crate::Konst::{path}__size"));
    let encode = parse(format!("crate::Konst::{path}__encode"));
    let args = parse(args.to_string());

    // Kept so an editor can still resolve the receiver and command name — see
    // `keeps_the_written_call_for_navigation`.
    let original = parse(command.to_string());

    quote! {
        {
            const __FPRIME_LEN: usize = #size(#args);
            const __FPRIME_CMD: [u8; __FPRIME_LEN] = #encode::<__FPRIME_LEN>(#args);

            if false {
                #original(#args);
            }

            unsafe { fprime_core::command(&__FPRIME_CMD) }
        }
    }
    .to_string()
}

#[test]
fn const_encodes_call_with_no_arguments() {
    assert_eq!(
        const_encode("CdhCore.cmdDisp.CMD_NO_OP();"),
        format!("{} ;", encoded("CdhCore.cmdDisp.CMD_NO_OP", ""))
    )
}

/// The enumerated constants are qualified by the first pass, and it is that
/// qualification which makes them recognisable as constants by the second.
#[test]
fn const_encodes_literals_and_enumerated_constants() {
    assert_eq!(
        const_encode("Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);"),
        format!(
            "{} ;",
            encoded(
                "Ref.dpDemo.Dp",
                "crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE"
            )
        )
    )
}

#[test]
fn const_encodes_string_literal() {
    assert_eq!(
        const_encode(r#"CdhCore.cmdDisp.CMD_NO_OP_STRING("STRINGS");"#),
        format!(
            "{} ;",
            encoded("CdhCore.cmdDisp.CMD_NO_OP_STRING", r#""STRINGS""#)
        )
    )
}

/// `-1` is a unary negation, not a literal, but is still a compile-time constant.
#[test]
fn const_encodes_negative_literal() {
    assert_eq!(
        const_encode("CdhCore.cmdDisp.CMD_TEST_CMD_1(-1, -2.5, 3);"),
        format!(
            "{} ;",
            encoded("CdhCore.cmdDisp.CMD_TEST_CMD_1", "-1, -2.5, 3")
        )
    )
}

/// A bare identifier must not be accepted as constant — that would move a local into a
/// `const` initialiser.
#[test]
fn runtime_argument_left_for_accessor() {
    let body = r#"CdhCore.cmdDisp.CMD_NO_OP_STRING(name);"#;
    assert_eq!(const_encode(body), expect(body));

    let body = "Ref.dpDemo.SelectColor(chosen);";
    assert_eq!(const_encode(body), expect(body));
}

/// A bare local name, even of the right enum type, stays unqualified, on the accessor.
#[test]
fn computed_argument_left_for_accessor() {
    let body = "Ref.dpDemo.Dp(IMMEDIATE, count + 1, PROC_TYPE_NONE);";
    assert_eq!(
        const_encode(body),
        expect(
            "Ref.dpDemo.Dp(
                 crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE,
                 count + 1,
                 crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE
             );"
        )
    )
}

/// Arity is the accessor's to complain about, so a wrong-arity call is not rewritten.
#[test]
fn wrong_arity_left_for_accessor() {
    let body = "CdhCore.cmdDisp.CMD_NO_OP(1);";
    assert_eq!(const_encode(body), expect(body));
}

/// Something that is not a command at all is untouched by the second pass.
#[test]
fn non_command_left_alone() {
    let body = "Ref.notAThing.Dp(1, 2, 3);";
    assert_eq!(const_encode(body), expect(body));
}

/// A struct argument is const-encodable too — the generated encoder descends its members
/// itself.
#[test]
fn const_encodes_struct_argument() {
    let arg = "crate::Defs::Ref::ChoicePair {
         firstChoice: crate::Defs::Ref::Choice::RED,
         secondChoice: crate::Defs::Ref::Choice::BLUE
     }";

    assert_eq!(
        const_encode(
            "Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: BLUE });"
        ),
        format!("{} ;", encoded("Ref.typeDemo.CHOICE_PAIR", arg))
    )
}

#[test]
fn const_encodes_array_argument() {
    let arg = "[crate::Defs::Ref::Choice::ONE, crate::Defs::Ref::Choice::TWO]";

    assert_eq!(
        const_encode("Ref.typeDemo.CHOICES([ONE, TWO]);"),
        format!("{} ;", encoded("Ref.typeDemo.CHOICES", arg))
    )
}

/// Nested arrays, a nested struct and a member array in one argument.
#[test]
fn const_encodes_deeply_nested_struct() {
    let body = "Ref.typeDemo.GLUTTON_OF_CHOICE(ChoiceSlurry {
             tooManyChoices: [[BLUE, RED], [TWO, TWO]],
             choiceAsMemberArray: [2, 3],
             choicePair: ChoicePair { firstChoice: RED, secondChoice: BLUE },
             separateChoice: ONE,
         });";

    assert!(
        const_encode(body).contains("GLUTTON_OF_CHOICE__encode"),
        "expected the nested struct to be const-encoded, got: {}",
        const_encode(body)
    )
}

/// `[x; N]` is qualified like an array literal, so it is accepted when the
/// repeat count matches the dictionary's array size.
#[test]
fn const_encodes_repeated_array() {
    let arg = "[crate::Defs::Ref::Choice::ONE ; 2]";

    assert_eq!(
        const_encode("Ref.typeDemo.CHOICES([ONE; 2]);"),
        format!("{} ;", encoded("Ref.typeDemo.CHOICES", arg))
    )
}

/// An array literal of the wrong length would not compile as the argument, so it
/// is left for the accessor to reject rather than turned into a `const`.
#[test]
fn wrong_length_array_left_for_accessor() {
    let body = "Ref.typeDemo.CHOICES([ONE, TWO, ONE]);";
    assert!(
        !const_encode(body).contains("__encode"),
        "expected fallback, got: {}",
        const_encode(body)
    )
}

/// A struct literal missing a member, or completing itself from elsewhere, is
/// not something we can evaluate.
#[test]
fn partial_struct_left_for_accessor() {
    let body = "Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, ..other });";
    assert!(
        !const_encode(body).contains("__encode"),
        "expected fallback, got: {}",
        const_encode(body)
    )
}

/// A runtime value anywhere inside an aggregate keeps the whole call on the accessor.
#[test]
fn runtime_member_left_for_accessor() {
    let body = "Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: chosen });";
    assert!(
        !const_encode(body).contains("__encode"),
        "expected fallback, got: {}",
        const_encode(body)
    )
}

/// Regression guard: the written call must survive in an unreachable branch so
/// goto-definition still resolves it (see `tools/goto_probe.py`).
#[test]
fn keeps_the_written_call_for_navigation() {
    let expansion = const_encode("Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);");

    assert!(
        expansion.contains("if false"),
        "the written call should survive in an unreachable branch: {expansion}"
    );

    // A `Konst` path gives an editor nothing to resolve `dpDemo` to.
    for token in ["Ref", "dpDemo", "Dp"] {
        assert!(
            expansion.contains(&format!("{token} ")) || expansion.contains(&format!("{token}.")),
            "`{token}` should appear in the expansion: {expansion}"
        );
    }
}

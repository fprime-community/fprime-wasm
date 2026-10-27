use crate::infer_path::test_support::{
    const_encode_block, descriptor_block, render_block, rewrite_block,
    test_body as expand_test_body,
};
use crate::{command, parameter, profile, telemetry};
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
                pub fn CMD_NO_OP(&self) -> super::Dict::Fw::CmdResponse {}
            },
        ),
        quote! {
            /// No-op command
            pub fn CMD_NO_OP(&self) -> super::Dict::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                    let len = super::__SCRATCH_SIZE;

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
                    arg2: crate::Dict::Ref::DpDemo::DpReqType,
                ) -> super::Dict::Fw::CmdResponse {}
            },
        ),
        quote! {
            pub fn CMD_TEST_CMD_1(
                &self,
                arg1: i32,
                arg2: crate::Dict::Ref::DpDemo::DpReqType,
            ) -> super::Dict::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                    let len = super::__SCRATCH_SIZE;

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
                pub fn CMD_NO_OP_STRING(&self, arg1: String<40>) -> super::Dict::Fw::CmdResponse {}
            },
        ),
        quote! {
            #[inline(always)]
            pub fn CMD_NO_OP_STRING(&self, arg1: &str) -> super::Dict::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                    let len = super::__SCRATCH_SIZE;

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

/// A modeled `String` type keeps its `crate::Dict::` path, not mistaken for `fprime_core::String<N>`.
#[test]
fn command_passes_modeled_string_by_value() {
    assert_expands(
        command::command(
            quote! { opcode = 0x1 },
            quote! {
                pub fn CMD(&self, arg1: crate::Dict::Fw::String) -> super::Dict::Fw::CmdResponse {}
            },
        ),
        quote! {
            pub fn CMD(&self, arg1: crate::Dict::Fw::String) -> super::Dict::Fw::CmdResponse {
                let __encoded = unsafe {
                    let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                    let len = super::__SCRATCH_SIZE;

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
                pub fn CommandsDispatched(&self) -> (u32, super::Dict::Fw::TimeValue) {}
            },
        ),
        quote! {
            /// Number of commands dispatched
            pub fn CommandsDispatched(&self) -> (u32, super::Dict::Fw::TimeValue) {
                const { assert!(<u32 as Serializable>::SIZE <= super::__SCRATCH_SIZE) };
                const {
                    assert!(<super::Dict::Fw::TimeValue as Serializable>::SIZE <= super::__TIME_SIZE)
                };

                let __time = unsafe {
                    let ptr = (&raw mut super::__TIME) as *mut u8;
                    let len = <super::Dict::Fw::TimeValue as Serializable>::SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                let __value = unsafe {
                    let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                    let len = <u32 as Serializable>::SIZE;

                    core::slice::from_raw_parts_mut(ptr, len)
                };

                unsafe {
                    telemetry(0x1000000, __time, __value)
                };

                (
                    <u32 as Serializable>::deserialize(__value),
                    <super::Dict::Fw::TimeValue as Serializable>::deserialize(__time)
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
                pub fn CHOICE_PRM(&self) -> crate::Dict::Ref::Choice {}
            },
        ),
        quote! {
            /// A choice
            pub fn CHOICE_PRM(&self) -> crate::Dict::Ref::Choice {
                let mut __value: [u8; <crate::Dict::Ref::Choice as Serializable>::SIZE] = unsafe {
                    #[allow(invalid_value)]
                    core::mem::MaybeUninit::uninit().assume_init()
                };

                unsafe {
                    parameter(0x10022000, &mut __value)
                };

                <crate::Dict::Ref::Choice as Serializable>::deserialize(&__value)
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
            quote! { pub fn CMD(&self) -> super::Dict::Fw::CmdResponse {} },
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
                 crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE
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
        expect("Ref.SG1.Dp(crate::Dict::Ref::SignalGen::DpReqType::IMMEDIATE, 0, 1);")
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
                 crate::Dict::Svc::EventManager::FilterSeverity::ACTIVITY_HI,
                 crate::Dict::Svc::EventManager::Enabled::DISABLED
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
            "Ref.typeDemo.CHOICES([crate::Dict::Ref::Choice::RED, crate::Dict::Ref::Choice::BLUE]);"
        )
    )
}

#[test]
fn qualifies_struct_literal_and_members() {
    assert_eq!(
        rewrite("Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: TWO });"),
        expect(
            "Ref.typeDemo.CHOICE_PAIR(crate::Dict::Ref::ChoicePair {
                 firstChoice: crate::Dict::Ref::Choice::RED,
                 secondChoice: crate::Dict::Ref::Choice::TWO
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
                     crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                     0,
                     crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE
                 )
                 .check();
             if ready { Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::BLUE); }"
        )
    )
}

/// Qualifying an argument's name must not drop the argument's own attributes (e.g. `#[cfg]`).
#[test]
fn keeps_argument_attributes() {
    assert_eq!(
        rewrite("Ref.dpDemo.SelectColor(#[cfg(feature = \"red\")] RED);"),
        expect(
            "Ref.dpDemo.SelectColor(#[cfg(feature = \"red\")] crate::Dict::Ref::DpDemo::ColorEnum::RED);"
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
            "Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::REraCompletionMarkerD);"
        )
    );

    // Half typed, so not a constant: the enum's constants are put in scope for the editor to
    // offer, marker preserved.
    let half_typed = "Ref.dpDemo.Dp(IMMEDIATE, 2, PROC_raCompletionMarker);";
    assert_eq!(
        rewrite(half_typed),
        expect(
            "Ref.dpDemo.Dp(crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE, 2, {
                 #[allow(unused_imports)]
                 use crate::Dict::Fw::DpCfg::ProcType:: * ;
                 PROC_raCompletionMarker
             });"
        )
    );

    // Nothing typed yet: left alone too.
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(IMMEDIATE, 2, raCompletionMarker);"),
        expect(
            "Ref.dpDemo.Dp(crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE, 2, raCompletionMarker);"
        )
    )
}

/// A call with too few arguments keeps the arguments it does have qualified.
#[test]
fn short_call_qualifies_partially() {
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(IMMEDIATE, 2);"),
        expect("Ref.dpDemo.Dp(crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE, 2);")
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
                 crate::Dict::Svc::EventManager::FilterSeverity::ACTIVITY_HI,
                 crate::Dict::Svc::EventManager::Enabled::DISABLED
             )
             Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::GREEN);"
        )
    )
}

/// A deleted argument leaves its slot empty rather than shifting later arguments onto
/// earlier parameters.
#[test]
fn gap_keeps_argument_positions() {
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(, 0, PROC_TYPE_NONE);"),
        expect("Ref.dpDemo.Dp(, 0, crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE);")
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
             Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::GREEN);"
        )
    )
}

/// A command is qualified wherever it sits, including nested in a non-command's argument.
#[test]
fn qualifies_nested_command() {
    assert_eq!(
        rewrite("record(Ref.dpDemo.SelectColor(BLUE));"),
        expect("record(Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::BLUE));")
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
        expect("crate::Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::BLUE);")
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
                 Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::BLUE);
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
            "Ref.dpDemo.SelectColor(crate::Dict::Ref::DpDemo::ColorEnum::BLraCompletionMarkerUE)
             Ref.dpDemo.Dp(
                 crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE
             );"
        )
    )
}

#[test]
fn passes_through_unknown_names() {
    // A local in a numeric slot, and the fully qualified path this pass writes itself —
    // longer than the dictionary's own name for the constant, so never a name it resolves.
    let untouched = "Ref.dpDemo.Dp(
         IMMEDIATE,
         priority,
         crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE
     );";
    assert_eq!(
        rewrite(untouched),
        expect(
            "Ref.dpDemo.Dp(
                 crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                 priority,
                 crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE
             );"
        )
    );

    // A qualifier that is not the enum's own names no constant at all.
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(IMMEDIATE, 0, Other::ProcType::PROC_TYPE_NONE);"),
        expect(
            "Ref.dpDemo.Dp(
                 crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 Other::ProcType::PROC_TYPE_NONE
             );"
        )
    );

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

/// A bare name in an enum slot that is none of its constants — a misspelling, a local, a
/// constant of the author's — is wrapped in the enum's scope rather than passed through, so a
/// misspelling earns rustc's similar-name hint into the enum. A qualified path is not wrapped,
/// and an attribute stays on the argument.
#[test]
fn scopes_an_unknown_name_to_its_enum() {
    assert_eq!(
        rewrite("Ref.dpDemo.Dp(MY_REQ_TYPE, 0, #[cfg(all())] PROC_TYPE_N);"),
        expect(
            "Ref.dpDemo.Dp(
                 {
                     #[allow(unused_imports)]
                     use crate::Dict::Ref::DpDemo::DpReqType:: * ;
                     MY_REQ_TYPE
                 },
                 0,
                 #[cfg(all())] {
                     #[allow(unused_imports)]
                     use crate::Dict::Fw::DpCfg::ProcType:: * ;
                     PROC_TYPE_N
                 }
             );"
        )
    );

    let qualified = "Ref.dpDemo.Dp(IMMEDIATE, 0, ProcType::PROC_TYPE_N);";
    assert_eq!(
        rewrite(qualified),
        expect(
            "Ref.dpDemo.Dp(crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE, 0, ProcType::PROC_TYPE_N);"
        )
    );
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
                "crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                 0,
                 crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE"
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

    // Wrapped in the enum's scope, which is a block and so never a constant.
    assert_eq!(
        const_encode("Ref.dpDemo.SelectColor(chosen);"),
        expect(
            "Ref.dpDemo.SelectColor({
                 #[allow(unused_imports)]
                 use crate::Dict::Ref::DpDemo::ColorEnum:: * ;
                 chosen
             });"
        )
    );
}

/// A bare local name, even of the right enum type, stays unqualified, on the accessor.
#[test]
fn computed_argument_left_for_accessor() {
    let body = "Ref.dpDemo.Dp(IMMEDIATE, count + 1, PROC_TYPE_NONE);";
    assert_eq!(
        const_encode(body),
        expect(
            "Ref.dpDemo.Dp(
                 crate::Dict::Ref::DpDemo::DpReqType::IMMEDIATE,
                 count + 1,
                 crate::Dict::Fw::DpCfg::ProcType::PROC_TYPE_NONE
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
    let arg = "crate::Dict::Ref::ChoicePair {
         firstChoice: crate::Dict::Ref::Choice::RED,
         secondChoice: crate::Dict::Ref::Choice::BLUE
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
    let arg = "[crate::Dict::Ref::Choice::ONE, crate::Dict::Ref::Choice::TWO]";

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
    let arg = "[crate::Dict::Ref::Choice::ONE ; 2]";

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

/// A struct or enum name may carry as much of its dictionary qualifier as the author cares
/// to write, and every spelling has to mean the one name — otherwise `Svc::SeqArgs { .. }`,
/// which is how the dictionary itself spells the type, resolves to nothing.
#[test]
fn qualifier_may_be_written_out_in_part() {
    let qualified = expect(
        "Ref.typeDemo.CHOICE_PAIR(crate::Dict::Ref::ChoicePair {
             firstChoice: crate::Dict::Ref::Choice::RED,
             secondChoice: crate::Dict::Ref::Choice::TWO
         });",
    );

    for written in [
        "ChoicePair { firstChoice: RED, secondChoice: TWO }",
        "Ref::ChoicePair { firstChoice: Choice::RED, secondChoice: TWO }",
        "Ref::ChoicePair { firstChoice: Ref::Choice::RED, secondChoice: Ref::Choice::TWO }",
    ] {
        assert_eq!(
            rewrite(&format!("Ref.typeDemo.CHOICE_PAIR({written});")),
            qualified,
            "`{written}` should name the same struct"
        )
    }
}

/// And each of those spellings const-encodes, which is what a test needs to match bytes.
#[test]
fn const_encodes_a_partly_qualified_struct() {
    let arg = "crate::Dict::Ref::ChoicePair {
         firstChoice: crate::Dict::Ref::Choice::RED,
         secondChoice: crate::Dict::Ref::Choice::BLUE
     }";

    assert_eq!(
        const_encode(
            "Ref.typeDemo.CHOICE_PAIR(Ref::ChoicePair {
                 firstChoice: Choice::RED,
                 secondChoice: Ref::Choice::BLUE
             });"
        ),
        format!("{} ;", encoded("Ref.typeDemo.CHOICE_PAIR", arg))
    )
}

/// The descriptor pass, as `#[fprime_test]` runs it.
fn descriptor(body: &str) -> String {
    descriptor_block(body, false).expect("failed to rewrite body")
}

/// The whole `#[fprime_test]` pipeline, marker detection included.
fn test_body(body: &str) -> String {
    expand_test_body(body).expect("failed to rewrite body")
}

/// A test that wrote arguments asked to match their exact bytes. When there are none to
/// compute, saying so beats falling back on the flight accessor, which reports
/// `CmdResponse: IntoCommand` — about neither the argument nor the command.
#[test]
fn a_runtime_argument_is_a_test_error() {
    let expansion = descriptor("t.expect_command(CdhCore.cmdDisp.CMD_NO_OP_STRING(name));");

    assert!(
        expansion.contains("compile_error"),
        "expected an error, got: {expansion}"
    );
    // Names the argument, says what it has to be, and offers the two ways to match without
    // it.
    for expected in [
        "arg1",
        "written out",
        "leave the parentheses off",
        "args_starting",
    ] {
        assert!(
            expansion.contains(expected),
            "the message should mention {expected:?}: {expansion}"
        )
    }
    // Still a `CmdMatch` on the opcode, so the error above is the only one reported.
    assert!(
        expansion.contains("fprime_test :: cmd"),
        "expected a recovery value, got: {expansion}"
    );
}

/// The reason is about the offending member, not just the whole argument.
#[test]
fn a_runtime_member_names_itself() {
    let expansion = descriptor(
        "t.expect_command(Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: chosen }));",
    );

    assert!(
        expansion.contains("Ref.Choice's constants"),
        "the message should name the member's enum: {expansion}"
    )
}

/// `[0; u8]`, where the length was reached for and not written.
#[test]
fn a_non_literal_array_length_says_what_it_should_be() {
    let expansion = descriptor("t.expect_command(Ref.typeDemo.CHOICES([ONE; count]));");

    assert!(
        expansion.contains("the length here has to be 2"),
        "the message should give the dictionary's length: {expansion}"
    )
}

/// Arity is the accessor's to complain about in a sequence, but a test has no accessor to
/// fall back on.
#[test]
fn wrong_arity_is_a_test_error() {
    let expansion = descriptor("t.expect_command(Ref.dpDemo.Dp(IMMEDIATE, 0));");

    assert!(
        expansion.contains("takes 3 arguments, but 2 were written"),
        "expected an arity error, got: {expansion}"
    )
}

/// A const-encodable call is a descriptor, not an error — the ordinary case.
#[test]
fn a_constant_call_is_a_descriptor() {
    let expansion = descriptor("t.expect_command(Ref.wasmSeq.LOAD(\"helloworld\"));");

    assert!(
        !expansion.contains("compile_error"),
        "expected a descriptor, got: {expansion}"
    );
    assert!(
        expansion.contains("crate :: Desc :: Ref :: wasmSeq :: LOAD"),
        "expected a descriptor, got: {expansion}"
    );
}

/// An editor expands this on every keystroke. A half-written argument is not a mistake to
/// report, and a `compile_error!` ahead of the cursor would move the tokens the editor maps
/// its position through — see `spanned_desc_path`.
#[test]
fn nothing_is_reported_while_completing() {
    let body = "t.expect_command(Ref.dpDemo.SelectColor(BLraCompletionMarkerUE));";
    let expansion = descriptor_block(body, true).expect("failed to rewrite body");

    assert!(
        !expansion.contains("compile_error"),
        "a half-written body should be left alone, got: {expansion}"
    );
    assert!(
        expansion.contains("BLraCompletionMarkerUE"),
        "the marker has to survive: {expansion}"
    );
}

/// rust-analyzer's own sentinel, which it puts where the cursor is.
const MARKER: &str = "raCompletionMarker";

/// `body` with the marker inserted `prefix` characters into `name` — what rust-analyzer
/// hands the macro on the keystroke after the cursor lands there.
fn at_cursor(body: &str, name: &str, prefix: &str) -> String {
    let typed = format!("{prefix}{MARKER}{}", &name[prefix.len()..]);
    body.replacen(name, &typed, 1)
}

/// Whether the two expansions say the same thing on every byte before the cursor.
fn agrees_before_the_cursor(what: &str, with: &str, without: &str) -> bool {
    let cut = with.find(MARKER).unwrap_or_else(|| {
        panic!(
            "{what}: the marker did not survive into the expansion, so rust-analyzer has \
             nothing at the cursor to map through:\n{with}"
        )
    });

    without.get(..cut) == Some(&with[..cut])
}

fn agrees_up_to_the_cursor(what: &str, with: &str, without: &str) {
    assert!(
        agrees_before_the_cursor(what, with, without),
        "{what}: the two expansions diverge before the cursor, so rust-analyzer maps the \
         position through to the wrong place.\n  with the marker: {with}\n  without it:      \
         {without}"
    );
}

/// rust-analyzer expands a macro twice on every keystroke — once with `raCompletionMarker`
/// where the cursor is, once without — and then finds the cursor in the real expansion at
/// the raw byte offset the marker sits at in the fake one. So the two have to agree up to
/// the cursor; after it they may differ freely.
///
/// `tools/completion_probe.py` measures what the editor then does with them, which needs
/// the editor. This measures the agreement itself, which does not.
#[test]
fn expansions_agree_up_to_the_cursor() {
    // A sequence body, through both of the passes `#[fprime]` runs.
    for (body, name, prefix) in [
        ("Ref.dpDemo.SelectColor(BLUE);", "BLUE", "BL"),
        (
            "Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);",
            "PROC_TYPE_NONE",
            "PROC_TYPE_N",
        ),
        (
            "Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: TWO });",
            "TWO",
            "T",
        ),
    ] {
        let typed = at_cursor(body, name, prefix);
        agrees_up_to_the_cursor(&format!("qualify {name}"), &rewrite(&typed), &rewrite(body));
        agrees_up_to_the_cursor(
            &format!("const-encode {name}"),
            &const_encode(&typed),
            &const_encode(body),
        );
    }

    // A chain segment, not an argument: the receiver, component or point name itself. These
    // only agree because the scan looks a name up by what has been typed so far.
    for (body, name, prefix) in [
        (
            "CdhCore.cmdDisp.CMD_NO_OP_STRING(\"STRINGS\");",
            "CMD_NO_OP_STRING",
            "CMD_NO_OP_S",
        ),
        ("Ref.dpDemo.SelectColor(BLUE);", "dpDemo", "dpD"),
    ] {
        let typed = at_cursor(body, name, prefix);
        agrees_up_to_the_cursor(
            &format!("const-encode {name}"),
            &const_encode(&typed),
            &const_encode(body),
        );
    }

    // And a test body, through the descriptor pass. `test_body` detects the marker itself,
    // as the macro does, so the lenient and the reporting paths are both reached.
    for (body, name, prefix) in [
        (
            "t.expect_command(Ref.dpDemo.SelectColor(BLUE));",
            "BLUE",
            "BL",
        ),
        // An argument that is not a constant: the reporting path, where the expansion the
        // author is mid-keystroke on and the one that carries the diagnostic differ most.
        (
            "t.expect_command(Ref.wasmSeq.LOAD(filename));",
            "filename",
            "file",
        ),
        // A bare point, which has no `if false { .. }` copy of the author's tokens — so the
        // `Desc` path this emits is the only thing the cursor can land in.
        ("t.never_command(Ref.wasmSeq.LOAD);", "LOAD", "LO"),
        (
            "t.initial_telemetry(CdhCore.events.EventsDropped, 5);",
            "EventsDropped",
            "Events",
        ),
    ] {
        let typed = at_cursor(body, name, prefix);
        agrees_up_to_the_cursor(
            &format!("descriptor {name}"),
            &test_body(&typed),
            &test_body(body),
        );
    }
}

/// The example crate's `restrict` profile, which is what these tests hold to.
const RESTRICT_PROFILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/example/profile_restrict.toml"
);

/// The restrict-mode errors for a sequence body, one per rejected construct.
fn restrict_errors(body: TokenStream) -> Vec<String> {
    let profile = profile::load(std::path::Path::new(RESTRICT_PROFILE)).unwrap();
    let findings = profile::check(&profile, &quote! { fn main() { #body } });
    assert!(findings.warnings.is_empty(), "restrict never warns");

    findings.errors.iter().map(|err| err.to_string()).collect()
}

/// Assert restrict mode accepts `body`.
#[track_caller]
fn restrict_accepts(body: TokenStream) {
    assert_eq!(
        Vec::<String>::new(),
        restrict_errors(body.clone()),
        "in `{body}`"
    );
}

/// Assert restrict mode rejects `body` with exactly `message`.
#[track_caller]
fn restrict_rejects(body: TokenStream, message: &str) {
    assert_eq!(vec![message], restrict_errors(body.clone()), "in `{body}`");
}

#[test]
fn restrict_accepts_values() {
    restrict_accepts(quote! { const LIMIT: f32 = 21.5; });
    restrict_accepts(quote! { let voltage = Tlm::Ref.power.BatteryVoltage().0; });
    restrict_accepts(quote! { let mut tries = 0; });
    restrict_accepts(quote! { tries += 1; });
    restrict_accepts(quote! { let x = !(tries > 3) && -voltage < LIMIT; });
    restrict_accepts(quote! { let x = (voltage as u32, [0u8; 4], [1, 2], ARRAY[0]); });
}

#[test]
fn restrict_accepts_commands() {
    restrict_accepts(quote! { Ref.power.PWR_OFF(Ref::Power::State::OFF, -1); });
    restrict_accepts(quote! {
        Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED, secondChoice: BLUE });
    });
}

#[test]
fn restrict_accepts_if() {
    restrict_accepts(quote! { if ok { return; } else { Ref.cmd.NO_OP(); } });
    restrict_accepts(quote! { if let Some(x) = OPTION { Ref.cmd.SEND(x); } });
}

#[test]
fn restrict_rejects_loops() {
    let message = "loops are not allowed in restrict mode";

    restrict_rejects(quote! { loop {} }, message);
    restrict_rejects(quote! { while ok {} }, message);
    restrict_rejects(quote! { for i in 0..3 { Ref.cmd.NO_OP(); } }, message);
}

#[test]
fn restrict_rejects_break_and_continue() {
    // Labelled blocks allow `break` without a loop.
    restrict_rejects(
        quote! { 'outer: { break 'outer; } },
        "`break` and `continue` are not allowed in restrict mode",
    );
}

#[test]
fn restrict_rejects_references() {
    restrict_rejects(
        quote! { let a = &x; },
        "references (`&`) are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { let b = *a; },
        "dereferencing with `*` is not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { let &c = x; },
        "references (`&`) are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { let ref d = x; },
        "references (`ref`) are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { let e: &u32 = y; },
        "reference types are not allowed in restrict mode",
    );
}

#[test]
fn restrict_rejects_macros() {
    let message = "macros are not allowed in restrict mode";

    restrict_rejects(quote! { println!("hi"); }, message);
    restrict_rejects(quote! { let c = vec![1]; }, message);
}

#[test]
fn restrict_rejects_other_constructs() {
    restrict_rejects(
        quote! { let f = || 1; },
        "closures are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { match x { _ => {} } },
        "`match` is not allowed, use `if` instead in restrict mode",
    );
    restrict_rejects(
        quote! { unsafe { foo() }; },
        "`unsafe` is not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { let g = h()?; },
        "`?` is not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { fn helper() {} },
        "only `const` may be declared inside a sequence in restrict mode",
    );
}

/// Constructs restrict mode rejects are still rejected when nested in ones it accepts.
#[test]
fn restrict_checks_nested_expressions() {
    restrict_rejects(
        quote! { Ref.cmd.SEND(&x); },
        "references (`&`) are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { if ok { let f = |y| y; } },
        "closures are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { const N: u32 = { while true {} 0 }; },
        "loops are not allowed in restrict mode",
    );
    restrict_rejects(
        quote! { let x = -{ loop {} }; },
        "loops are not allowed in restrict mode",
    );
}

/// Every rejected construct is reported, not just the first.
#[test]
fn restrict_reports_every_error() {
    assert_eq!(2, restrict_errors(quote! { loop {} let a = &x; }).len());
}

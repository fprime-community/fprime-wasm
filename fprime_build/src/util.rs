use crate::tree::Qualifier;
use fprime_dictionary::naming::split_qualified_name;
use fprime_dictionary::{Command, Dictionary, FloatKind, IntegerKind, TypeDefinition, TypeName};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use std::str::FromStr;

pub(crate) use fprime_dictionary::naming::str_to_ident;

pub(crate) fn type_name_size(dictionary: &Dictionary, ty: &TypeName) -> usize {
    match ty {
        TypeName::Integer { name } => match name {
            IntegerKind::U8 | IntegerKind::I8 => 1,
            IntegerKind::U16 | IntegerKind::I16 => 2,
            IntegerKind::U32 | IntegerKind::I32 => 4,
            IntegerKind::U64 | IntegerKind::I64 => 8,
        },
        TypeName::Float { name } => match name {
            FloatKind::F32 => 4,
            FloatKind::F64 => 8,
        },
        TypeName::Bool => 1,
        TypeName::String { size } => {
            // Size prefix
            type_name_size(
                &dictionary,
                &TypeName::QualifiedIdentifier {
                    name: "FwSizeType".to_string(),
                },
            ) + (*size as usize)
        }
        TypeName::QualifiedIdentifier { name } => type_definition_size(
            &dictionary,
            &dictionary
                .type_definitions
                .get(&*name)
                .expect("Expected type definition"),
        ),
    }
}

fn type_definition_size(dictionary: &Dictionary, ty: &TypeDefinition) -> usize {
    match ty {
        TypeDefinition::Array(a) => (a.size as usize) * type_name_size(dictionary, &a.element_type),
        TypeDefinition::Enum(e) => type_name_size(dictionary, &e.representation_type),
        TypeDefinition::Struct(s) => s.members.iter().fold(0, |size, m| {
            size + (type_name_size(dictionary, &m.type_name) * (m.size.unwrap_or(1) as usize))
        }),
        TypeDefinition::Alias(a) => type_name_size(dictionary, &a.underlying_type),
    }
}

fn cmd_size(dictionary: &Dictionary, cmd: &Command) -> usize {
    let opcode_size = type_definition_size(
        &dictionary,
        dictionary.type_definitions.get("FwOpcodeType").unwrap(),
    );

    opcode_size
        + cmd.formal_params.iter().fold(0, |size, param| {
            type_name_size(dictionary, &param.type_name) + size
        })
}

/// The crate-root items every accessor leans on: the scratch buffers, `now()` and
/// `TimeValue`'s ordering.
pub(crate) fn global_helpers(dictionary: &Dictionary) -> TokenStream {
    let memory = global_memory(dictionary);
    let now = time_now();
    let cmp = time_cmp();

    quote! {
        #[allow(unused_imports)]
        use fprime_core::*;

        #memory
        #now
        #cmp
    }
}

fn global_memory(dictionary: &Dictionary) -> TokenStream {
    // Compute the upper bound scratch space needed
    // Scratch memory is used for telemetry, commands and parameters
    // TODO(tumbar) Add parameters
    let max_size = dictionary
        .commands
        .iter()
        .map(|cmd| cmd_size(&dictionary, &cmd))
        .chain(
            dictionary
                .telemetry_channels
                .iter()
                .map(|tlm| type_name_size(&dictionary, &tlm.type_name)),
        )
        .max()
        .unwrap_or(0);

    quote! {
        const __SCRATCH_SIZE: usize = #max_size;
        static mut __SCRATCH: [u8; __SCRATCH_SIZE] = [0x0; __SCRATCH_SIZE];

        const __TIME_SIZE: usize = crate::Dict::Fw::TimeValue::SIZE;
        static mut __TIME: [u8; __TIME_SIZE] = [0; __TIME_SIZE];
    }
}

fn time_now() -> TokenStream {
    quote! {
        pub fn now() -> crate::Dict::Fw::TimeValue {
            use fprime_core::Serializable;
            let mut buf: [u8; crate::Dict::Fw::TimeValue::SIZE] = [0; crate::Dict::Fw::TimeValue::SIZE];
            unsafe {
                fprime_core::time::time_read(&mut buf);
            }

            crate::Dict::Fw::TimeValue::deserialize(&buf)
        }
    }
}

fn time_cmp() -> TokenStream {
    quote! {
        impl PartialOrd for crate::Dict::Fw::TimeValue {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                if self.timeBase != other.timeBase {
                    fprime_core::panic(fprime_core::PanicCode::TimeBaseIncomparable);
                } else if self.timeContext != other.timeContext {
                    fprime_core::panic(fprime_core::PanicCode::TimeContextIncomparable);
                }

                match self.seconds.partial_cmp(&other.seconds) {
                    Some(core::cmp::Ordering::Equal) => {}
                    ord => return ord,
                }

                self.useconds.partial_cmp(&other.useconds)
            }
        }
    }
}

pub(crate) fn split_identifier(qi: &str) -> (Qualifier, Ident) {
    let (qualifier, name) = split_qualified_name(qi);

    (
        qualifier.iter().map(|s| s.to_string()).collect(),
        str_to_ident(name),
    )
}

pub(crate) fn annotate(inner: TokenStream, annotation: &Option<String>) -> TokenStream {
    match annotation {
        None => inner,
        Some(annotation) => {
            let annot = quote_doctest::doc_comment(annotation);
            quote! {
                #annot
                #inner
            }
        }
    }
}

pub(crate) fn annotate_with_args(
    inner: TokenStream,
    annotation: &Option<String>,
    arg_annotations: Vec<(String, Option<String>)>,
) -> TokenStream {
    if arg_annotations.len() == 0 {
        return annotate(inner, annotation);
    }

    let brief = match annotation {
        None => "".to_string(),
        Some(annotation) => annotation.clone(),
    };

    let args = arg_annotations
        .iter()
        .map(|(name, annot)| match annot {
            None => format!(" * `{}`", name),
            Some(arg_a) => format!(" * `{}` - {}", name, arg_a),
        })
        .collect::<Vec<String>>()
        .join("\n");

    let annot = quote_doctest::doc_comment(format!("{}\n\n{}", brief, args));
    quote! {
        #annot
        #inner
    }
}

pub(crate) fn hex_literal(value: u64) -> Literal {
    let hex_string = format!("0x{:X}", value);
    Literal::from_str(&hex_string).expect("Failed to parse hex literal string")
}

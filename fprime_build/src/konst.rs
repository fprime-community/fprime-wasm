//! Const encoders for commands whose arguments are known at compile time.
//!
//! For each const-encodable command this emits a `_size` / `_encode` pair. A
//! call site that passes only literals and enumerated constants is rewritten by
//! `#[fprime_main]` to bind the buffer as a `const`, which puts the whole
//! `Fw::ComBuffer` in rodata instead of assembling it with a run of stores on
//! every call. See `.notes/const-command-encoding.md`.

use crate::tree::Qualifier;
use crate::types::type_name;
use crate::util::{split_identifier, str_to_ident, type_name_size};
use fprime_dictionary::konst::{ConstArg, ENCODE_SUFFIX, SIZE_SUFFIX};
use fprime_dictionary::{Command, Dictionary};
use proc_macro2::Ident;
use proc_macro2::{Literal, TokenStream};
use quote::quote;

/// The name of a command's size function, e.g. `CMD_NO_OP_STRING__size`.
fn size_fn(name: &str) -> proc_macro2::Ident {
    str_to_ident(&format!("{}{}", name, SIZE_SUFFIX))
}

/// The name of a command's encoder, e.g. `CMD_NO_OP_STRING__encode`.
fn encode_fn(name: &str) -> proc_macro2::Ident {
    str_to_ident(&format!("{}{}", name, ENCODE_SUFFIX))
}

pub fn command(dict: &Dictionary, cmd: &Command) -> Option<(Qualifier, TokenStream)> {
    let args: Vec<(proc_macro2::Ident, TokenStream, ConstArg)> = cmd
        .formal_params
        .iter()
        .map(|param| {
            let arg = dict.const_arg(&param.type_name)?;
            let name = str_to_ident(&param.name);

            // A `string size N` argument is passed as a `&str`, matching the
            // calling convention of the runtime accessor.
            let ty = match arg {
                ConstArg::Str { .. } => quote! { &str },
                _ => type_name(&param.type_name),
            };

            Some((name, ty, arg))
        })
        .collect::<Option<_>>()?;

    let opcode_arg = dict.const_arg(&opcode_type())?;
    let ConstArg::Scalar { put: put_opcode } = opcode_arg else {
        return None;
    };
    let put_opcode = str_to_ident(put_opcode);
    let opcode = Literal::u64_unsuffixed(cmd.opcode);
    let opcode_size = type_name_size(dict, &opcode_type());

    // The size of everything that is not a string is fixed by the dictionary; a
    // string contributes its `FwSizeStoreType` prefix plus however much of it
    // survives truncation, which only `str_len` can say.
    let mut fixed = opcode_size;
    let mut variable = vec![];
    for (name, _, arg) in &args {
        match arg {
            ConstArg::Str { capacity, prefix } => {
                let capacity = Literal::u32_unsuffixed(*capacity);
                let prefix = Literal::u32_unsuffixed(*prefix);
                variable.push(quote! { + str_len(#name, #capacity, #prefix) });
            }
            _ => {
                fixed += param_size(dict, cmd, name);
            }
        }
    }
    let fixed = Literal::usize_unsuffixed(fixed);

    // `put_*` returns the offset past what it wrote, so the encoder threads the
    // offset through and then proves it landed exactly on `N`.
    let writes = args
        .iter()
        .map(|(name, _, arg)| write_value(arg, &quote! { #name }, 0));

    // `mut` is only warranted when something writes after the opcode.
    let opcode_binding = if args.is_empty() {
        quote! { let __offset = #put_opcode(&mut __buf, 0, #opcode); }
    } else {
        quote! { let mut __offset = #put_opcode(&mut __buf, 0, #opcode); }
    };

    let params = args.iter().map(|(name, ty, _)| quote! { #name: #ty, });
    let size_params = params.clone();

    let (q, name) = split_identifier(&cmd.name);
    let size = size_fn(&name.to_string());
    let encode = encode_fn(&name.to_string());

    let tokens = quote! {
        /// Wire size of this command with these arguments.
        #[allow(unused_variables)]
        pub const fn #size(#(#size_params)*) -> usize {
            #fixed #(#variable)*
        }

        /// The encoded `Fw::ComBuffer` for this command with these arguments.
        ///
        /// `N` must be what the matching `__size` returned; the assertion is
        /// proven during const evaluation, so a mismatch is a compile error and
        /// costs nothing at runtime.
        pub const fn #encode<const N: usize>(#(#params)*) -> [u8; N] {
            let mut __buf = [0u8; N];
            #opcode_binding
            #(#writes)*

            assert!(__offset == N);
            __buf
        }
    };

    Some((q, tokens))
}

/// `FwOpcodeType`, the dictionary's own name for a command opcode.
fn opcode_type() -> fprime_dictionary::TypeName {
    fprime_dictionary::TypeName::QualifiedIdentifier {
        name: "FwOpcodeType".to_string(),
    }
}

fn param_size(dict: &Dictionary, cmd: &Command, name: &proc_macro2::Ident) -> usize {
    let param = cmd
        .formal_params
        .iter()
        .find(|param| str_to_ident(&param.name) == *name)
        .expect("parameter was taken from this command");

    type_name_size(dict, &param.type_name)
}

/// The writes for a value of the shape `plan`, reached by the expression
/// `access`.
///
/// Structs and arrays are descended here rather than handed to a `const` trait
/// method, because `const fn` in a trait is not stable (E0379) and because the
/// dictionary already knows the whole shape: emitting the field and element
/// writes directly needs no trait, no derive support and no nightly feature.
///
/// `depth` only names the loop counters, so nested arrays do not shadow.
fn write_value(plan: &ConstArg, access: &TokenStream, depth: usize) -> TokenStream {
    match plan {
        ConstArg::Scalar { put } => {
            let put = str_to_ident(put);
            quote! { __offset = #put(&mut __buf, __offset, #access); }
        }
        ConstArg::Cast { put, cast } => {
            let put = str_to_ident(put);
            let cast = str_to_ident(cast);
            quote! { __offset = #put(&mut __buf, __offset, #access as #cast); }
        }
        ConstArg::Str { capacity, prefix } => {
            let capacity = Literal::u32_unsuffixed(*capacity);
            let prefix = Literal::u32_unsuffixed(*prefix);
            quote! { __offset = put_str(&mut __buf, __offset, #access, #capacity, #prefix); }
        }
        ConstArg::Array { element, size } => {
            // A `while` rather than an unrolled run of writes: both cost nothing
            // at runtime once const-evaluated, and the loop keeps the generated
            // file small.
            let index = Ident::new(&format!("__i{}", depth), proc_macro2::Span::call_site());
            let size = Literal::u32_unsuffixed(*size);
            let inner = write_value(element, &quote! { #access[#index] }, depth + 1);

            quote! {
                let mut #index = 0;
                while #index < #size {
                    #inner
                    #index += 1;
                }
            }
        }
        ConstArg::Struct { members } => {
            // In dictionary index order, which is the order the fields are
            // declared in and so the order `Serializable` writes them.
            let writes = members.iter().map(|member| {
                let name = str_to_ident(&member.name);

                write_value(&member.encoding, &quote! { #access.#name }, depth)
            });

            quote! { #(#writes)* }
        }
    }
}

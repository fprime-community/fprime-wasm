use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenTree};
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod command;
mod fprime_test;
mod infer_path;
mod parameter;
mod serializable;
mod signature;
mod telemetry;

/// Render a macro result, reporting failures where the macro was invoked.
fn expand(result: syn::Result<proc_macro2::TokenStream>) -> TokenStream {
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

#[proc_macro_derive(Serializable)]
pub fn derive_serializable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    expand(serializable::derive_serializable(input))
}

/// Implement a dictionary command accessor from its signature.
///
/// The formal parameters and the `Fw::CmdResponse` return type are read off the
/// stub's signature, so the attribute only carries the opcode:
///
/// ```ignore
/// #[fprime_command(opcode = 0x1000001)]
/// pub fn CMD_NO_OP_STRING(&self, arg1: String<40>) -> super::Defs::Fw::CmdResponse {}
/// ```
///
/// A `String<N>` parameter is the wire type, not the calling convention: the
/// caller passes a `&str` that is truncated to `N` bytes on the way out.
///
/// The expansion encodes the command into the enclosing module's `__SCRATCH`
/// buffer, so it requires `__SCRATCH`, `__SCRATCH_SIZE` and the dictionary's
/// `FwOpcodeType` to be in scope alongside `fprime_core`'s prelude.
#[proc_macro_attribute]
pub fn fprime_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(command::command(attr.into(), item.into()))
}

/// Implement a dictionary telemetry channel accessor from its signature.
///
/// The channel value type and the time type are read off the stub's `(value,
/// time)` return type, so the attribute only carries the channel id:
///
/// ```ignore
/// #[fprime_telemetry(id = 0x1000000)]
/// pub fn CommandsDispatched(&self) -> (u32, super::Defs::Fw::TimeValue) {}
/// ```
#[proc_macro_attribute]
pub fn fprime_telemetry(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(telemetry::telemetry(attr.into(), item.into()))
}

/// Implement a dictionary parameter accessor from its signature.
///
/// The parameter type is read off the stub's return type, so the attribute only
/// carries the parameter id:
///
/// ```ignore
/// #[fprime_parameter(id = 0x2000000)]
/// pub fn parameter1(&self) -> u32 {}
/// ```
#[proc_macro_attribute]
pub fn fprime_parameter(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(parameter::parameter(attr.into(), item.into()))
}

/// Enable the F Prime sequencing DSL in a function body.
///
/// Command arguments may name an enumerated constant on its own, and the
/// expansion resolves it against the dictionary using the command's formal
/// parameter types:
///
/// ```ignore
/// #[fprime]
/// fn take_a_data_product() {
///     Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);
/// }
/// ```
///
/// The dictionary comes from the `fprime_build::generate` call in the crate's
/// `build.rs`, which publishes it through the `FPRIME_DICTIONARY` environment
/// variable.
#[proc_macro_attribute]
pub fn fprime(attr: TokenStream, item: TokenStream) -> TokenStream {
    infer_enum_paths(attr.into(), item.into()).into()
}

/// A test that runs one sequence on the on-board interpreter and checks what it did.
///
/// ```ignore
/// #[fprime_test(sequence = "safing")]
/// fn powers_down_on_low_battery(t: Test) {
///     t.initial_telemetry(Ref.power.BatteryVoltage, 21.5);
///     t.expect_command(Ref.power.PWR_OFF());
///     t.expect_exit(0);
/// }
/// ```
#[proc_macro_attribute]
pub fn fprime_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(fprime_test::expand(attr.into(), item.into()))
}

/// Wires up the Wasm entry point, and enables the sequencing DSL (see [`fprime`]).
#[proc_macro_attribute]
pub fn fprime_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let entry = export(infer_enum_paths(attr.into(), item.into()));

    quote! {
        #[panic_handler]
        fn __panic_handler(info: &core::panic::PanicInfo) -> ! {
            fprime_core::panic_handler(info);
        }

        #[unsafe(no_mangle)]
        #entry
    }
    .into()
}

/// Resolve the sequencing DSL in `item`, keeping the item whatever happens.
fn infer_enum_paths(
    attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match infer_path::rewrite(attr, item.clone()) {
        Ok(qualified) => qualified,
        Err(err) => {
            let error = err.to_compile_error();
            quote! {
                #error
                #item
            }
        }
    }
}

fn export(item: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let trees: Vec<TokenTree> = item.into_iter().collect();
    let mut index = 0;

    while matches!(trees.get(index), Some(TokenTree::Punct(punct)) if punct.as_char() == '#') {
        index += 2;
    }

    match trees.get(index) {
        Some(TokenTree::Ident(ident)) if ident != "pub" => {
            let visibility = TokenTree::Ident(Ident::new("pub", ident.span()));

            trees[..index]
                .iter()
                .cloned()
                .chain([visibility])
                .chain(trees[index..].iter().cloned())
                .collect()
        }
        _ => trees.into_iter().collect(),
    }
}

#[cfg(test)]
mod test {
    mod test;
}

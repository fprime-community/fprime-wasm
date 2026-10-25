//! Shared parsing for the dictionary accessor attributes.
//!
//! Every accessor is declared as a bodyless stub whose signature carries the
//! modeled types, plus an attribute carrying the one thing the signature cannot
//! express: the dictionary id or opcode.

use proc_macro2::TokenStream;
use syn::parse::{ParseStream, Parser};
use syn::{Expr, GenericArgument, Ident, ItemFn, LitInt, PathArguments, ReturnType, Token, Type};

/// Parse a `#[fprime_*(<key> = 0x...)]` argument list into its literal.
pub(crate) fn id_argument(attr: TokenStream, key: &str) -> syn::Result<LitInt> {
    if attr.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("expected `{} = <value>`", key),
        ));
    }

    let parser = |input: ParseStream| {
        let ident: Ident = input.parse()?;
        if ident != key {
            return Err(syn::Error::new(
                ident.span(),
                format!("expected `{}`, found `{}`", key, ident),
            ));
        }

        input.parse::<Token![=]>()?;
        let value: LitInt = input.parse()?;

        if !input.is_empty() {
            return Err(input.error(format!("unexpected tokens after the `{}` value", key)));
        }

        Ok(value)
    };

    Parser::parse2(parser, attr)
}

/// Parse the annotated item as an accessor stub.
///
/// The body is supplied by the macro, so the stub must not declare one.
pub(crate) fn parse_stub(item: TokenStream) -> syn::Result<ItemFn> {
    let stub: ItemFn = syn::parse2(item)?;

    if !stub.block.stmts.is_empty() {
        return Err(syn::Error::new_spanned(
            &stub.block,
            "expected an empty body, the implementation is generated from the signature",
        ));
    }

    Ok(stub)
}

/// The declared return type, which is where the accessor's modeled types live.
pub(crate) fn return_type(stub: &ItemFn) -> syn::Result<&Type> {
    match &stub.sig.output {
        ReturnType::Default => Err(syn::Error::new_spanned(
            &stub.sig,
            "expected a return type naming the modeled type",
        )),
        ReturnType::Type(_, ty) => Ok(ty),
    }
}

/// `Some(N)` when `ty` is a [`fprime_core::String<N>`](fprime_core::String).
///
/// Modeled types always carry their full `crate::Dict::` path, so an
/// unqualified `String<N>` is unambiguously the `fprime_core` re-export that a
/// string-typed dictionary member renders as.
pub(crate) fn string_capacity(ty: &Type) -> Option<&Expr> {
    let Type::Path(path) = ty else {
        return None;
    };

    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }

    let segment = &path.path.segments[0];
    if segment.ident != "String" {
        return None;
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };

    match (arguments.args.len(), arguments.args.first()) {
        (1, Some(GenericArgument::Const(size))) => Some(size),
        _ => None,
    }
}

//! Shared parsing for the dictionary accessor attributes.
//!
//! Every accessor is declared as a bodyless stub whose signature carries the
//! modeled types, plus an attribute carrying the one thing the signature cannot
//! express: the dictionary id or opcode.

use proc_macro2::TokenStream;
use syn::parse::{ParseStream, Parser};
use syn::{GenericArgument, Ident, ItemFn, LitInt, PathArguments, ReturnType, Token, Type};

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

/// Whether `ty` is the dictionary's `crate::String<N>`, a `string size N` (or the
/// bare `String<N>` it is in scope as).
///
/// Modeled types always carry their full `crate::Dict::` path, so neither is
/// mistaken for a dictionary type that happens to be named `String`.
pub(crate) fn is_string(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };

    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return false;
    }

    let segments: Vec<_> = path.path.segments.iter().collect();
    let segment = match segments.as_slice() {
        [segment] => segment,
        [krate, segment] if krate.ident == "crate" && krate.arguments.is_none() => segment,
        _ => return false,
    };

    if segment.ident != "String" {
        return false;
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };

    matches!(
        arguments.args.iter().collect::<Vec<_>>().as_slice(),
        [GenericArgument::Const(_)]
    )
}

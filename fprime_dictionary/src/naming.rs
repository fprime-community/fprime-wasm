use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

pub fn str_to_ident(name: &str) -> Ident {
    match name {
        name @ ("as" | "async" | "await" | "break" | "const" | "continue" | "crate" | "dyn"
        | "else" | "enum" | "extern" | "false" | "fn" | "for" | "if" | "impl" | "in"
        | "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub" | "ref" | "return"
        | "self" | "Self" | "static" | "struct" | "super" | "trait" | "true" | "type"
        | "unsafe" | "use" | "where" | "while" | "names" | "abstract" | "become"
        | "box" | "do" | "final" | "gen" | "macro" | "override" | "priv" | "try"
        | "typeof" | "unsized" | "virtual" | "yield") => {
            // Protect against Rust keyword overlap
            Ident::new_raw(name, Span::call_site())
        }
        name => Ident::new(name, Span::call_site()),
    }
}

/// Splits `Ref.DpDemo.DpReqType` into `(["Ref", "DpDemo"], "DpReqType")`.
pub fn split_qualified_name(qualified_name: &str) -> (Vec<&str>, &str) {
    let mut qualifier: Vec<&str> = qualified_name.split('.').collect();

    // `split` always yields at least one element
    let name = qualifier.pop().expect("empty qualified identifier");

    (qualifier, name)
}

/// The Rust path of a generated type definition, e.g. `Ref.DpDemo.DpReqType` becomes
/// `crate::Defs::Ref::DpDemo::DpReqType`.
pub fn definition_path(qualified_name: &str) -> TokenStream {
    let (_, name) = split_qualified_name(qualified_name);

    definition_path_with_name(qualified_name, str_to_ident(name))
}

/// [`definition_path`], with the trailing identifier supplied instead of derived.
pub fn definition_path_with_name(qualified_name: &str, name: Ident) -> TokenStream {
    let (qualifier, _) = split_qualified_name(qualified_name);
    let modules = qualifier.into_iter().map(str_to_ident);

    quote! { crate::Defs::#(#modules::)*#name }
}

/// The Rust path of a generated descriptor, e.g. `Ref.power.BatteryVoltage` becomes
/// `crate::Desc::Ref::power::BatteryVoltage`.
pub fn desc_path(qualified_name: &str) -> TokenStream {
    let (qualifier, name) = split_qualified_name(qualified_name);
    let modules = qualifier.into_iter().map(str_to_ident);
    let name = str_to_ident(name);

    quote! { crate::Desc::#(#modules::)*#name }
}

/// The Rust path of a generated const encoder, e.g. `CdhCore.cmdDisp.CMD_NO_OP`
/// with [`crate::konst::ENCODE_SUFFIX`] becomes
/// `crate::Konst::CdhCore::cmdDisp::CMD_NO_OP__encode`.
pub fn konst_path(qualified_name: &str, suffix: &str) -> TokenStream {
    let (qualifier, name) = split_qualified_name(qualified_name);
    let modules = qualifier.into_iter().map(str_to_ident);
    let name = str_to_ident(&format!("{}{}", name, suffix));

    quote! { crate::Konst::#(#modules::)*#name }
}

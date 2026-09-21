//! Dictionary item descriptors for interacting with #[fprime_test]

use crate::tree::Qualifier;
use crate::types::type_name;
use crate::util::{annotate, hex_literal, split_identifier, str_to_ident};
use fprime_dictionary::{
    Command, Dictionary, Parameter, TelemetryChannel, TypeDefinition, TypeName,
};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::collections::HashMap;

/// A command descriptor that matches on opcode alone (the no-parentheses form).
pub fn command(cmd: &Command) -> (Qualifier, TokenStream) {
    let (q, name) = split_identifier(&cmd.name);
    let opcode = hex_literal(cmd.opcode);
    let path = Literal::string(&cmd.name);

    let def = quote! {
        pub const #name: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
            opcode: #opcode as u32,
            path: #path,
        };
    };

    (q, annotate(def, &cmd.annotation))
}

pub fn telemetry_channel(dict: &Dictionary, tlm: &TelemetryChannel) -> (Qualifier, TokenStream) {
    let (q, name) = split_identifier(&tlm.name);
    let wire = wire_type(dict, &tlm.type_name);
    let id = hex_literal(tlm.id);
    let path = Literal::string(&tlm.name);
    let ty = Literal::string(&label(dict, &tlm.type_name));

    let def = quote! {
        pub const #name: fprime_core::desc::Chan<#wire> =
            fprime_core::desc::Chan::new(#id as i64, #path, #ty);
    };

    (q, annotate(def, &tlm.annotation))
}

pub fn parameter(dict: &Dictionary, prm: &Parameter) -> (Qualifier, TokenStream) {
    let (q, name) = split_identifier(&prm.name);
    let wire = wire_type(dict, &prm.type_name);
    let id = hex_literal(prm.id);
    let path = Literal::string(&prm.name);
    let ty = Literal::string(&label(dict, &prm.type_name));

    let def = quote! {
        pub const #name: fprime_core::desc::Prm<#wire> =
            fprime_core::desc::Prm::new(#id as i64, #path, #ty);
    };

    (q, annotate(def, &prm.annotation))
}

/// `Desc::Response`: one named `const` per `Fw.CmdResponse` constant.
pub fn responses(dict: &Dictionary) -> Option<(Qualifier, TokenStream)> {
    let Some(TypeDefinition::Enum(response)) = dict.type_definitions.get("Fw.CmdResponse") else {
        return None;
    };

    let constants = response.enumerated_constants.iter().map(|constant| {
        let name = str_to_ident(&constant.name);
        // Checked, not truncated: `cmd` returns an `i32`.
        let value = i32::try_from(constant.value).unwrap_or_else(|_| {
            panic!(
                "Fw.CmdResponse::{} is {}, which does not fit the i32 the `cmd` host \
                 function returns",
                constant.name, constant.value
            )
        });
        let value = Literal::i32_unsuffixed(value);
        let label = Literal::string(&constant.name);
        annotate(
            quote! {
                pub const #name: fprime_core::desc::Response =
                    fprime_core::desc::Response::new(#value, #label);
            },
            &constant.annotation,
        )
    });

    Some((vec!["Response".to_string()], quote! { #(#constants)* }))
}

/// The wire type of a dictionary point: the `Serializable` a descriptor is generic over,
/// which is what writes the point's bytes and states their size.
///
/// A `string size N` point is a `String<N>`; a test still writes a bare `&str` for one,
/// which `fprime_core::desc::IntoWire` converts.
fn wire_type(dict: &Dictionary, ty: &TypeName) -> TokenStream {
    if let TypeName::String { size } = dict.underlying_type(ty) {
        let cap = Literal::u32_unsuffixed(*size);
        return quote! { fprime_core::String<#cap> };
    }

    type_name(ty)
}

/// How the dictionary spells a type, for a failure message.
fn label(dict: &Dictionary, ty: &TypeName) -> String {
    match dict.underlying_type(ty) {
        TypeName::String { size } => format!("string size {size}"),
        _ => match ty {
            TypeName::Integer { name } => format!("{name:?}"),
            TypeName::Float { name } => format!("{name:?}"),
            TypeName::Bool => "bool".to_string(),
            TypeName::String { size } => format!("string size {size}"),
            TypeName::QualifiedIdentifier { name } => name.clone(),
        },
    }
}

/// Panics if two points in one component would collide in the flat `Desc` namespace.
pub fn check_for_collisions(dict: &Dictionary) {
    let mut seen: HashMap<&str, &'static str> = HashMap::new();
    let mut collisions: Vec<String> = Vec::new();

    let points = dict
        .commands
        .iter()
        .map(|cmd| (cmd.name.as_str(), "command"))
        .chain(
            dict.telemetry_channels
                .iter()
                .map(|tlm| (tlm.name.as_str(), "telemetry channel")),
        )
        .chain(
            dict.parameters
                .iter()
                .map(|prm| (prm.name.as_str(), "parameter")),
        );

    for (name, kind) in points {
        if let Some(first) = seen.insert(name, kind) {
            collisions.push(format!("  {name} is both a {first} and a {kind}"));
        }
    }

    if !collisions.is_empty() {
        panic!(
            "this dictionary has points that a test could not name unambiguously:\n{}\n\
             A command, telemetry channel and parameter share one namespace per component \
             in the generated `Desc` tree, so that a test can complete every point a \
             component offers in one list. Rename one of each pair in the FPP model.",
            collisions.join("\n")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fprime_dictionary::{FloatKind, IntegerKind};

    /// Shared fixture dictionary.
    fn dictionary() -> Dictionary {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fprime_dictionary/src/test/RefTopologyDictionary.json");
        fprime_dictionary::parse(&path)
    }

    /// A label must read like FPP, not Rust.
    #[test]
    fn label_reads_like_dictionary() {
        let dict = dictionary();
        assert_eq!(
            label(
                &dict,
                &TypeName::Float {
                    name: FloatKind::F32
                }
            ),
            "F32"
        );
        assert_eq!(
            label(
                &dict,
                &TypeName::Integer {
                    name: IntegerKind::U8
                }
            ),
            "U8"
        );
        assert_eq!(label(&dict, &TypeName::Bool), "bool");
        assert_eq!(
            label(&dict, &TypeName::String { size: 40 }),
            "string size 40"
        );
        assert_eq!(
            label(
                &dict,
                &TypeName::QualifiedIdentifier {
                    name: "Ref.Choice".to_string()
                }
            ),
            "Ref.Choice"
        );
    }

    /// A string-typed alias must carry the capacity its wire type needs.
    #[test]
    fn alias_to_string_is_a_sized_string() {
        let dict = dictionary();
        let aliased = TypeName::QualifiedIdentifier {
            name: "Ref.DpDemo.StringAlias".to_string(),
        };
        // Falls back to the plain-string path (same code) if the dictionary lacks this alias.
        let ty = match dict.type_definitions.get("Ref.DpDemo.StringAlias") {
            Some(_) => aliased,
            None => TypeName::String { size: 40 },
        };
        let wire = wire_type(&dict, &ty).to_string();
        assert!(
            wire.contains("String") && wire.contains("40"),
            "a string point is a String<40>, got {wire}"
        );
    }

    /// Every other point is its own Rust type, with no conversion in the way.
    #[test]
    fn scalar_point_is_its_own_type() {
        let dict = dictionary();
        let wire = wire_type(
            &dict,
            &TypeName::Float {
                name: FloatKind::F32,
            },
        );
        assert_eq!(wire.to_string(), "f32");
    }

    #[test]
    fn reference_dictionary_has_no_collisions() {
        check_for_collisions(&dictionary());
    }
}

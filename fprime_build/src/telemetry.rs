use crate::tree::Qualifier;
use crate::types::type_name;
use crate::util::{annotate, hex_literal, split_identifier};
use proc_macro2::TokenStream;
use quote::quote;

pub fn telemetry_channel(tlm: &fprime_dictionary::TelemetryChannel) -> (Qualifier, TokenStream) {
    let (q, name) = split_identifier(&tlm.name);
    let ty = type_name(&tlm.type_name);

    let id = hex_literal(tlm.id);

    let def = quote! {
        #[fprime_telemetry(id = #id)]
        pub fn #name(&self) -> (#ty, super::Dict::Fw::TimeValue) {}
    };

    (q, annotate(def, &tlm.annotation))
}

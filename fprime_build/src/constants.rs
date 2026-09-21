use fprime_dictionary::{Constant, Dictionary};
use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    tree::Qualifier,
    types::{type_name, type_name_is_string},
    util::{annotate, split_identifier},
    values::value,
};

pub fn constant(c: &Constant, dict: &Dictionary) -> (Qualifier, TokenStream) {
    let (q, name) = split_identifier(&c.qualified_name);
    let ty = if type_name_is_string(&c.type_name, dict) {
        quote! { &'static str }
    } else {
        type_name(&c.type_name)
    };

    let v = value(&c.value);

    let def = quote! {
        pub const #name: #ty = #v;
    };

    (q, annotate(def, &c.annotation))
}

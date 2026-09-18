use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput};

/// Returns the repr integer type as a string (e.g. "u8", "i32") if present.
fn enum_repr_type_name(attrs: &[syn::Attribute]) -> Option<Ident> {
    let mut repr: Option<Ident> = None;

    for attr in attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }

        let _ = attr.parse_nested_meta(|meta| {
            if let Some(ident) = meta.path.get_ident() {
                repr = Some(ident.clone());
            }
            Ok(())
        });

        // Stop after first repr attribute
        if repr.is_some() {
            break;
        }
    }

    repr
}

/// The name a field is bound to while deserializing, positional for tuple structs.
fn field_binding(index: usize, field: &syn::Field) -> Ident {
    match &field.ident {
        None => Ident::new(
            &format!("_{}", Literal::usize_unsuffixed(index)),
            Span::call_site(),
        ),
        Some(name) => name.clone(),
    }
}

fn derive_struct(input: &DeriveInput, s: &DataStruct) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let size = s.fields.iter().map(|field| {
        let ty = &field.ty;
        quote! { <#ty as Serializable>::SIZE }
    });

    let serialize_to = s
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| match &field.ident {
            None => {
                let name = Literal::usize_unsuffixed(i);
                quote! { self.#name.serialize_to(__to, __offset); }
            }
            Some(name) => quote! { self.#name.serialize_to(__to, __offset); },
        });

    let deserialize_from = s.fields.iter().enumerate().map(|(i, field)| {
        let ty = &field.ty;
        let name = field_binding(i, field);

        quote! { let #name: #ty = Serializable::deserialize_from(__from, __offset); }
    });

    let field_names: Vec<Ident> = s
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| field_binding(i, field))
        .collect();

    quote! {
        impl #impl_generics Serializable for #name #ty_generics #where_clause {
            const SIZE: usize = 0 #(+ #size)*;

            fn serialize_to(&self, __to: &mut [u8], __offset: &mut usize) {
                #(#serialize_to)*
            }

            fn deserialize_from(__from: &[u8], __offset: &mut usize) -> Self {
                #(#deserialize_from)*
                Self {
                    #(#field_names,)*
                }
            }
        }
    }
}

fn derive_enum(input: &DeriveInput, e: &DataEnum) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let Some(repr) = enum_repr_type_name(&input.attrs) else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Serializable can only be derived on enums with explicit repr() attributes",
        ));
    };

    let mut match_branches = vec![];
    for variant in &e.variants {
        let Some((_, value)) = &variant.discriminant else {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "Serializable can only be derived on enums with explicit values on all variants",
            ));
        };

        let variant_name = &variant.ident;
        match_branches.push(quote! {
            #value => Self::#variant_name,
        })
    }

    Ok(quote! {
        impl #impl_generics Serializable for #name #ty_generics #where_clause {
            const SIZE: usize = #repr::SIZE;

            fn serialize_to(&self, __to: &mut [u8], __offset: &mut usize) {
                (*self as #repr).serialize_to(__to, __offset);
            }

            fn deserialize_from(__from: &[u8], __offset: &mut usize) -> Self {
                let raw: #repr = Serializable::deserialize_from(__from, __offset);
                match raw {
                    #(#match_branches)*
                    _ => fprime_core::panic(fprime_core::PanicCode::InvalidEnum),
                }
            }
        }
    })
}

pub(crate) fn derive_serializable(input: DeriveInput) -> syn::Result<TokenStream> {
    match &input.data {
        Data::Struct(s) => Ok(derive_struct(&input, s)),
        Data::Enum(e) => derive_enum(&input, e),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "Serializable can only be derived for structs and enums",
        )),
    }
}

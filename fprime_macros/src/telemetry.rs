use crate::signature::{id_argument, parse_stub, return_type};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::Type;
use syn::spanned::Spanned;

pub(crate) fn telemetry(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let id = id_argument(attr, "id")?;
    let stub = parse_stub(item)?;

    // A channel reads back as its value paired with the time it was last written
    let output = return_type(&stub)?;
    let Type::Tuple(output) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "expected a `(value, time)` return type",
        ));
    };

    if output.elems.len() != 2 {
        return Err(syn::Error::new_spanned(
            output,
            "expected a `(value, time)` return type",
        ));
    }

    let value = &output.elems[0];
    let time = &output.elems[1];

    let attrs = &stub.attrs;
    let vis = &stub.vis;
    let sig = &stub.sig;

    // Span the body onto the stub's own braces, see `command`
    let body = quote_spanned! { stub.block.span() =>
        {
            const { assert!(<#value as Serializable>::SIZE <= super::__SCRATCH_SIZE) };
            const { assert!(<#time as Serializable>::SIZE <= super::__TIME_SIZE) };

            let __time = unsafe {
                let ptr = (&raw mut super::__TIME) as *mut u8;
                let len = <#time as Serializable>::SIZE;

                core::slice::from_raw_parts_mut(ptr, len)
            };

            let __value = unsafe {
                let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                let len = <#value as Serializable>::SIZE;

                core::slice::from_raw_parts_mut(ptr, len)
            };

            unsafe {
                telemetry(#id, __time, __value)
            };

            (
                <#value as Serializable>::deserialize(__value),
                <#time as Serializable>::deserialize(__time)
            )
        }
    };

    Ok(quote! {
        #(#attrs)*
        #vis #sig #body
    })
}

use crate::signature::{id_argument, parse_stub, string_capacity};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{FnArg, parse_quote_spanned};

pub(crate) fn command(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let opcode = id_argument(attr, "opcode")?;
    let stub = parse_stub(item)?;

    let attrs = &stub.attrs;
    let vis = &stub.vis;
    let mut sig = stub.sig.clone();

    // Serialize each formal parameter in declaration order, rewriting the
    // signature where the wire type is not the type we want callers to pass.
    let mut serialize = vec![];
    let mut has_str_arg = false;
    for arg in sig.inputs.iter_mut() {
        let FnArg::Typed(arg) = arg else {
            continue;
        };

        // Attribute the generated statement to the argument it serializes
        let span = arg.span();
        match string_capacity(&arg.ty).cloned() {
            // TODO(tumbar) Make string commanding configurable
            // Currently we accept &str and truncate size to fit
            Some(size) => {
                has_str_arg = true;
                let name = &arg.pat;
                serialize.push(quote_spanned! { span =>
                    serialize_str::<#size>(#name, __encoded, &mut __offset);
                });

                let ty_span = arg.ty.span();
                *arg.ty = parse_quote_spanned! { ty_span => &str };
            }
            None => {
                let name = &arg.pat;
                serialize.push(quote_spanned! { span =>
                    #name.serialize_to(__encoded, &mut __offset);
                });
            }
        }
    }

    // Span the body onto the stub's own braces. `Span::call_site()` would leave
    // the expanded item straddling two files, which costs an LSP the mapping it
    // needs to resolve the accessor back to its declaration
    let body = quote_spanned! { stub.block.span() =>
        {
            let __encoded = unsafe {
                let ptr = (&raw mut super::__SCRATCH) as *mut u8;
                let len = super::__SCRATCH_SIZE;

                core::slice::from_raw_parts_mut(ptr, len)
            };

            let mut __offset: usize = 0;
            let __opcode: FwOpcodeType = #opcode;
            __opcode.serialize_to(__encoded, &mut __offset);
            #(#serialize)*

            unsafe { command(__encoded.get_unchecked(0..__offset)) }
        }
    };

    let inline = has_str_arg.then(|| {
        quote_spanned! { stub.block.span() =>
            #[inline(always)]
        }
    });

    Ok(quote! {
        #(#attrs)*
        #inline
        #vis #sig #body
    })
}

use crate::signature::{id_argument, is_string, parse_stub};
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
        match is_string(&arg.ty) {
            true => {
                has_str_arg = true;
                let name = &arg.pat;
                let wire = &arg.ty;
                serialize.push(quote_spanned! { span =>
                    serialize_str::<#wire>(#name, __encoded, &mut __offset);
                });

                let ty_span = arg.ty.span();
                *arg.ty = parse_quote_spanned! { ty_span => &str };
            }
            false => {
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

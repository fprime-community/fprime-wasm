//! A dictionary point becomes the `Desc` descriptor a test matches on.

use crate::infer_path::scan::{Chain, Lower, Rewrite, scan};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::naming::desc_path;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{quote, quote_spanned};

/// `crate::Desc::<segments>`, built from the author's own ident tokens so goto/hover still
/// resolve them.
///
/// The invented tokens (`crate`, `Desc`, `::`) are spanned on `scaffold`, the chain's first
/// `.`. On the call site, rustc would report every type error about the descriptor on the
/// `#[fprime_test]` attribute. On one of the author's idents, goto-definition on that ident
/// would also answer with the invented tokens' definitions. A punct has none.
fn spanned_desc_path(segments: &[Ident], scaffold: Span) -> TokenStream {
    quote_spanned! { scaffold => crate::Desc #(:: #segments)* }
}

/// Pass two of `#[fprime_test]`: every dictionary point in a test body becomes a `Desc`
/// descriptor. A chain that is not a point — `t.initial_telemetry(..)`,
/// `event().containing(..)`, a local — is left alone.
///
/// `completing` says an editor is mid-keystroke in this body, which suppresses diagnostics.
pub(super) fn describe(symbols: &Symbols, tokens: TokenStream, completing: bool) -> TokenStream {
    scan(
        symbols,
        tokens,
        &Describe {
            symbols,
            completing,
        },
    )
}

struct Describe<'a> {
    symbols: &'a Symbols,
    completing: bool,
}

impl Lower for Describe<'_> {
    fn chain(&self, chain: &Chain<'_>) -> Rewrite {
        // Anything but a command call is a bare descriptor. A channel written with parentheses
        // keeps them, for rustc to reject at the right token.
        let (Point::Command(command), Some(args)) = (&chain.point, &chain.args) else {
            return Rewrite::Chain(spanned_desc_path(&chain.segments, chain.scaffold));
        };

        let original = chain.written_call();

        let why = match self.symbols.buffer(command, args) {
            Ok(buffer) => {
                return Rewrite::Call(buffer.block(&original, {
                    // Unspanned: the author's tokens are already in `if false { .. }`, and a
                    // second copy of their spans would give goto two definitions.
                    let path = desc_path(&chain.name);
                    quote! {
                        {
                            // A const of reference type gives `&'static [u8]` without static
                            // promotion; identical buffers still dedupe across call sites.
                            const __FPRIME_BUF: &[u8] = &__FPRIME_CMD;
                            #path.with(__FPRIME_BUF)
                        }
                    }
                }));
            }
            Err(why) => why,
        };

        // A test that wrote arguments asked for their exact bytes, and has no run-time fallback
        // as `#[fprime]` does: passing the call through would only earn an unrelated
        // `CmdResponse: IntoCommand` error. So the reason is reported instead.
        //
        // While an editor is typing, only the reason is left out. The author's tokens come
        // first either way, since rust-analyzer's two expansions must agree up to the cursor —
        // see `expansions_agree_up_to_the_cursor`.
        let error = match self.completing {
            true => TokenStream::new(),
            false => why.to_compile_error(),
        };
        // The dictionary holds an opcode as a u64; `FwOpcodeType` is narrower.
        let opcode = Literal::u64_unsuffixed(command.opcode);

        Rewrite::Call(quote! {
            {
                if false { #original; }
                #error
                // A value of the right type, so the reason above is the only error.
                ::fprime_test::cmd(#opcode as u32)
            }
        })
    }
}

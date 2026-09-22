//! A dictionary point becomes the `Desc` descriptor a test matches on.

use crate::infer_path::scan::{Chain, Lower, Rewrite, scan};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::naming::desc_path;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{quote, quote_spanned};

/// `crate::Desc::<segments>`, built from the author's own ident tokens so goto/hover still
/// resolve them.
///
/// `scaffold` puts the tokens this invents — `crate`, `Desc`, `::` — inside the chain the
/// author wrote, at its first `.`. On `Span::call_site()`, which is what `quote!` gives them,
/// they claim the whole `#[fprime_test(..)]`: rustc then reports every type error *about the
/// descriptor* on the attribute rather than on the expression that caused it, since the path
/// node begins with a token that points there. A channel passed where a command belongs, or a
/// `()` after a parameter, are the common ones.
///
/// The `.` is deliberate. Locating this on one of the author's idents would make
/// goto-definition on that ident answer with the invented tokens' definitions too — the
/// receiver once returned six targets that way. A punct has no definition to offer.
fn spanned_desc_path(segments: &[Ident], scaffold: Span) -> TokenStream {
    quote_spanned! { scaffold => crate::Desc #(:: #segments)* }
}

/// Pass two of `#[fprime_test]`: every dictionary point in a test body becomes a `Desc`
/// descriptor. A chain that is not a point — `t.initial_telemetry(..)`,
/// `event().containing(..)`, a local — is left alone.
///
/// `completing` says an editor is mid-keystroke somewhere in this body, which suppresses the
/// diagnostics a half-written one would otherwise earn. See `super::rewrite_test_body`.
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
        // A command *call* carries its arguments; anything else is bare. A channel or
        // parameter written with parentheses lands here too, and keeps them — that mistake is
        // the type error `spanned_desc_path` exists to report on the right token.
        let (Point::Command(command), Some(args)) = (&chain.point, &chain.args) else {
            // The only chance to keep a bare point mappable to what the author wrote.
            return Rewrite::Chain(spanned_desc_path(&chain.segments, chain.scaffold));
        };

        let original = chain.written_call();

        if let Some(encoded) = self
            .symbols
            .const_encoded_call(command, args, original.clone(), {
                // desc_path, not spanned_desc_path — this arm already emits the author's tokens
                // once, inside `if false { .. }`; duplicating spans would give goto two
                // definitions for one token.
                let path = desc_path(&chain.name);
                quote! {
                    {
                        // A const item of reference type is a const context, giving
                        // `&'static [u8]` without static promotion, and identical buffers still
                        // dedupe across call sites.
                        const __FPRIME_BUF: &[u8] = &__FPRIME_CMD;
                        #path.with(__FPRIME_BUF)
                    }
                }
            })
        {
            return Rewrite::Call(encoded);
        }

        // Unlike `#[fprime]`, where falling back on the accessor is the right answer — it
        // encodes the same command at run time — a test that wrote arguments asked to match
        // the exact bytes they encode to, and there are none to compute. Leaving the author's
        // tokens here resolves them to the *flight* accessor through the sequence crate's
        // glob, so the only thing reported is `CmdResponse: IntoCommand`, about neither the
        // argument nor the command.
        //
        // While an editor is mid-keystroke the reason is left out — but *only* the reason. The
        // author's tokens come first and identically either way, because the two expansions
        // rust-analyzer builds have to agree up to the cursor and may differ only after it.
        // `expansions_agree_up_to_the_cursor` measures that.
        let error = match self.completing {
            true => TokenStream::new(),
            false => self.symbols.why_not_const(command, args).to_compile_error(),
        };
        // `as u32`, as the generated `CmdDesc` does: the dictionary holds an opcode as a u64
        // and `FwOpcodeType` is narrower.
        let opcode = Literal::u64_unsuffixed(command.opcode);

        Rewrite::Call(quote! {
            {
                // Kept for type checking and for goto/hover on the command while the error is
                // on screen, as the const arm above does.
                if false { #original; }
                #error
                // A value of the right type, so the reason above is the only thing reported:
                // the opcode with any arguments, which is the weakest thing the author could
                // have meant.
                ::fprime_test::cmd(#opcode as u32)
            }
        })
    }
}

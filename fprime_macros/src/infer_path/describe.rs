//! A dictionary point becomes the `Desc` descriptor a test matches on.

use crate::infer_path::name::unraw;
use crate::infer_path::scan::{field_separator_at, ident_at, path_separator_at};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::naming::desc_path;
use proc_macro2::{Delimiter, Group, Ident, Literal, Span, TokenStream, TokenTree};
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

/// A dotted chain that resolved to a dictionary point.
struct DottedPoint {
    /// The dictionary's own dotted name.
    name: String,
    /// The author's own ident tokens per segment (spans included), not re-derived from
    /// `name` — see [`spanned_desc_path`].
    segments: Vec<Ident>,
    /// Span of the chain's first `.`, which is where the invented tokens go — see
    /// [`spanned_desc_path`].
    scaffold: Span,
    /// Index just past the chain, which is where its `( .. )` would be.
    end: usize,
}

impl Symbols {
    /// Rewrites every dictionary path in a test body into a `Desc` descriptor. A chain that
    /// is not a dictionary point (`t.initial_telemetry(..)`, `event().containing(..)`, a
    /// local) is left alone.
    ///
    /// `completing` suppresses the diagnostics a half-written body would otherwise earn —
    /// see [`rewrite_test_body`].
    pub(super) fn descriptor_calls(&self, tokens: TokenStream, completing: bool) -> TokenStream {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut out = TokenStream::new();
        let mut index = 0;

        while index < trees.len() {
            // Only at the start of a chain, so the `power` in `Ref.power.PWR_OFF` is never
            // mistaken for a root of its own.
            let begins_a_chain = match index.checked_sub(1).map(|previous| &trees[previous]) {
                Some(TokenTree::Punct(punct)) => !matches!(punct.as_char(), '.' | ':'),
                _ => true,
            };

            if begins_a_chain && let Some(found) = self.dotted_point(&trees, index) {
                let (rewritten, next) = self.descriptor_for(&trees, index, &found, completing);
                out.extend(rewritten);
                index = next;
                continue;
            }

            match &trees[index] {
                TokenTree::Group(group) => {
                    let mut descended = Group::new(
                        group.delimiter(),
                        self.descriptor_calls(group.stream(), completing),
                    );
                    descended.set_span(group.span());
                    out.extend([TokenTree::Group(descended)]);
                }
                tree => out.extend([tree.clone()]),
            }

            index += 1;
        }

        out
    }

    /// The descriptor expression for a resolved point, and the index just past what it
    /// consumed.
    fn descriptor_for(
        &self,
        trees: &[TokenTree],
        start: usize,
        found: &DottedPoint,
        completing: bool,
    ) -> (TokenStream, usize) {
        // A command *call* carries its arguments; a bare one does not.
        if let Some(Point::Command(command)) = self.point(&found.name)
            && let Some(TokenTree::Group(args)) = trees.get(found.end)
            && args.delimiter() == Delimiter::Parenthesis
        {
            let original: TokenStream = trees[start..=found.end].iter().cloned().collect();
            if let Some(encoded) = self.const_encoded_call(command, args, original.clone(), {
                // desc_path, not spanned_desc_path — this arm already emits the author's
                // tokens once, inside `if false { .. }`; duplicating spans would give goto
                // two definitions for one token.
                let path = desc_path(&found.name);
                quote! {
                    {
                        // A const item of reference type is a const context, giving
                        // `&'static [u8]` without static promotion, and identical buffers
                        // still dedupe across call sites.
                        const __FPRIME_BUF: &[u8] = &__FPRIME_CMD;
                        #path.with(__FPRIME_BUF)
                    }
                }
            }) {
                return (encoded, found.end + 1);
            }

            // Unlike `#[fprime_main]`, where falling back on the accessor is the right answer
            // — it encodes the same command at run time — a test that wrote arguments asked
            // to match the exact bytes they encode to, and there are none to compute. Leaving
            // the author's tokens here resolves them to the *flight* accessor through the
            // sequence crate's glob, so the only thing reported is
            // `CmdResponse: IntoCommand`, which is about neither the argument nor the
            // command.
            // A half-written argument is not a mistake to report, so while an editor is
            // mid-keystroke the reason is left out — but *only* the reason. The author's
            // tokens come first and identically either way, because the two expansions
            // rust-analyzer builds have to agree up to the cursor and may only differ after
            // it. Emitting the error ahead of them moved the offset it maps through, which
            // `expansions_agree_up_to_the_cursor` now measures.
            let error = match completing {
                true => TokenStream::new(),
                false => self.why_not_const(command, args).to_compile_error(),
            };
            // `as u32`, as the generated `CmdDesc` does: the dictionary holds an opcode as a
            // u64 and `FwOpcodeType` is narrower.
            let opcode = Literal::u64_unsuffixed(command.opcode);
            return (
                quote! {
                    {
                        // Kept for type checking and for goto/hover on the command while the
                        // error is on screen, as the const arm above does.
                        if false { #original; }
                        #error
                        // A value of the right type, so the reason above is the only thing
                        // reported: the opcode with any arguments, which is the weakest thing
                        // the author could have meant.
                        ::fprime_test::cmd(#opcode as u32)
                    }
                },
                found.end + 1,
            );
        }

        // A bare descriptor, e.g. `t.never_command(Ref.wasmSeq.LOAD)` — the only chance to
        // keep it mappable to what the author wrote.
        (
            spanned_desc_path(&found.segments, found.scaffold),
            found.end,
        )
    }

    /// Resolve the longest dotted chain starting at `start` to a dictionary point.
    fn dotted_point(&self, trees: &[TokenTree], start: usize) -> Option<DottedPoint> {
        let mut index = start;
        let mut root = ident_at(trees, index)?;

        index += 1;
        while let Some(segment) = path_separator_at(trees, index) {
            root = ident_at(trees, segment)?;
            index = segment + 1;
        }

        let mut parts = vec![unraw(root)];
        let mut segments = vec![root.clone()];
        let mut scaffold = None;
        while let Some(field) = field_separator_at(trees, index) {
            let segment = ident_at(trees, field)?;
            // The first `.`, kept as somewhere inside the chain to put the tokens the
            // expansion invents — see `spanned_desc_path`.
            scaffold = scaffold.or_else(|| Some(trees[index].span()));
            parts.push(unraw(segment));
            segments.push(segment.clone());
            index = field + 1;
        }

        // A single-segment name is never a point — treating one as such would rewrite the
        // author's own locals.
        if parts.len() < 2 {
            return None;
        }

        let name = parts.join(".");
        self.point(&name)?;
        Some(DottedPoint {
            name,
            segments,
            // A chain of two or more parts has a `.` in it, so this is always the author's.
            scaffold: scaffold.unwrap_or_else(Span::call_site),
            end: index,
        })
    }
}

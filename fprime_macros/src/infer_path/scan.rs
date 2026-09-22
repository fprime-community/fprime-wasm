//! Finding the dictionary names in a token stream.
//!
//! One traversal serves every pass, and it is the only place raw tokens are read. What
//! counts as a name is settled here once; what a pass does with one is [`Lower::chain`]'s
//! business.
//!
//! It works on tokens rather than a parsed body because a body being edited does not parse:
//! a sequence still qualifies around a missing `;`, a deleted argument still leaves its slot
//! empty rather than shifting later arguments onto earlier parameters, and an argument that
//! is not an expression is still passed through.

use crate::infer_path::name::dictionary_name;
use crate::infer_path::symbols::{Point, Symbols};
use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};

/// A dotted chain the author wrote, resolved to a dictionary point.
pub(super) struct Chain<'a> {
    /// The dictionary's own dotted name, e.g. `"Ref.power.PWR_OFF"`.
    pub(super) name: String,
    /// The author's own ident tokens, one per segment, spans included. Never re-derived from
    /// `name`: a minted ident has no span to offer, and `describe::spanned_desc_path` needs
    /// these to stay navigable.
    pub(super) segments: Vec<Ident>,
    /// The chain's first `.`. A pass puts the tokens it invents here, because a punct has no
    /// definition for goto to answer with — see `describe::spanned_desc_path`.
    pub(super) scaffold: Span,
    /// What the dictionary says this name is.
    pub(super) point: Point<'a>,
    /// The `( .. )` the author wrote after the chain, if there is one. Its absence is what
    /// tells a bare `Ref.power.PWR_OFF` from a call.
    pub(super) args: Option<Group>,
    /// The chain as written — the `::`-qualified receiver and every `.` included, which
    /// `segments` alone cannot reproduce.
    pub(super) written: TokenStream,
}

impl Chain<'_> {
    /// The chain and its arguments, exactly as written. What a pass keeps inside
    /// `if false { .. }` for type checking and for goto/hover on the command name.
    pub(super) fn written_call(&self) -> TokenStream {
        let mut out = self.written.clone();
        if let Some(args) = &self.args {
            out.extend([TokenTree::Group(args.clone())]);
        }

        out
    }
}

/// What a pass wants in a chain's place.
pub(super) enum Rewrite {
    /// These tokens for the chain alone. Anything after it — a `( .. )` included — is walked
    /// as ordinary tokens, which is what a bare descriptor needs and why a channel written
    /// with parentheses keeps them.
    Chain(TokenStream),
    /// These tokens for the chain *and* its argument group. Whatever the pass wanted from
    /// inside those arguments it has already done, so they are not walked again.
    Call(TokenStream),
    /// Not this pass's business. The chain is walked as ordinary tokens, so a command nested
    /// inside its arguments is still found.
    Skip,
}

/// One pass's answer for each name a scan resolves.
pub(super) trait Lower {
    fn chain(&self, chain: &Chain<'_>) -> Rewrite;
}

/// Offers `lower` every dictionary name in `tokens`, and copies everything else through.
pub(super) fn scan(symbols: &Symbols, tokens: TokenStream, lower: &impl Lower) -> TokenStream {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out = TokenStream::new();
    let mut index = 0;

    while index < trees.len() {
        if begins_a_chain(&trees, index)
            && let Some((chain, end)) = resolve(symbols, &trees, index)
            && let Some(next) = apply(&chain, end, lower.chain(&chain), &mut out)
        {
            index = next;
            continue;
        }

        match &trees[index] {
            // A command in a block, a closure or an argument is still a command.
            TokenTree::Group(group) => {
                let mut descended =
                    Group::new(group.delimiter(), scan(symbols, group.stream(), lower));
                descended.set_span(group.span());
                out.extend([TokenTree::Group(descended)]);
            }
            tree => out.extend([tree.clone()]),
        }

        index += 1;
    }

    out
}

/// Applies one [`Rewrite`], answering with the index to resume at — or `None` for a chain the
/// pass declined, which is then walked as ordinary tokens.
fn apply(chain: &Chain<'_>, end: usize, rewrite: Rewrite, out: &mut TokenStream) -> Option<usize> {
    match rewrite {
        Rewrite::Skip => None,
        Rewrite::Chain(tokens) => {
            out.extend(tokens);
            Some(end)
        }
        Rewrite::Call(tokens) => {
            out.extend(tokens);
            // A pass only answers `Call` for a chain that had arguments, but saying so here
            // rather than trusting it keeps the resume index right either way.
            Some(end + usize::from(chain.args.is_some()))
        }
    }
}

/// Whether the tree at `index` could begin a chain, rather than sitting inside one.
///
/// Only the token before it decides. A `.` or a `:` there means this ident is a segment of
/// something already being scanned, so the `power` in `Ref.power.PWR_OFF` is never mistaken
/// for a root of its own. The `:` covers `::`, which is two `Punct`s.
fn begins_a_chain(trees: &[TokenTree], index: usize) -> bool {
    match index.checked_sub(1).map(|previous| &trees[previous]) {
        Some(TokenTree::Punct(punct)) => !matches!(punct.as_char(), '.' | ':'),
        _ => true,
    }
}

/// The longest chain starting at `index`, if it names a dictionary point, with the index just
/// past the chain itself — which is where its `( .. )` would be.
fn resolve<'a>(
    symbols: &'a Symbols,
    trees: &[TokenTree],
    index: usize,
) -> Option<(Chain<'a>, usize)> {
    let mut at = index;
    let mut root = ident_at(trees, at)?;

    // A `::`-qualified receiver is skipped past, so `crate::Ref.dpDemo.CMD` resolves as
    // `Ref.dpDemo.CMD` does.
    at += 1;
    while let Some(segment) = path_separator_at(trees, at) {
        root = ident_at(trees, segment)?;
        at = segment + 1;
    }

    let mut parts = vec![dictionary_name(root)];
    let mut segments = vec![root.clone()];
    let mut scaffold = None;

    while let Some(field) = field_separator_at(trees, at) {
        let segment = ident_at(trees, field)?;
        scaffold = scaffold.or_else(|| Some(trees[at].span()));
        parts.push(dictionary_name(segment));
        segments.push(segment.clone());
        at = field + 1;
    }

    let name = parts.join(".");
    let point = symbols.point(&name)?;

    Some((
        Chain {
            name,
            segments,
            // `Symbols::point` refuses a name with no `.` in it, so the chain has one and
            // this is always a token the author wrote.
            scaffold: scaffold.unwrap_or_else(Span::call_site),
            point,
            args: parenthesised(trees, at),
            written: trees[index..at].iter().cloned().collect(),
        },
        at,
    ))
}

/// The `( .. )` at `index`, if that is what is there. A `{ .. }` or a `[ .. ]` is not an
/// argument list, and neither is a macro call's `!`.
fn parenthesised(trees: &[TokenTree], index: usize) -> Option<Group> {
    match trees.get(index) {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis => {
            Some(group.clone())
        }
        _ => None,
    }
}

/// One argument of a call, with the comma that ended it — kept so the rendered output has the
/// author's own separators back, trailing comma included.
pub(super) struct Argument {
    pub(super) tokens: TokenStream,
    pub(super) separator: Option<TokenTree>,
}

/// Splits an argument list on its commas, keeping them.
pub(super) fn arguments(tokens: TokenStream) -> Vec<Argument> {
    let mut arguments = Vec::new();
    let mut current = TokenStream::new();

    for tree in tokens {
        match &tree {
            TokenTree::Punct(punct) if punct.as_char() == ',' => {
                arguments.push(Argument {
                    tokens: std::mem::take(&mut current),
                    separator: Some(tree),
                });
            }
            _ => current.extend([tree]),
        }
    }

    // A trailing comma leaves no argument after it; an empty list has none at all.
    if !current.is_empty() {
        arguments.push(Argument {
            tokens: current,
            separator: None,
        });
    }

    arguments
}

fn ident_at(trees: &[TokenTree], index: usize) -> Option<&Ident> {
    match trees.get(index) {
        Some(TokenTree::Ident(ident)) => Some(ident),
        _ => None,
    }
}

/// Where the name after a `::` at `index` would be.
fn path_separator_at(trees: &[TokenTree], index: usize) -> Option<usize> {
    let (TokenTree::Punct(first), TokenTree::Punct(second)) =
        (trees.get(index)?, trees.get(index + 1)?)
    else {
        return None;
    };

    (first.as_char() == ':' && second.as_char() == ':').then_some(index + 2)
}

/// Where the name after a `.` at `index` would be.
fn field_separator_at(trees: &[TokenTree], index: usize) -> Option<usize> {
    let TokenTree::Punct(punct) = trees.get(index)? else {
        return None;
    };

    (punct.as_char() == '.').then_some(index + 1)
}

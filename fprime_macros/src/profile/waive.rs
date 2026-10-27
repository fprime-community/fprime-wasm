//! `#[rule_waive("why")]`: accepting one node a profile would reject, with the reason on
//! record beside it.
//!
//! Stable Rust allows an attribute on a statement or an expression only if a macro removes
//! it before rustc sees it, which [`strip`] does once the check has read it.

use proc_macro2::{Delimiter, Group, Span, TokenStream, TokenTree};
use syn::{Attribute, LitStr};

const NAME: &str = "rule_waive";

pub(crate) struct Waiver {
    pub(crate) span: Span,
    pub(crate) reason: String,
}

/// The waiver among `attrs`, if any. A node takes one: two would each claim the same errors.
pub(crate) fn parse(attrs: &[Attribute]) -> Option<syn::Result<Waiver>> {
    let mut waivers = attrs.iter().filter(|attr| attr.path().is_ident(NAME));
    let first = waivers.next()?;

    if let Some(second) = waivers.next() {
        return Some(Err(syn::Error::new_spanned(
            second,
            "one `#[rule_waive]` per node: put every reason in one message",
        )));
    }

    Some(waiver(first))
}

fn waiver(attr: &Attribute) -> syn::Result<Waiver> {
    let usage = || syn::Error::new_spanned(attr, "expected `#[rule_waive(\"why this is safe\")]`");

    let reason: LitStr = attr.parse_args().map_err(|_| usage())?;
    if reason.value().trim().is_empty() {
        return Err(usage());
    }

    // Spans don't join on stable, so the whole attribute would be underlined as just its `#`.
    Ok(Waiver {
        span: reason.span(),
        reason: reason.value(),
    })
}

/// `tokens` without any `#[rule_waive(..)]`, everything else and every span untouched.
pub(crate) fn strip(tokens: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out = TokenStream::new();
    let mut index = 0;

    while index < trees.len() {
        if is_waiver(&trees, index) {
            index += 2;
            continue;
        }

        match &trees[index] {
            TokenTree::Group(group) => {
                let mut stripped = Group::new(group.delimiter(), strip(group.stream()));
                stripped.set_span(group.span());
                out.extend([TokenTree::Group(stripped)]);
            }
            tree => out.extend([tree.clone()]),
        }
        index += 1;
    }

    out
}

/// Where the first waiver in `tokens` is, if there is one.
pub(crate) fn find(tokens: &TokenStream) -> Option<Span> {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();

    (0..trees.len()).find_map(|index| match &trees[index] {
        _ if is_waiver(&trees, index) => Some(trees[index + 1].span()),
        TokenTree::Group(group) => find(&group.stream()),
        _ => None,
    })
}

/// Whether a `#` at `index` begins `#[rule_waive ..]`.
fn is_waiver(trees: &[TokenTree], index: usize) -> bool {
    let (Some(TokenTree::Punct(hash)), Some(TokenTree::Group(group))) =
        (trees.get(index), trees.get(index + 1))
    else {
        return false;
    };

    hash.as_char() == '#'
        && group.delimiter() == Delimiter::Bracket
        && matches!(group.stream().into_iter().next(), Some(TokenTree::Ident(name)) if name == NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn strip_removes_only_waivers_at_any_depth() {
        let stripped = strip(quote! {
            #[rule_waive("a")]
            loop { let x = f(#[rule_waive("b")] y); }
            #[allow(unused)]
            let z = 0;
        });

        assert_eq!(
            stripped.to_string(),
            quote! { loop { let x = f(y); } #[allow(unused)] let z = 0; }.to_string()
        );
    }
}

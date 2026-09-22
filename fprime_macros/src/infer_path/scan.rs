//! Finding the dictionary names in a token stream: the only place raw tokens are read.

use crate::infer_path::name::unraw;
use crate::infer_path::symbols::Symbols;
use fprime_dictionary::Command;
use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};

/// A dotted chain that resolved to a command, and where its `( .. )` sits.
pub(super) struct CommandCall<'a> {
    pub(super) command: &'a Command,
    /// Index of the argument group, so a caller can both read it and resume past it.
    pub(super) args: usize,
}

/// One argument of a call, with the comma that ended it — kept so the rendered output has
/// the author's own separators back, trailing comma included.
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

pub(super) fn ident_at(trees: &[TokenTree], index: usize) -> Option<&Ident> {
    match trees.get(index) {
        Some(TokenTree::Ident(ident)) => Some(ident),
        _ => None,
    }
}

/// Where the name after a `::` at `index` would be.
pub(super) fn path_separator_at(trees: &[TokenTree], index: usize) -> Option<usize> {
    let (TokenTree::Punct(first), TokenTree::Punct(second)) =
        (trees.get(index)?, trees.get(index + 1)?)
    else {
        return None;
    };

    (first.as_char() == ':' && second.as_char() == ':').then_some(index + 2)
}

/// Where the name after a `.` at `index` would be.
pub(super) fn field_separator_at(trees: &[TokenTree], index: usize) -> Option<usize> {
    let TokenTree::Punct(punct) = trees.get(index)? else {
        return None;
    };

    (punct.as_char() == '.').then_some(index + 1)
}

impl Symbols {
    pub(super) fn command_call(
        &self,
        trees: &[TokenTree],
        start: usize,
    ) -> Option<CommandCall<'_>> {
        let mut index = start;
        let mut root = ident_at(trees, index)?;

        index += 1;
        while let Some(segment) = path_separator_at(trees, index) {
            root = ident_at(trees, segment)?;
            index = segment + 1;
        }

        let mut parts = vec![unraw(root)];
        while let Some(field) = field_separator_at(trees, index) {
            parts.push(unraw(ident_at(trees, field)?));
            index = field + 1;
        }

        let TokenTree::Group(args) = trees.get(index)? else {
            return None;
        };

        if args.delimiter() != Delimiter::Parenthesis {
            return None;
        }

        Some(CommandCall {
            command: self.command(&parts.join("."))?,
            args: index,
        })
    }
}

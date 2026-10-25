//! Bare dictionary names become `crate::Defs::..` paths.

use crate::infer_path::name::{declared_in, unraw, written_path};
use crate::infer_path::scan::{Chain, Lower, Rewrite, arguments, scan};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::konst::MAX_DEPTH;
use fprime_dictionary::naming::{definition_path, definition_path_with_name, split_qualified_name};
use fprime_dictionary::{Command, TypeDefinition, TypeName};
use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Expr, ExprBlock, ExprPath, ExprStruct, Member};

/// Replaces a struct path with its dictionary path, e.g. `ChoicePair` or `Ref::ChoicePair`
/// -> `crate::Defs::Ref::ChoicePair`.
fn qualify_struct_path(literal: &mut ExprStruct, qualified_name: &str) {
    let Some(name) = written_path(literal.qself.as_ref(), &literal.path) else {
        return;
    };

    let (_, short_name) = split_qualified_name(qualified_name);
    if !name.names(&qualified_name.split('.').collect::<Vec<_>>()) {
        return;
    }

    literal.path = parse(definition_path_with_name(
        qualified_name,
        name.emit(short_name),
    ));
}

/// `{ use <enum>::*; NAME }`, for a bare name in an enum argument that is none of its
/// constants.
///
/// A local or a constant of the author's still resolves as it did, since the glob only adds
/// names and a name that was one of these constants was qualified instead. A misspelt constant
/// now earns rustc's "a unit variant with a similar name exists" pointing into the enum, rather
/// than a bare "cannot find value". It also puts the constants in scope for an editor
/// completing a prefix, which then offers them by their bare names.
///
/// Whether to wrap depends only on the stripped name, so the two expansions rust-analyzer
/// builds wrap alike — see `name::dictionary_name`.
fn with_constants_in_scope(written: &mut ExprPath, enum_name: &str) -> Expr {
    let enum_path = definition_path(enum_name);

    // An attribute stays on the argument, as it does when the name is qualified instead.
    let attrs = std::mem::take(&mut written.attrs);
    let mut scoped: ExprBlock = parse(quote! {
        {
            #[allow(unused_imports)]
            use #enum_path::*;
            #written
        }
    });
    scoped.attrs = attrs;

    Expr::Block(scoped)
}

fn parse<T: syn::parse::Parse>(tokens: TokenStream) -> T {
    syn::parse2(tokens).expect("generated dictionary path should parse")
}

/// Pass one of both pipelines: the bare dictionary names in `tokens` become
/// `crate::Defs::..` paths.
///
/// It has to run before either pass two. A constant is only recognisable as one once
/// qualified, and a call that has already been rewritten is no longer in a command position
/// for qualification to find — see `.notes/const-command-encoding.md`.
pub(super) fn qualify(symbols: &Symbols, tokens: TokenStream) -> TokenStream {
    scan(symbols, tokens, &Qualify { symbols })
}

struct Qualify<'a> {
    symbols: &'a Symbols,
}

impl Lower for Qualify<'_> {
    fn chain(&self, chain: &Chain<'_>) -> Rewrite {
        // Only a command has formal parameters to qualify its arguments against, and only a
        // call has arguments. Everything else is walked on, so a command nested inside a
        // channel read is still found.
        let (Point::Command(command), Some(args)) = (&chain.point, &chain.args) else {
            return Rewrite::Skip;
        };

        let mut out = chain.written.clone();
        out.extend([TokenTree::Group(self.symbols.qualify_args(args, command))]);

        Rewrite::Call(out)
    }
}

impl Symbols {
    fn qualify_args(&self, args: &Group, command: &Command) -> Group {
        let mut rewritten = TokenStream::new();

        for (position, argument) in arguments(args.stream()).into_iter().enumerate() {
            let written = qualify(self, argument.tokens);

            let expected = command.formal_params.get(position).and_then(|param| {
                syn::parse2::<Expr>(written.clone())
                    .ok()
                    .map(|expr| (param, expr))
            });

            match expected {
                Some((param, mut expr)) => {
                    self.qualify_value(&mut expr, &param.type_name, 0);
                    rewritten.extend(expr.into_token_stream());
                }
                None => rewritten.extend(written),
            }

            rewritten.extend(argument.separator);
        }

        let mut qualified = Group::new(args.delimiter(), rewritten);
        qualified.set_span(args.span());
        qualified
    }

    /// Qualifies one value against its expected dictionary type; left untouched if the
    /// dictionary has nothing to say about it.
    fn qualify_value(&self, expr: &mut Expr, expected: &TypeName, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }

        match self.dictionary.definition(expected) {
            Some(TypeDefinition::Enum(ty)) => {
                let Expr::Path(original) = expr else {
                    return;
                };

                let Some(name) = written_path(original.qself.as_ref(), &original.path) else {
                    return;
                };

                let Some(constant) = ty
                    .enumerated_constants
                    .iter()
                    .find(|c| c.name == name.leaf())
                else {
                    // Nothing typed yet is left bare, as is a qualified path, which is not a
                    // misspelt constant of this enum.
                    if original.path.segments.len() == 1 && !name.leaf().is_empty() {
                        *expr = with_constants_in_scope(original, &ty.qualified_name);
                    }
                    return;
                };

                // `Choice::RED` and `Ref::Choice::RED` are the same constant; a qualifier
                // that is not the enum's own is some other name entirely.
                if !name.names(&declared_in(&ty.qualified_name, &constant.name)) {
                    return;
                }

                let constant = name.emit(&constant.name);
                let enum_path = definition_path(&ty.qualified_name);

                let attrs = std::mem::take(&mut original.attrs);
                let mut qualified: ExprPath = parse(quote! { #enum_path::#constant });
                qualified.attrs = attrs;

                *expr = Expr::Path(qualified);
            }
            Some(TypeDefinition::Struct(ty)) => {
                let Expr::Struct(literal) = expr else {
                    return;
                };

                qualify_struct_path(literal, &ty.qualified_name);

                for field in literal.fields.iter_mut() {
                    let Member::Named(ident) = &field.member else {
                        continue;
                    };

                    let name = unraw(ident);
                    let Some(member) = ty.members.iter().find(|member| member.name == name) else {
                        continue;
                    };

                    match member.size {
                        None => self.qualify_value(&mut field.expr, &member.type_name, depth + 1),
                        Some(_) => self.qualify_elements(&mut field.expr, &member.type_name, depth),
                    }
                }
            }
            Some(TypeDefinition::Array(ty)) => self.qualify_elements(expr, &ty.element_type, depth),
            _ => {}
        }
    }

    /// Qualify every element of an array literal, or the repeated element of `[x; n]`.
    fn qualify_elements(&self, expr: &mut Expr, element: &TypeName, depth: usize) {
        match expr {
            Expr::Array(array) => {
                for element_expr in array.elems.iter_mut() {
                    self.qualify_value(element_expr, element, depth + 1);
                }
            }
            Expr::Repeat(repeat) => self.qualify_value(&mut repeat.expr, element, depth + 1),
            _ => {}
        }
    }
}

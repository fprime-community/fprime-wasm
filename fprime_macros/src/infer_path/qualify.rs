//! Bare dictionary names become `crate::Dict::..` paths.

use crate::infer_path::name::{declared_in, unraw, written_path};
use crate::infer_path::scan::{Chain, Lower, Rewrite, arguments, scan};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::naming::{definition_path, definition_path_with_name};
use fprime_dictionary::{Command, TypeDefinition, TypeName};
use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Expr, ExprBlock, ExprPath, ExprStruct, Member};

/// Replaces a struct path with its dictionary path, e.g. `ChoicePair` or `Ref::ChoicePair`
/// -> `crate::Dict::Ref::ChoicePair`.
fn qualify_struct_path(literal: &mut ExprStruct, qualified_name: &str) {
    let Some(name) = written_path(literal.qself.as_ref(), &literal.path) else {
        return;
    };

    if !name.names(&qualified_name.split('.').collect::<Vec<_>>()) {
        return;
    }

    literal.path = parse(definition_path_with_name(qualified_name, name.emit()));
}

/// `{ use <enum>::*; NAME }`, for a bare name in an enum argument that is none of its
/// constants.
///
/// A local or a constant of the author's still resolves, since the glob only adds names. A
/// misspelt constant earns rustc's "a unit variant with a similar name exists" pointing into
/// the enum, and an editor completing a prefix offers the constants by their bare names.
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
/// `crate::Dict::..` paths.
///
/// It runs first because a constant is only recognisable as one once qualified, and a call
/// rewritten by a later pass is no longer in a command position for this one to find.
pub(super) fn qualify(symbols: &Symbols, tokens: TokenStream) -> TokenStream {
    scan(symbols, tokens, &Qualify { symbols })
}

struct Qualify<'a> {
    symbols: &'a Symbols,
}

impl Lower for Qualify<'_> {
    fn chain(&self, chain: &Chain<'_>) -> Rewrite {
        // Only a command call has arguments to qualify. Skipping anything else still walks
        // it, so a command nested inside is found.
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
                    self.qualify_value(&mut expr, &param.type_name);
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
    ///
    /// Recursion follows `expr`, which is finite, so it needs no depth limit of its own.
    fn qualify_value(&self, expr: &mut Expr, expected: &TypeName) {
        match self.dictionary.definition(expected) {
            Some(TypeDefinition::Enum(ty)) => {
                let Expr::Path(original) = expr else {
                    return;
                };

                let Some(name) = written_path(original.qself.as_ref(), &original.path) else {
                    return;
                };

                let is_constant = ty
                    .enumerated_constants
                    .iter()
                    .any(|constant| constant.name == name.leaf());

                if !is_constant {
                    // Nothing typed yet is left bare, as is a qualified path, which is not a
                    // misspelt constant of this enum.
                    if original.path.segments.len() == 1 && !name.leaf().is_empty() {
                        *expr = with_constants_in_scope(original, &ty.qualified_name);
                    }
                    return;
                }

                // `Choice::RED` and `Ref::Choice::RED` are the same constant; a qualifier
                // that is not the enum's own is some other name entirely.
                if !name.names(&declared_in(&ty.qualified_name, name.leaf())) {
                    return;
                }

                let constant = name.emit();
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
                        None => self.qualify_value(&mut field.expr, &member.type_name),
                        Some(_) => self.qualify_elements(&mut field.expr, &member.type_name),
                    }
                }
            }
            Some(TypeDefinition::Array(ty)) => self.qualify_elements(expr, &ty.element_type),
            _ => {}
        }
    }

    /// Qualify every element of an array literal, or the repeated element of `[x; n]`.
    fn qualify_elements(&self, expr: &mut Expr, element: &TypeName) {
        match expr {
            Expr::Array(array) => {
                for element_expr in array.elems.iter_mut() {
                    self.qualify_value(element_expr, element);
                }
            }
            Expr::Repeat(repeat) => self.qualify_value(&mut repeat.expr, element),
            _ => {}
        }
    }
}

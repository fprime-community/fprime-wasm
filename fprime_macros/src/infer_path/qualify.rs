//! Bare dictionary names become `crate::Defs::..` paths.

use crate::infer_path::MAX_DEPTH;
use crate::infer_path::name::{declared_in, unraw, written_path};
use crate::infer_path::scan::arguments;
use crate::infer_path::symbols::Symbols;
use fprime_dictionary::naming::{definition_path, definition_path_with_name, split_qualified_name};
use fprime_dictionary::{Command, TypeDefinition, TypeName};
use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Expr, ExprPath, ExprStruct, Member};

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

fn parse<T: syn::parse::Parse>(tokens: TokenStream) -> T {
    syn::parse2(tokens).expect("generated dictionary path should parse")
}

impl Symbols {
    pub(super) fn qualify_calls(&self, tokens: TokenStream) -> TokenStream {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut qualified = TokenStream::new();
        let mut index = 0;

        while index < trees.len() {
            let begins_a_chain = match index.checked_sub(1).map(|previous| &trees[previous]) {
                Some(TokenTree::Punct(punct)) => !matches!(punct.as_char(), '.' | ':'),
                _ => true,
            };

            if begins_a_chain
                && let Some(call) = self.command_call(&trees, index)
                && let Some(TokenTree::Group(args)) = trees.get(call.args)
            {
                qualified.extend(trees[index..call.args].iter().cloned());
                qualified.extend([TokenTree::Group(self.qualify_args(args, call.command))]);
                index = call.args + 1;
                continue;
            }

            match &trees[index] {
                // A command in a block, a closure or an argument is still a command.
                TokenTree::Group(group) => {
                    let mut descended =
                        Group::new(group.delimiter(), self.qualify_calls(group.stream()));
                    descended.set_span(group.span());
                    qualified.extend([TokenTree::Group(descended)]);
                }
                tree => qualified.extend([tree.clone()]),
            }

            index += 1;
        }

        qualified
    }

    fn qualify_args(&self, args: &Group, command: &Command) -> Group {
        let mut rewritten = TokenStream::new();

        for (position, argument) in arguments(args.stream()).into_iter().enumerate() {
            let written = self.qualify_calls(argument.tokens);

            let expected = command.formal_params.get(position).and_then(|param| {
                syn::parse2::<Expr>(written.clone())
                    .ok()
                    .map(|expr| (param, expr))
            });

            match expected {
                Some((param, mut expr)) => {
                    self.qualify(&mut expr, &param.type_name, 0);
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

    /// Qualifies `expr` against its expected dictionary type; left untouched if the
    /// dictionary has nothing to say about it.
    fn qualify(&self, expr: &mut Expr, expected: &TypeName, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }

        match self.definition(expected) {
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
                        None => self.qualify(&mut field.expr, &member.type_name, depth + 1),
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
                    self.qualify(element_expr, element, depth + 1);
                }
            }
            Expr::Repeat(repeat) => self.qualify(&mut repeat.expr, element, depth + 1),
            _ => {}
        }
    }
}

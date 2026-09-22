//! A call whose arguments are all constants becomes its `Fw::ComBuffer`.

use crate::infer_path::MAX_DEPTH;
use crate::infer_path::name::unraw;
use crate::infer_path::scan::arguments;
use crate::infer_path::symbols::Symbols;
use fprime_dictionary::konst::{ENCODE_SUFFIX, SIZE_SUFFIX};
use fprime_dictionary::naming::{konst_path, split_qualified_name};
use fprime_dictionary::{Command, TypeDefinition, TypeName};
use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Expr, Lit, Member, Path as SynPath, UnOp};

/// `Ok` when the shape held, and why it did not otherwise.
fn require(held: bool, expr: &Expr, reason: impl std::fmt::Display) -> syn::Result<()> {
    if held {
        return Ok(());
    }

    Err(syn::Error::new_spanned(expr, reason))
}

/// `was` or `were`, so a count reads as a sentence.
fn were(count: usize) -> &'static str {
    if count == 1 { "was" } else { "were" }
}

/// `, like `RED` or `BLUE`` — enough of a list to show what one looks like, and never the
/// whole of an enum with two hundred constants in it.
fn for_example<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let shown: Vec<String> = names.take(2).map(|name| format!("`{name}`")).collect();

    match shown.len() {
        0 => String::new(),
        _ => format!(", like {}", shown.join(" or ")),
    }
}

/// How the dictionary spells a type, for a message.
fn type_label(ty: &TypeName) -> String {
    match ty {
        TypeName::Integer { name } => format!("{name:?}"),
        TypeName::Float { name } => format!("{name:?}"),
        TypeName::Bool => "bool".to_string(),
        TypeName::String { size } => format!("string size {size}"),
        TypeName::QualifiedIdentifier { name } => name.clone(),
    }
}

/// The two ways a test can match a command whose arguments it cannot write out, appended to
/// every reason [`Symbols::why_not_const`] gives.
fn matching_alternatives(command: &Command) -> String {
    format!(
        "To match {name} whatever its arguments, leave the parentheses off:\n    \
         t.expect_command({name})\n\
         To match only the first bytes of its arguments:\n    \
         t.expect_command(cmd({opcode:#010x}).args_starting(&[..]))",
        name = command.name,
        opcode = command.opcode,
    )
}

/// A numeric literal, or a negated one: `-1` parses as a unary negation.
fn is_numeric_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(lit) => matches!(lit.lit, Lit::Int(_) | Lit::Float(_)),
        Expr::Unary(unary) => matches!(unary.op, UnOp::Neg(_)) && is_numeric_literal(&unary.expr),
        _ => false,
    }
}

/// Whether `expr` is a path this pass qualified (rooted at `crate`).
fn is_qualified_definition(expr: &Expr) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };

    path.qself.is_none() && is_qualified_path(&path.path)
}

/// Whether `path` is rooted at `crate` — the shape this pass writes.
fn is_qualified_path(path: &SynPath) -> bool {
    path.segments.len() > 1
        && path
            .segments
            .first()
            .is_some_and(|segment| segment.ident == "crate")
}

impl Symbols {
    /// Replaces every command call whose arguments are all compile-time constants with a
    /// reference to its encoded `Fw::ComBuffer`.
    pub(super) fn const_encode_calls(&self, tokens: TokenStream) -> TokenStream {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut encoded = TokenStream::new();
        let mut index = 0;

        while index < trees.len() {
            let begins_a_chain = match index.checked_sub(1).map(|previous| &trees[previous]) {
                Some(TokenTree::Punct(punct)) => !matches!(punct.as_char(), '.' | ':'),
                _ => true,
            };

            if begins_a_chain
                && let Some(call) = self.command_call(&trees, index)
                && let Some(TokenTree::Group(args)) = trees.get(call.args)
                && let Some(buffer) = self.const_encoded_call(
                    call.command,
                    args,
                    trees[index..=call.args].iter().cloned().collect(),
                    quote! { unsafe { fprime_core::command(&__FPRIME_CMD) } },
                )
            {
                encoded.extend(buffer);
                index = call.args + 1;
                continue;
            }

            match &trees[index] {
                TokenTree::Group(group) => {
                    let mut descended =
                        Group::new(group.delimiter(), self.const_encode_calls(group.stream()));
                    descended.set_span(group.span());
                    encoded.extend([TokenTree::Group(descended)]);
                }
                tree => encoded.extend([tree.clone()]),
            }

            index += 1;
        }

        encoded
    }

    /// The const-encoded form of `command` applied to `args`, or `None` for the runtime
    /// `__SCRATCH` path.
    pub(super) fn const_encoded_call(
        &self,
        command: &Command,
        args: &Group,
        original: TokenStream,
        tail: TokenStream,
    ) -> Option<TokenStream> {
        if !self.dictionary.const_encodable(command) {
            return None;
        }

        let parsed = arguments(args.stream());

        // Wrong arity is left for the accessor to report.
        if parsed.len() != command.formal_params.len() {
            return None;
        }

        for (param, argument) in command.formal_params.iter().zip(&parsed) {
            let expr = syn::parse2::<Expr>(argument.tokens.clone()).ok()?;

            if !self.is_const_argument(&expr, &param.type_name) {
                return None;
            }
        }

        let size = konst_path(&command.name, SIZE_SUFFIX);
        let encode = konst_path(&command.name, ENCODE_SUFFIX);
        let args = args.stream();

        // `#original` is kept for type checking and goto/hover on the command name, not
        // behaviour. `#tail` is what `#[fprime_main]` (dispatch) and `#[fprime_test]`
        // (descriptor) differ in.
        Some(quote! {
            {
                const __FPRIME_LEN: usize = #size(#args);
                const __FPRIME_CMD: [u8; __FPRIME_LEN] = #encode::<__FPRIME_LEN>(#args);

                if false {
                    #original;
                }

                #tail
            }
        })
    }

    /// Whether `expr` may appear in the `const` initialiser of a const-encoded
    /// call.
    fn is_const_argument(&self, expr: &Expr, expected: &TypeName) -> bool {
        // No name for the value, since the reason is thrown away: this is the `#[fprime_main]`
        // side, which only decides between the `const` buffer and the run-time accessor.
        self.check_const_argument(expr, expected, "").is_ok()
    }

    /// The same question, answered with the reason when the answer is no. `#[fprime_main]`
    /// only needs the yes or no — a call it cannot fold goes to the run-time accessor — but
    /// `#[fprime_test]` has to say why, since for a test there is no fallback.
    ///
    /// `at` is what to call the value in that reason: the formal parameter's name, extended a
    /// member or an index at a time on the way down, so a reason about a nested value says
    /// `choices.choicePair.firstChoice` rather than `choices`.
    fn check_const_argument(&self, expr: &Expr, expected: &TypeName, at: &str) -> syn::Result<()> {
        self.check_const_argument_at(expr, expected, at, 0)
    }

    fn check_const_argument_at(
        &self,
        expr: &Expr,
        expected: &TypeName,
        at: &str,
        depth: usize,
    ) -> syn::Result<()> {
        if depth > MAX_DEPTH {
            return Err(syn::Error::new_spanned(
                expr,
                format!("`{at}` nests too deeply to work out its bytes"),
            ));
        }

        match self.dictionary.resolved_type(expected) {
            Some(TypeName::String { .. }) => require(
                matches!(expr, Expr::Lit(lit) if matches!(lit.lit, Lit::Str(_))),
                expr,
                format!("`{at}` has to be a string written out here, like \"sequencer.wasm\""),
            ),
            Some(TypeName::Bool) => require(
                matches!(expr, Expr::Lit(lit) if matches!(lit.lit, Lit::Bool(_))),
                expr,
                format!("`{at}` has to be `true` or `false`"),
            ),
            Some(TypeName::Integer { .. } | TypeName::Float { .. }) => require(
                is_numeric_literal(expr),
                expr,
                format!("`{at}` has to be a number written out here, like `0`"),
            ),
            Some(TypeName::QualifiedIdentifier { name }) => {
                match self.dictionary.type_definitions.get(name) {
                    // Every constant this pass recognised is already `crate`-rooted, so
                    // anything else is a name the dictionary does not have.
                    Some(TypeDefinition::Enum(ty)) => require(
                        is_qualified_definition(expr),
                        expr,
                        format!(
                            "`{at}` has to be one of {}'s constants{}",
                            ty.qualified_name,
                            for_example(ty.enumerated_constants.iter().map(|c| c.name.as_str()))
                        ),
                    ),
                    Some(TypeDefinition::Array(ty)) => {
                        self.check_const_elements(expr, &ty.element_type, ty.size, at, depth)
                    }
                    Some(TypeDefinition::Struct(ty)) => {
                        self.check_const_struct(expr, ty, at, depth)
                    }
                    _ => Err(syn::Error::new_spanned(
                        expr,
                        format!("the dictionary does not define {name}, which is what `{at}` is"),
                    )),
                }
            }
            _ => Err(syn::Error::new_spanned(
                expr,
                format!("`{at}` cannot be written out as a constant"),
            )),
        }
    }

    /// A struct literal naming the dictionary's own type, with every member written out.
    fn check_const_struct(
        &self,
        expr: &Expr,
        ty: &fprime_dictionary::StructType,
        at: &str,
        depth: usize,
    ) -> syn::Result<()> {
        let (_, short_name) = split_qualified_name(&ty.qualified_name);

        let Expr::Struct(literal) = expr else {
            return Err(syn::Error::new_spanned(
                expr,
                format!("`{at}` has to be a `{short_name} {{ .. }}` literal"),
            ));
        };

        // A literal with `..rest` can't be evaluated.
        if let Some(rest) = &literal.rest {
            return Err(syn::Error::new_spanned(
                rest,
                format!(
                    "every member of `{at}` has to be written out here, so `..` cannot stand in \
                     for the rest"
                ),
            ));
        }

        if !is_qualified_path(&literal.path) {
            return Err(syn::Error::new_spanned(
                &literal.path,
                format!(
                    "`{at}` is a {}, so write it as `{short_name} {{ .. }}`",
                    ty.qualified_name
                ),
            ));
        }

        if ty.members.len() != literal.fields.len() {
            return Err(syn::Error::new_spanned(
                literal,
                format!(
                    "`{at}` has {} members, but {} {} written",
                    ty.members.len(),
                    literal.fields.len(),
                    were(literal.fields.len()),
                ),
            ));
        }

        // Every member has to be given, and given a constant.
        for member in &ty.members {
            let field = literal.fields.iter().find(
                |field| matches!(&field.member, Member::Named(name) if unraw(name) == member.name),
            );

            let Some(field) = field else {
                return Err(syn::Error::new_spanned(
                    literal,
                    format!("`{at}` has a member `{}`, which is missing", member.name),
                ));
            };

            let at = format!("{at}.{}", member.name);
            match member.size {
                None => {
                    self.check_const_argument_at(&field.expr, &member.type_name, &at, depth + 1)?
                }
                Some(size) => {
                    self.check_const_elements(&field.expr, &member.type_name, size, &at, depth)?
                }
            }
        }

        Ok(())
    }

    /// An array of exactly `size` constants, as a list or `[x; n]` repeat.
    fn check_const_elements(
        &self,
        expr: &Expr,
        element: &TypeName,
        size: u32,
        at: &str,
        depth: usize,
    ) -> syn::Result<()> {
        match expr {
            Expr::Array(array) => {
                if array.elems.len() != size as usize {
                    return Err(syn::Error::new_spanned(
                        array,
                        format!(
                            "`{at}` holds {size} values, but {} {} written",
                            array.elems.len(),
                            were(array.elems.len()),
                        ),
                    ));
                }

                for (index, elem) in array.elems.iter().enumerate() {
                    let at = format!("{at}[{index}]");
                    self.check_const_argument_at(elem, element, &at, depth + 1)?;
                }
                Ok(())
            }
            Expr::Repeat(repeat) => {
                // The length has to be a plain literal for us to know it matches.
                let written = match repeat.len.as_ref() {
                    Expr::Lit(lit) => match &lit.lit {
                        Lit::Int(len) => len.base10_parse::<u32>().ok(),
                        _ => None,
                    },
                    _ => None,
                };

                if written != Some(size) {
                    return Err(syn::Error::new_spanned(
                        &repeat.len,
                        format!("`{at}` holds {size} values, so the length here has to be {size}"),
                    ));
                }

                self.check_const_argument_at(&repeat.expr, element, at, depth + 1)
            }
            _ => Err(syn::Error::new_spanned(
                expr,
                format!("`{at}` has to be a list of {size} values, written out here"),
            )),
        }
    }

    /// Why a command call in a test body cannot be matched by its exact argument bytes.
    ///
    /// Makes the same checks as [`Symbols::const_encoded_call`], in the same order, so this
    /// is always the one that made it give up.
    pub(super) fn why_not_const(&self, command: &Command, args: &Group) -> syn::Error {
        let help = matching_alternatives(command);
        let at = |span: Span, reason: &str| syn::Error::new(span, format!("{reason}\n\n{help}"));

        let unencodable = command
            .formal_params
            .iter()
            .find(|param| self.dictionary.const_arg(&param.type_name).is_none());

        if let Some(param) = unencodable {
            return at(
                args.span(),
                &format!(
                    "`{}` is a {}, which a test cannot write out as bytes",
                    param.name,
                    type_label(&param.type_name),
                ),
            );
        }

        let parsed = arguments(args.stream());
        if parsed.len() != command.formal_params.len() {
            return at(
                args.span(),
                &format!(
                    "{} takes {} arguments, but {} {} written",
                    command.name,
                    command.formal_params.len(),
                    parsed.len(),
                    were(parsed.len()),
                ),
            );
        }

        for (param, argument) in command.formal_params.iter().zip(&parsed) {
            let Ok(expr) = syn::parse2::<Expr>(argument.tokens.clone()) else {
                return at(
                    argument.tokens.span(),
                    &format!("`{}` is not something a test can read", param.name),
                );
            };

            if let Err(err) = self.check_const_argument(&expr, &param.type_name, &param.name) {
                return at(err.span(), &err.to_string());
            }
        }

        // Unreachable: every check `const_encoded_call` makes has just passed.
        at(
            args.span(),
            "this command cannot be matched by its exact arguments",
        )
    }
}

//! A call whose arguments are all constants becomes its `Fw::ComBuffer`.

use crate::infer_path::name::unraw;
use crate::infer_path::scan::{Chain, Lower, Rewrite, arguments, scan};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::konst::{ENCODE_SUFFIX, SIZE_SUFFIX};
use fprime_dictionary::naming::{konst_path, split_qualified_name, type_label};
use fprime_dictionary::{Command, TypeDefinition, TypeName};
use proc_macro2::{Group, Span, TokenStream};
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

/// The two ways a test can match a command whose arguments it cannot write out, appended to
/// every reason [`Symbols::buffer`] refuses with.
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

/// Pass two of `#[fprime]`: every command call whose arguments are all compile-time
/// constants becomes a reference to its encoded `Fw::ComBuffer`.
pub(super) fn encode(symbols: &Symbols, tokens: TokenStream) -> TokenStream {
    scan(symbols, tokens, &Encode { symbols })
}

struct Encode<'a> {
    symbols: &'a Symbols,
}

impl Lower for Encode<'_> {
    fn chain(&self, chain: &Chain<'_>) -> Rewrite {
        let (Point::Command(command), Some(args)) = (&chain.point, &chain.args) else {
            return Rewrite::Skip;
        };

        match self.symbols.buffer(command, args) {
            Ok(buffer) => Rewrite::Call(buffer.block(
                &chain.written_call(),
                quote! { unsafe { fprime_core::command(&__FPRIME_CMD) } },
            )),
            // Left to the run-time accessor, which encodes the same command, so the reason is
            // not worth reporting. Skipping still walks the arguments for a nested command.
            Err(_) => Rewrite::Skip,
        }
    }
}

/// The two `const` items a folded call needs, and the arguments they are built from.
pub(super) struct Buffer {
    size: TokenStream,
    encode: TokenStream,
    args: TokenStream,
}

impl Buffer {
    /// The whole block, with `tail` as its value — the one thing the two pipelines differ in.
    ///
    /// `original` is kept only for type checking and goto/hover on the command name. The
    /// arguments are emitted three times, which is safe because only constants get here.
    pub(super) fn block(&self, original: &TokenStream, tail: TokenStream) -> TokenStream {
        let Buffer { size, encode, args } = self;

        quote! {
            {
                const __FPRIME_LEN: usize = #size(#args);
                const __FPRIME_CMD: [u8; __FPRIME_LEN] = #encode::<__FPRIME_LEN>(#args);

                if false {
                    #original;
                }

                #tail
            }
        }
    }
}

impl Symbols {
    /// The `const` items for `command` applied to `args`, or why they cannot be built.
    ///
    /// `#[fprime]` discards the reason and falls back on the run-time accessor.
    /// `#[fprime_test]` reports it, since a test that wrote arguments has no fallback.
    pub(super) fn buffer(&self, command: &Command, args: &Group) -> Result<Buffer, syn::Error> {
        let help = matching_alternatives(command);
        let at = |span: Span, reason: &str| syn::Error::new(span, format!("{reason}\n\n{help}"));

        let unencodable = command
            .formal_params
            .iter()
            .find(|param| self.dictionary.const_arg(&param.type_name).is_none());

        if let Some(param) = unencodable {
            return Err(at(
                args.span(),
                &format!(
                    "`{}` is a {}, which a test cannot write out as bytes",
                    param.name,
                    type_label(&self.dictionary, &param.type_name),
                ),
            ));
        }

        let parsed = arguments(args.stream());
        if parsed.len() != command.formal_params.len() {
            return Err(at(
                args.span(),
                &format!(
                    "{} takes {} arguments, but {} {} written",
                    command.name,
                    command.formal_params.len(),
                    parsed.len(),
                    were(parsed.len()),
                ),
            ));
        }

        for (param, argument) in command.formal_params.iter().zip(&parsed) {
            let Ok(expr) = syn::parse2::<Expr>(argument.tokens.clone()) else {
                return Err(at(
                    argument.tokens.span(),
                    &format!("`{}` is not something a test can read", param.name),
                ));
            };

            self.check_const_argument(&expr, &param.type_name, &param.name)
                .map_err(|err| at(err.span(), &err.to_string()))?;
        }

        Ok(Buffer {
            size: konst_path(&command.name, SIZE_SUFFIX),
            encode: konst_path(&command.name, ENCODE_SUFFIX),
            args: args.stream(),
        })
    }

    /// Whether `expr` is a constant of type `expected`, and why not if it is not.
    ///
    /// `at` names the value in that reason — the parameter's name, extended a member or an
    /// index at a time, so it reads `choices.choicePair.firstChoice` rather than `choices`.
    ///
    /// Recursion follows `expr`, which is finite, so it needs no depth limit of its own.
    fn check_const_argument(&self, expr: &Expr, expected: &TypeName, at: &str) -> syn::Result<()> {
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
                        self.check_const_elements(expr, &ty.element_type, ty.size, at)
                    }
                    Some(TypeDefinition::Struct(ty)) => self.check_const_struct(expr, ty, at),
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
                None => self.check_const_argument(&field.expr, &member.type_name, &at)?,
                Some(size) => {
                    self.check_const_elements(&field.expr, &member.type_name, size, &at)?
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
                    self.check_const_argument(elem, element, &at)?;
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

                self.check_const_argument(&repeat.expr, element, at)
            }
            _ => Err(syn::Error::new_spanned(
                expr,
                format!("`{at}` has to be a list of {size} values, written out here"),
            )),
        }
    }
}

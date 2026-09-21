//! A passthrough DSL for writing F Prime command sequences.
//!
//! Inside a function marked `#[fprime]`, a command argument may name an
//! enumerated constant on its own:
//!
//! ```ignore
//! Ref.dpDemo.Dp(IMMEDIATE, 0, PROC_TYPE_NONE);
//! ```
//!
//! This gets expanded into:
//!
//! ```ignore
//! Ref.dpDemo
//!     .Dp(crate::Defs::Ref::DpDemo::DpReqType::IMMEDIATE, 0, crate::Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE);
//! ```

use fprime_dictionary::konst::{ENCODE_SUFFIX, SIZE_SUFFIX};
use fprime_dictionary::naming::{
    definition_path, definition_path_with_name, desc_path, konst_path, split_qualified_name,
    str_to_ident,
};
use fprime_dictionary::{Command, Dictionary, TypeDefinition, TypeName};
use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote, quote_spanned};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use syn::{Expr, ExprPath, ExprStruct, Lit, Member, Path as SynPath, UnOp};

/// Aliases followed before concluding the dictionary is cyclic.
const MAX_ALIAS_HOPS: usize = 32;

/// Nesting depth qualified inside a single command argument
const MAX_DEPTH: usize = 32;
const COMPLETION_MARKER: &str = "raCompletionMarker";
const MISSING_DICTIONARY: &str = concat!(
    "the F Prime DSL needs a dictionary to resolve names against, but `",
    "FPRIME_DICTIONARY",
    "` is not set.\n",
    "Call `fprime_build::generate(\"<topology>Dictionary.json\")` from this crate's `build.rs`."
);

/// Rewrite the bare enumerated constants in `item` into fully qualified paths.
pub(crate) fn rewrite(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "this attribute takes no arguments",
        ));
    }

    let span = signature_span(&item);

    let Ok(dictionary) = std::env::var(fprime_dictionary::DICTIONARY_ENV) else {
        return Err(syn::Error::new(span, MISSING_DICTIONARY));
    };

    let resolver = resolver(Path::new(&dictionary)).map_err(|err| syn::Error::new(span, err))?;

    let qualified = resolver.qualify_calls(item);

    Ok(resolver.const_encode_calls(qualified))
}

/// Qualifies, then descriptor-rewrites — in that order, since qualifying needs the call
/// still in command position.
pub(crate) fn rewrite_test_body(body: TokenStream, span: Span) -> syn::Result<TokenStream> {
    let Ok(dictionary) = std::env::var(fprime_dictionary::DICTIONARY_ENV) else {
        return Err(syn::Error::new(span, MISSING_DICTIONARY));
    };

    let resolver = resolver(Path::new(&dictionary)).map_err(|err| syn::Error::new(span, err))?;

    let qualified = resolver.qualify_calls(body);
    Ok(resolver.descriptor_calls(qualified))
}

fn signature_span(item: &TokenStream) -> Span {
    let mut first = None;
    let mut after_fn = false;

    for tree in item.clone() {
        first = first.or(Some(tree.span()));

        match &tree {
            TokenTree::Ident(name) if after_fn => return name.span(),
            TokenTree::Ident(keyword) if keyword == "fn" => after_fn = true,
            _ => {}
        }
    }

    first.unwrap_or_else(Span::call_site)
}

/// Dictionaries are parsed once per compilation and shared by every expansion.
fn resolver(dictionary: &Path) -> Result<&'static Resolver, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, &'static Resolver>>> = OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(|err| err.into_inner());

    if let Some(resolver) = cache.get(dictionary) {
        return Ok(resolver);
    }

    let parsed = fprime_dictionary::try_parse(dictionary)?;
    let resolver: &'static Resolver = Box::leak(Box::new(Resolver::new(parsed)));
    cache.insert(dictionary.to_path_buf(), resolver);

    Ok(resolver)
}

struct Resolver {
    dictionary: Dictionary,
    commands: HashMap<String, usize>,
    /// Channel and parameter points — needed by `#[fprime_test]`, not `#[fprime_main]`.
    channels: HashMap<String, usize>,
    parameters: HashMap<String, usize>,
}

/// Which kind of point a dotted name is.
enum Point<'a> {
    Command(&'a Command),
    Channel,
    Parameter,
}

impl Resolver {
    fn new(dictionary: Dictionary) -> Self {
        let commands = dictionary
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| (command.name.clone(), index))
            .collect();
        let channels = dictionary
            .telemetry_channels
            .iter()
            .enumerate()
            .map(|(index, channel)| (channel.name.clone(), index))
            .collect();
        let parameters = dictionary
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.clone(), index))
            .collect();

        Self {
            dictionary,
            commands,
            channels,
            parameters,
        }
    }

    /// The point `name` refers to, if any (commands checked first).
    fn point(&self, name: &str) -> Option<Point<'_>> {
        if let Some(command) = self.command(name) {
            return Some(Point::Command(command));
        }
        if self.channels.contains_key(name) {
            return Some(Point::Channel);
        }
        if self.parameters.contains_key(name) {
            return Some(Point::Parameter);
        }
        None
    }

    fn command(&self, name: &str) -> Option<&Command> {
        self.commands
            .get(name)
            .map(|index| &self.dictionary.commands[*index])
    }

    /// Follow aliases down to the definition that carries the constants or members.
    fn definition(&self, ty: &TypeName) -> Option<&TypeDefinition> {
        let TypeName::QualifiedIdentifier { name } = ty else {
            return None;
        };

        let mut definition = self.dictionary.type_definitions.get(name)?;
        for _ in 0..MAX_ALIAS_HOPS {
            let TypeDefinition::Alias(alias) = definition else {
                return Some(definition);
            };

            let TypeName::QualifiedIdentifier { name } = &alias.underlying_type else {
                return None;
            };

            definition = self.dictionary.type_definitions.get(name)?;
        }

        None
    }

    fn qualify_calls(&self, tokens: TokenStream) -> TokenStream {
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

    fn command_call(&self, trees: &[TokenTree], start: usize) -> Option<CommandCall<'_>> {
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

    /// Rewrites every dictionary path in a test body into a `Desc` descriptor. A chain that
    /// is not a dictionary point (`t.initial_telemetry(..)`, `event().containing(..)`, a
    /// local) is left alone.
    fn descriptor_calls(&self, tokens: TokenStream) -> TokenStream {
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
                let (rewritten, next) = self.descriptor_for(&trees, index, &found);
                out.extend(rewritten);
                index = next;
                continue;
            }

            match &trees[index] {
                TokenTree::Group(group) => {
                    let mut descended =
                        Group::new(group.delimiter(), self.descriptor_calls(group.stream()));
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
    ) -> (TokenStream, usize) {
        // A command *call* carries its arguments; a bare one does not.
        if let Some(Point::Command(command)) = self.point(&found.name)
            && let Some(TokenTree::Group(args)) = trees.get(found.end)
            && args.delimiter() == Delimiter::Parenthesis
        {
            let original: TokenStream = trees[start..=found.end].iter().cloned().collect();
            if let Some(encoded) = self.const_encoded_call(command, args, original, {
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
            // Not const-encodable — left for the accessor to reject against its real signature.
            let original: TokenStream = trees[start..=found.end].iter().cloned().collect();
            return (original, found.end + 1);
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

    /// Replaces every command call whose arguments are all compile-time constants with a
    /// reference to its encoded `Fw::ComBuffer`.
    fn const_encode_calls(&self, tokens: TokenStream) -> TokenStream {
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
    fn const_encoded_call(
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
        self.is_const_argument_at(expr, expected, 0)
    }

    fn is_const_argument_at(&self, expr: &Expr, expected: &TypeName, depth: usize) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }

        match self.dictionary.resolved_type(expected) {
            Some(TypeName::String { .. }) => {
                matches!(expr, Expr::Lit(lit) if matches!(lit.lit, Lit::Str(_)))
            }
            Some(TypeName::Bool) => {
                matches!(expr, Expr::Lit(lit) if matches!(lit.lit, Lit::Bool(_)))
            }
            Some(TypeName::Integer { .. } | TypeName::Float { .. }) => is_numeric_literal(expr),
            Some(TypeName::QualifiedIdentifier { name }) => {
                match self.dictionary.type_definitions.get(name) {
                    Some(TypeDefinition::Enum(_)) => is_qualified_definition(expr),
                    Some(TypeDefinition::Array(ty)) => {
                        self.is_const_elements(expr, &ty.element_type, ty.size, depth)
                    }
                    Some(TypeDefinition::Struct(ty)) => {
                        let Expr::Struct(literal) = expr else {
                            return false;
                        };

                        // A literal with `..rest` can't be evaluated.
                        if literal.rest.is_some() || !is_qualified_path(&literal.path) {
                            return false;
                        }

                        // Every member has to be given, and given a constant.
                        ty.members.len() == literal.fields.len()
                            && ty.members.iter().all(|member| {
                                let field = literal.fields.iter().find(|field| {
                                    matches!(&field.member, Member::Named(name)
                                        if unraw(name) == member.name)
                                });

                                let Some(field) = field else {
                                    return false;
                                };

                                match member.size {
                                    None => self.is_const_argument_at(
                                        &field.expr,
                                        &member.type_name,
                                        depth + 1,
                                    ),
                                    Some(size) => self.is_const_elements(
                                        &field.expr,
                                        &member.type_name,
                                        size,
                                        depth,
                                    ),
                                }
                            })
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// An array of exactly `size` constants, as a list or `[x; n]` repeat.
    fn is_const_elements(&self, expr: &Expr, element: &TypeName, size: u32, depth: usize) -> bool {
        match expr {
            Expr::Array(array) => {
                array.elems.len() == size as usize
                    && array
                        .elems
                        .iter()
                        .all(|elem| self.is_const_argument_at(elem, element, depth + 1))
            }
            Expr::Repeat(repeat) => {
                // The length has to be a plain literal for us to know it matches.
                let Expr::Lit(lit) = repeat.len.as_ref() else {
                    return false;
                };
                let Lit::Int(len) = &lit.lit else {
                    return false;
                };

                len.base10_parse::<u32>().is_ok_and(|len| len == size)
                    && self.is_const_argument_at(&repeat.expr, element, depth + 1)
            }
            _ => false,
        }
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

                let Some(name) = bare_name(original.qself.as_ref(), &original.path) else {
                    return;
                };

                let Some(constant) = ty
                    .enumerated_constants
                    .iter()
                    .find(|c| c.name == name.lookup)
                else {
                    return;
                };

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

struct CommandCall<'a> {
    command: &'a Command,
    args: usize,
}

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

struct Argument {
    tokens: TokenStream,
    separator: Option<TokenTree>,
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

/// Splits an argument list on its commas, keeping them.
fn arguments(tokens: TokenStream) -> Vec<Argument> {
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

/// Replaces an unqualified struct path with its dictionary path, e.g. `ChoicePair` ->
/// `crate::Defs::Ref::ChoicePair`.
fn qualify_struct_path(literal: &mut ExprStruct, qualified_name: &str) {
    let Some(name) = bare_name(literal.qself.as_ref(), &literal.path) else {
        return;
    };

    let (_, short_name) = split_qualified_name(qualified_name);
    if name.lookup != short_name {
        return;
    }

    literal.path = parse(definition_path_with_name(
        qualified_name,
        name.emit(short_name),
    ));
}

/// A bare identifier, possibly one an editor is partway through typing.
struct BareName {
    /// The name to look up, with any editor marker stripped.
    lookup: String,
    /// The identifier as written, marker included.
    written: Ident,
    span: Span,
    /// Whether an editor is mid-completion on this name.
    in_progress: bool,
}

impl BareName {
    /// The identifier to emit for a dictionary name this resolved to.
    fn emit(&self, dictionary_name: &str) -> Ident {
        if self.in_progress {
            return self.written.clone();
        }

        let mut ident = str_to_ident(dictionary_name);
        ident.set_span(self.span);

        ident
    }
}

fn bare_name(qself: Option<&syn::QSelf>, path: &SynPath) -> Option<BareName> {
    if qself.is_some() || path.leading_colon.is_some() || path.segments.len() != 1 {
        return None;
    }

    let segment = path.segments.first()?;
    if !segment.arguments.is_none() {
        return None;
    }

    let written = unraw(&segment.ident);
    let in_progress = written.contains(COMPLETION_MARKER);

    Some(BareName {
        lookup: if in_progress {
            written.replace(COMPLETION_MARKER, "")
        } else {
            written
        },
        written: segment.ident.clone(),
        span: segment.ident.span(),
        in_progress,
    })
}

/// The identifier's name with any `r#` prefix stripped.
fn unraw(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").map(str::to_string).unwrap_or(name)
}

fn parse<T: syn::parse::Parse>(tokens: TokenStream) -> T {
    syn::parse2(tokens).expect("generated dictionary path should parse")
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// The dictionary the workspace tests share.
    const DICTIONARY: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fprime_dictionary/src/test/RefTopologyDictionary.json"
    );

    /// Apply the DSL to a sequence body and render the result as tokens.
    pub(crate) fn rewrite_block(body: &str) -> syn::Result<String> {
        let resolver = resolver(Path::new(DICTIONARY))
            .map_err(|err| syn::Error::new(Span::call_site(), err))?;

        Ok(resolver.qualify_calls(syn::parse_str(body)?).to_string())
    }

    /// Both passes, in the order `rewrite` runs them: qualify, then const-encode.
    pub(crate) fn const_encode_block(body: &str) -> syn::Result<String> {
        let resolver = resolver(Path::new(DICTIONARY))
            .map_err(|err| syn::Error::new(Span::call_site(), err))?;

        let qualified = resolver.qualify_calls(syn::parse_str(body)?);

        Ok(resolver.const_encode_calls(qualified).to_string())
    }

    /// Renders a body without rewriting it, for writing expectations as ordinary Rust.
    pub(crate) fn render_block(body: &str) -> syn::Result<String> {
        Ok(syn::parse_str::<TokenStream>(body)?.to_string())
    }
}

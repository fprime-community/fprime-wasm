//! Walking a sequence body against a [`Profile`].
//!
//! The check runs on the body as written, before `infer_path` expands it — the expansion
//! itself uses `unsafe`, references and `const` items.

use super::config::{Binary, MethodCall, Profile, Rule, Unary};
use super::waive::{self, Waiver};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Attribute, BinOp, Expr, ExprMethodCall, Item, ItemFn, Macro, Member, Pat, Stmt, Token, Type,
    UnOp,
};

/// Syn's catch-all for syntax it parses but does not model, which no rule can name.
const UNSUPPORTED: &str = "this syntax is not supported by sequence profiles";

/// What a body broke, in the order it was written.
#[derive(Default)]
pub(crate) struct Findings {
    pub(crate) errors: Vec<syn::Error>,
    pub(crate) warnings: Vec<(Span, String)>,
}

/// Every [`Rule`] in `profile` that `item`'s body breaks.
///
/// An item that doesn't parse as a function is left for the compiler to report.
pub(crate) fn check(profile: &Profile, item: &TokenStream) -> Findings {
    let Ok(function) = syn::parse2::<ItemFn>(item.clone()) else {
        return Findings::default();
    };

    let mut checker = Checker {
        profile,
        findings: Findings::default(),
    };
    for stmt in &function.block.stmts {
        checker.visit_stmt(stmt);
    }

    checker.findings
}

impl Findings {
    /// The errors as one `compile_error!`, and each warning as the use of a `#[deprecated]`
    /// const at the warned-about node — stable Rust has no other way for a proc macro to warn.
    pub(crate) fn into_tokens(self, profile: &Ident) -> TokenStream {
        let errors = self
            .errors
            .into_iter()
            .reduce(|mut all, next| {
                all.combine(next);
                all
            })
            .map(syn::Error::into_compile_error);

        // Named after the profile, since rustc's message names the const.
        let warnings = self.warnings.into_iter().map(|(span, message)| {
            let used = Ident::new(&profile.to_string(), span);
            quote! {
                const _: () = {
                    #[deprecated(note = #message)]
                    #[allow(non_upper_case_globals)]
                    const #profile: () = ();
                    #used
                };
            }
        });

        quote! {
            #errors
            #(#warnings)*
        }
    }
}

struct Checker<'p> {
    profile: &'p Profile,
    findings: Findings,
}

impl Checker<'_> {
    /// Records what `rule` says about the node at `span`, answering whether to walk into it.
    fn apply(&mut self, rule: &Rule, span: Span) -> bool {
        match rule {
            Rule::Allow => true,
            Rule::Warn { message } => {
                self.findings.warnings.push((span, message.clone()));
                true
            }
            Rule::Error { message } => {
                self.findings.errors.push(syn::Error::new(span, message));
                false
            }
        }
    }

    fn unsupported(&mut self, span: Span) {
        self.findings
            .errors
            .push(syn::Error::new(span, UNSUPPORTED));
    }

    /// Runs `check` on a node carrying `attrs`, then settles a `#[rule_waive]` among them
    /// against the errors `check` found — so a waiver covers its whole node, nested ones
    /// included.
    fn waivable(&mut self, attrs: &[Attribute], check: impl FnOnce(&mut Self)) {
        let waiver = waive::parse(attrs);
        let before = self.findings.errors.len();
        check(self);

        match waiver {
            Some(Ok(waiver)) => self.settle(waiver, before),
            Some(Err(err)) => self.findings.errors.push(err),
            None => {}
        }
    }

    fn settle(&mut self, waiver: Waiver, before: usize) {
        if self.findings.errors.len() == before {
            // A waiver outliving the code that needed it would hide the next real finding.
            self.findings.errors.push(syn::Error::new(
                waiver.span,
                "nothing here breaks the profile; remove this `#[rule_waive]`",
            ));
            return;
        }

        match &self.profile.waivers {
            Rule::Allow => self.findings.errors.truncate(before),
            Rule::Warn { message } => {
                self.findings.errors.truncate(before);
                let note = format!("{message}: {}", waiver.reason);
                self.findings.warnings.push((waiver.span, note));
            }
            // The waived errors stand, and the waiver says why it didn't help.
            Rule::Error { message } => self
                .findings
                .errors
                .push(syn::Error::new(waiver.span, message)),
        }
    }

    fn check_item(&mut self, i: &Item) {
        let rules = &self.profile.statements.item;
        let rule = match i {
            Item::Const(_) => &rules.r#const,
            Item::Enum(_) => &rules.r#enum,
            Item::ExternCrate(_) => &rules.extern_crate,
            Item::Fn(_) => &rules.r#fn,
            Item::ForeignMod(_) => &rules.foreign_mod,
            Item::Impl(_) => &rules.r#impl,
            // By path, in `visit_macro`.
            Item::Macro(_) => return visit::visit_item(self, i),
            Item::Mod(_) => &rules.r#mod,
            Item::Static(_) => &rules.r#static,
            Item::Struct(_) => &rules.r#struct,
            Item::Trait(_) => &rules.r#trait,
            Item::TraitAlias(_) => &rules.trait_alias,
            Item::Type(_) => &rules.r#type,
            Item::Union(_) => &rules.r#union,
            Item::Use(_) => &rules.r#use,
            _ => return self.unsupported(i.span()),
        };

        if self.apply(rule, i.span()) {
            visit::visit_item(self, i);
        }
    }

    fn check_expr(&mut self, i: &Expr) {
        let rules = &self.profile.expressions;
        let rule = match i {
            Expr::Array(_) => &rules.array,
            Expr::Assign(_) => &rules.assign,
            Expr::Async(_) => &rules.r#async,
            Expr::Await(_) => &rules.r#await,
            Expr::Binary(binary) => match binary_rule(&rules.binary, &binary.op) {
                Some(rule) => rule,
                None => return self.unsupported(i.span()),
            },
            Expr::Block(_) => &rules.block,
            Expr::Break(_) => &rules.r#break,
            Expr::Call(call) => match &*call.func {
                Expr::Path(func) if func.qself.is_none() => rules.call.rule(&path_name(&func.path)),
                _ => &rules.call.default,
            },
            Expr::Cast(_) => &rules.cast,
            Expr::Closure(_) => &rules.closure,
            Expr::Const(_) => &rules.r#const,
            Expr::Continue(_) => &rules.r#continue,
            Expr::Field(_) => &rules.field,
            Expr::ForLoop(_) => &rules.for_loop,
            Expr::Group(_) => &rules.group,
            Expr::If(_) => &rules.r#if,
            Expr::Index(_) => &rules.index,
            Expr::Infer(_) => &rules.infer,
            Expr::Let(_) => &rules.r#let,
            Expr::Lit(_) => &rules.lit,
            Expr::Loop(_) => &rules.r#loop,
            Expr::Macro(_) => return visit::visit_expr(self, i),
            Expr::Match(_) => &rules.r#match,
            Expr::MethodCall(call) => method_call_rule(&rules.method_call, call),
            Expr::Paren(_) => &rules.paren,
            Expr::Path(_) => &rules.path,
            Expr::Range(_) => &rules.range,
            Expr::RawAddr(_) => &rules.raw_addr,
            Expr::Reference(_) => &rules.reference,
            Expr::Repeat(_) => &rules.repeat,
            Expr::Return(_) => &rules.r#return,
            Expr::Struct(_) => &rules.r#struct,
            Expr::Try(_) => &rules.r#try,
            Expr::TryBlock(_) => &rules.try_block,
            Expr::Tuple(_) => &rules.tuple,
            Expr::Unary(unary) => match unary_rule(&rules.unary, &unary.op) {
                Some(rule) => rule,
                None => return self.unsupported(i.span()),
            },
            Expr::Unsafe(_) => &rules.r#unsafe,
            Expr::While(_) => &rules.r#while,
            Expr::Yield(_) => &rules.r#yield,
            _ => return self.unsupported(i.span()),
        };

        if self.apply(rule, i.span()) {
            visit::visit_expr(self, i);
        }
    }
}

impl<'ast> Visit<'ast> for Checker<'_> {
    fn visit_stmt(&mut self, i: &'ast Stmt) {
        let rules = &self.profile.statements;
        let (rule, attrs) = match i {
            Stmt::Local(local) => (&rules.local, &local.attrs[..]),
            // The expression's attributes are its own, in `visit_expr`.
            Stmt::Expr(_, _) => (&rules.expr, &[][..]),
            // By path, in `visit_macro`.
            Stmt::Macro(mac) => {
                return self.waivable(&mac.attrs, |this| this.visit_macro(&mac.mac));
            }
            // By kind, in `visit_item`.
            Stmt::Item(_) => return visit::visit_stmt(self, i),
        };

        self.waivable(attrs, |this| {
            if this.apply(rule, i.span()) {
                visit::visit_stmt(this, i);
            }
        });
    }

    fn visit_item(&mut self, i: &'ast Item) {
        self.waivable(item_attrs(i), |this| this.check_item(i));
    }

    fn visit_expr(&mut self, i: &'ast Expr) {
        self.waivable(expr_attrs(i), |this| this.check_expr(i));
    }

    /// Every macro, in any position, by the path it is invoked with. To syn its arguments are
    /// only tokens, so an allowed macro's are checked as the comma-separated expressions most
    /// macros take, and one whose arguments aren't that is an error — allowing a macro must
    /// not let unchecked code through.
    fn visit_macro(&mut self, i: &'ast Macro) {
        let name = path_name(&i.path);
        if !self.apply(self.profile.macros.rule(&name), i.span()) {
            return;
        }

        match i.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
            Ok(arguments) => arguments
                .iter()
                .for_each(|argument| self.visit_expr(argument)),
            Err(_) => self.findings.errors.push(syn::Error::new(
                i.delimiter.span().join(),
                format!("the arguments of `{name}!` are not expressions a profile can check"),
            )),
        }
    }

    fn visit_pat(&mut self, i: &'ast Pat) {
        let rules = &self.profile.patterns;
        let rule = match i {
            Pat::Const(_) => &rules.r#const,
            Pat::Ident(ident) => {
                if ident.by_ref.is_some() && !self.apply(&rules.by_ref, i.span()) {
                    return;
                }
                &rules.ident
            }
            Pat::Lit(_) => &rules.lit,
            Pat::Macro(_) => return visit::visit_pat(self, i),
            Pat::Or(_) => &rules.or,
            Pat::Paren(_) => &rules.paren,
            Pat::Path(_) => &rules.path,
            Pat::Range(_) => &rules.range,
            Pat::Reference(_) => &rules.reference,
            Pat::Rest(_) => &rules.rest,
            Pat::Slice(_) => &rules.slice,
            Pat::Struct(_) => &rules.r#struct,
            Pat::Tuple(_) => &rules.tuple,
            Pat::TupleStruct(_) => &rules.tuple_struct,
            Pat::Type(_) => &rules.r#type,
            Pat::Wild(_) => &rules.wild,
            _ => return self.unsupported(i.span()),
        };

        if self.apply(rule, i.span()) {
            visit::visit_pat(self, i);
        }
    }

    fn visit_type(&mut self, i: &'ast Type) {
        let rules = &self.profile.types;
        let rule = match i {
            Type::Array(_) => &rules.array,
            Type::BareFn(_) => &rules.bare_fn,
            Type::Group(_) => &rules.group,
            Type::ImplTrait(_) => &rules.impl_trait,
            Type::Infer(_) => &rules.infer,
            Type::Macro(_) => return visit::visit_type(self, i),
            Type::Never(_) => &rules.never,
            Type::Paren(_) => &rules.paren,
            Type::Path(_) => &rules.path,
            Type::Ptr(_) => &rules.ptr,
            Type::Reference(_) => &rules.reference,
            Type::Slice(_) => &rules.slice,
            Type::TraitObject(_) => &rules.trait_object,
            Type::Tuple(_) => &rules.tuple,
            _ => return self.unsupported(i.span()),
        };

        if self.apply(rule, i.span()) {
            visit::visit_type(self, i);
        }
    }
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn expr_attrs(expr: &Expr) -> &[Attribute] {
    match expr {
        Expr::Array(e) => &e.attrs,
        Expr::Assign(e) => &e.attrs,
        Expr::Async(e) => &e.attrs,
        Expr::Await(e) => &e.attrs,
        Expr::Binary(e) => &e.attrs,
        Expr::Block(e) => &e.attrs,
        Expr::Break(e) => &e.attrs,
        Expr::Call(e) => &e.attrs,
        Expr::Cast(e) => &e.attrs,
        Expr::Closure(e) => &e.attrs,
        Expr::Const(e) => &e.attrs,
        Expr::Continue(e) => &e.attrs,
        Expr::Field(e) => &e.attrs,
        Expr::ForLoop(e) => &e.attrs,
        Expr::Group(e) => &e.attrs,
        Expr::If(e) => &e.attrs,
        Expr::Index(e) => &e.attrs,
        Expr::Infer(e) => &e.attrs,
        Expr::Let(e) => &e.attrs,
        Expr::Lit(e) => &e.attrs,
        Expr::Loop(e) => &e.attrs,
        Expr::Macro(e) => &e.attrs,
        Expr::Match(e) => &e.attrs,
        Expr::MethodCall(e) => &e.attrs,
        Expr::Paren(e) => &e.attrs,
        Expr::Path(e) => &e.attrs,
        Expr::Range(e) => &e.attrs,
        Expr::RawAddr(e) => &e.attrs,
        Expr::Reference(e) => &e.attrs,
        Expr::Repeat(e) => &e.attrs,
        Expr::Return(e) => &e.attrs,
        Expr::Struct(e) => &e.attrs,
        Expr::Try(e) => &e.attrs,
        Expr::TryBlock(e) => &e.attrs,
        Expr::Tuple(e) => &e.attrs,
        Expr::Unary(e) => &e.attrs,
        Expr::Unsafe(e) => &e.attrs,
        Expr::While(e) => &e.attrs,
        Expr::Yield(e) => &e.attrs,
        _ => &[],
    }
}

/// `None` for an operator newer than this match, as `BinOp` is non-exhaustive.
fn binary_rule<'p>(rules: &'p Binary, op: &BinOp) -> Option<&'p Rule> {
    Some(match op {
        BinOp::Add(_) => &rules.add,
        BinOp::Sub(_) => &rules.sub,
        BinOp::Mul(_) => &rules.mul,
        BinOp::Div(_) => &rules.div,
        BinOp::Rem(_) => &rules.rem,
        BinOp::And(_) => &rules.and,
        BinOp::Or(_) => &rules.or,
        BinOp::BitXor(_) => &rules.bit_xor,
        BinOp::BitAnd(_) => &rules.bit_and,
        BinOp::BitOr(_) => &rules.bit_or,
        BinOp::Shl(_) => &rules.shl,
        BinOp::Shr(_) => &rules.shr,
        BinOp::Eq(_) => &rules.eq,
        BinOp::Lt(_) => &rules.lt,
        BinOp::Le(_) => &rules.le,
        BinOp::Ne(_) => &rules.ne,
        BinOp::Ge(_) => &rules.ge,
        BinOp::Gt(_) => &rules.gt,
        BinOp::AddAssign(_) => &rules.add_assign,
        BinOp::SubAssign(_) => &rules.sub_assign,
        BinOp::MulAssign(_) => &rules.mul_assign,
        BinOp::DivAssign(_) => &rules.div_assign,
        BinOp::RemAssign(_) => &rules.rem_assign,
        BinOp::BitXorAssign(_) => &rules.bit_xor_assign,
        BinOp::BitAndAssign(_) => &rules.bit_and_assign,
        BinOp::BitOrAssign(_) => &rules.bit_or_assign,
        BinOp::ShlAssign(_) => &rules.shl_assign,
        BinOp::ShrAssign(_) => &rules.shr_assign,
        _ => return None,
    })
}

fn unary_rule<'p>(rules: &'p Unary, op: &UnOp) -> Option<&'p Rule> {
    Some(match op {
        UnOp::Deref(_) => &rules.deref,
        UnOp::Not(_) => &rules.not,
        UnOp::Neg(_) => &rules.neg,
        _ => return None,
    })
}

/// Which namespace a dictionary chain is rooted in. The generated `Cmd`, `Tlm` and `Prm`
/// modules are what make this decidable from syntax; a sequence crate glob-imports `Cmd`,
/// so an unprefixed chain is a command.
enum Namespace {
    Command,
    Telemetry,
    Parameter,
}

fn method_call_rule<'p>(rules: &'p MethodCall, call: &ExprMethodCall) -> &'p Rule {
    match point(call) {
        Some((Namespace::Command, name)) => rules.commands.rule(&name),
        Some((Namespace::Telemetry, name)) => rules.telemetry.rule(&name),
        Some((Namespace::Parameter, name)) => rules.parameters.rule(&name),
        None => &rules.other,
    }
}

/// The dictionary point a call names and its dotted name, e.g. `Tlm::Ref.power.Volts()` is
/// telemetry `Ref.power.Volts` — or `None` for a call that isn't on a dotted chain, like
/// `x.pow(2)`.
fn point(call: &ExprMethodCall) -> Option<(Namespace, String)> {
    let mut segments = vec![unraw(&call.method)];
    let mut receiver = &*call.receiver;

    let root = loop {
        match receiver {
            Expr::Field(field) => {
                let Member::Named(ident) = &field.member else {
                    return None;
                };
                segments.push(unraw(ident));
                receiver = &field.base;
            }
            Expr::Path(path) if path.qself.is_none() => break &path.path,
            _ => return None,
        }
    };

    // A point is at least `Component.NAME`.
    if segments.len() < 2 {
        return None;
    }

    let mut path = root.segments.iter().rev();
    segments.push(unraw(&path.next()?.ident));
    segments.reverse();

    let namespace = match path.next().map(|segment| unraw(&segment.ident)).as_deref() {
        Some("Tlm") => Namespace::Telemetry,
        Some("Prm") => Namespace::Parameter,
        // `Cmd::`, `crate::` or nothing at all.
        _ => Namespace::Command,
    };

    Some((namespace, segments.join(".")))
}

/// `set_fail_mode` or `fprime_core::set_fail_mode`, as a [`Select`](super::config::Select)
/// matches it.
fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| unraw(&segment.ident))
        .collect::<Vec<_>>()
        .join("::")
}

/// The identifier's name with any `r#` prefix stripped, as a profile spells it.
fn unraw(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").map(str::to_string).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::config::Select;

    const RESTRICT: &str = include_str!("../../../crates/example/profile_restrict.toml");

    fn error(message: &str) -> Rule {
        Rule::Error {
            message: message.into(),
        }
    }

    /// `restrict`, with each kind of method call an error unless a test opens it up.
    fn profile() -> Profile {
        let mut profile = crate::profile::parse(RESTRICT).unwrap();
        let calls = &mut profile.expressions.method_call;
        for (select, kind) in [
            (&mut calls.commands, "command"),
            (&mut calls.telemetry, "telemetry"),
            (&mut calls.parameters, "parameter"),
        ] {
            select.default = error(kind);
        }
        calls.other = error("other");

        profile
    }

    fn errors(profile: &Profile, body: TokenStream) -> Vec<String> {
        check(profile, &quote! { fn main() { #body } })
            .errors
            .iter()
            .map(|err| err.to_string())
            .collect()
    }

    #[test]
    fn method_calls_split_by_namespace() {
        let profile = profile();

        assert_eq!(
            errors(&profile, quote! { Ref.power.PWR_OFF(); }),
            ["command"]
        );
        assert_eq!(
            errors(&profile, quote! { Cmd::Ref.power.PWR_OFF(); }),
            ["command"]
        );
        assert_eq!(
            errors(&profile, quote! { Tlm::Ref.power.Volts(); }),
            ["telemetry"]
        );
        assert_eq!(
            errors(&profile, quote! { crate::Prm::Ref.a.b(); }),
            ["parameter"]
        );
        assert_eq!(errors(&profile, quote! { x.pow(2); }), ["other"]);
    }

    #[test]
    fn commands_select_by_glob() {
        let mut profile = profile();
        profile.expressions.method_call.commands = toml::from_str::<Select>(
            r#"
            default = { Error = { message = "command" } }
            patterns = [{ name = "Ref.dpDemo.*", rule = "Allow" }]
            "#,
        )
        .unwrap();

        assert!(errors(&profile, quote! { Ref.dpDemo.SelectColor(RED); }).is_empty());
        assert_eq!(
            errors(&profile, quote! { Ref.power.PWR_OFF(); }),
            ["command"]
        );
    }

    /// An allowed call's arguments are still checked; a rejected one's are not.
    #[test]
    fn arguments_are_checked_only_when_the_call_is_allowed() {
        let mut profile = profile();
        profile.expressions.method_call.commands.default = Rule::Allow;

        assert_eq!(
            errors(&profile, quote! { Ref.cmd.SEND(Tlm::Ref.a.B()); }),
            ["telemetry"]
        );
        profile.expressions.method_call.commands.default = error("command");
        assert_eq!(
            errors(&profile, quote! { Ref.cmd.SEND(Tlm::Ref.a.B()); }),
            ["command"]
        );
    }

    #[test]
    fn warnings_compile() {
        let mut profile = profile();
        profile.expressions.method_call.commands.default = Rule::Warn {
            message: "careful".into(),
        };

        let findings = check(&profile, &quote! { fn main() { Ref.power.PWR_OFF(); } });
        assert!(findings.errors.is_empty());
        assert_eq!(findings.warnings.len(), 1);
        assert!(
            findings
                .into_tokens(&Ident::new("restrict", Span::call_site()))
                .to_string()
                .contains("deprecated (note = \"careful\")")
        );
    }

    #[test]
    fn plain_calls_select_by_path() {
        let mut profile = profile();
        profile.expressions.call = toml::from_str::<Select>(
            r#"
            default = { Error = { message = "call" } }
            patterns = [{ name = "set_fail_mode", rule = "Allow" }]
            "#,
        )
        .unwrap();

        assert!(errors(&profile, quote! { set_fail_mode(FailMode::Permissive); }).is_empty());
        assert_eq!(errors(&profile, quote! { sleep(1); }), ["call"]);
    }

    fn findings(profile: &Profile, body: TokenStream) -> (Vec<String>, Vec<String>) {
        let findings = check(profile, &quote! { fn main() { #body } });
        (
            findings.errors.iter().map(|err| err.to_string()).collect(),
            findings
                .warnings
                .into_iter()
                .map(|(_, note)| note)
                .collect(),
        )
    }

    #[test]
    fn macros_select_by_path_and_check_their_arguments() {
        let mut profile = profile();
        profile.expressions.method_call.telemetry.default = Rule::Allow;
        profile.macros = toml::from_str::<Select>(
            r#"
            default = { Error = { message = "macro" } }
            patterns = [{ name = "messagef", rule = "Allow" }]
            "#,
        )
        .unwrap();

        assert!(
            errors(
                &profile,
                quote! { messagef!(ActivityHi, "{}", Tlm::Ref.a.B().0); }
            )
            .is_empty()
        );
        assert_eq!(errors(&profile, quote! { println!("hi"); }), ["macro"]);
        assert_eq!(errors(&profile, quote! { let v = vec![1]; }), ["macro"]);
        // Allowed, but what it is given is still held to the profile.
        assert_eq!(
            errors(&profile, quote! { messagef!(ActivityHi, "{}", loop {}); }),
            ["loops are not allowed in restrict mode"]
        );
        assert_eq!(
            errors(&profile, quote! { messagef!(fn f() {}); }),
            ["the arguments of `messagef!` are not expressions a profile can check"]
        );
    }

    #[test]
    fn a_waiver_covers_its_node_and_everything_in_it() {
        let profile = profile();

        let (errors, warnings) = findings(
            &profile,
            quote! {
                #[rule_waive("bounded by the watchdog")]
                loop { Ref.cmd.NO_OP(); }
                let x = &y;
            },
        );
        assert_eq!(
            errors,
            ["references (`&`) are not allowed in restrict mode"]
        );
        assert_eq!(warnings, ["restrict mode waived: bounded by the watchdog"]);

        let (errors, _) = findings(&profile, quote! { let x = f(#[rule_waive("ok")] &y); });
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn waivers_follow_the_profile() {
        let mut profile = profile();
        let body = quote! { #[rule_waive("why")] loop {} };

        profile.waivers = Rule::Allow;
        assert_eq!(findings(&profile, body.clone()), (vec![], vec![]));

        profile.waivers = error("no waivers in flight sequences");
        assert_eq!(
            findings(&profile, body).0,
            [
                "loops are not allowed in restrict mode",
                "no waivers in flight sequences"
            ]
        );
    }

    #[test]
    fn a_waiver_must_be_needed_and_say_why() {
        let profile = profile();

        assert_eq!(
            errors(&profile, quote! { #[rule_waive("why")] let x = 1; }),
            ["nothing here breaks the profile; remove this `#[rule_waive]`"]
        );
        assert_eq!(
            errors(&profile, quote! { #[rule_waive("")] loop {} }),
            [
                "loops are not allowed in restrict mode",
                "expected `#[rule_waive(\"why this is safe\")]`"
            ]
        );
    }

    /// The reference profile documents every setting, so it must parse, and it allows all of
    /// them, so a body using a bit of everything breaks nothing.
    #[test]
    fn the_reference_profile_allows_everything() {
        let reference = include_str!("../../../crates/example/profile_reference.toml");
        let profile = crate::profile::parse(reference).unwrap();

        let (errors, warnings) = findings(
            &profile,
            quote! {
                const N: usize = 4;
                let mut values: [u32; N] = [0; N];
                for (i, v) in values.iter_mut().enumerate() { *v = i as u32 * 2; }
                let total = match values { [a, .., ref b] if a < *b => a + b, _ => 0 };
                loop { if total > 0 { break; } }
                messagef!(ActivityHi, "{}", Tlm::Ref.a.B().0 + Prm::Ref.c.D());
                Ref.cmd.SEND(&values, |x| x, unsafe { f() }, g()?);
            },
        );
        assert!(
            errors.is_empty() && warnings.is_empty(),
            "{errors:?} {warnings:?}"
        );
    }

    #[test]
    fn a_missing_rule_is_an_error() {
        let err = crate::profile::parse(&RESTRICT.replace("loop = ", "lop = ")).unwrap_err();
        assert!(err.to_string().contains("lop"), "{err}");
    }
}

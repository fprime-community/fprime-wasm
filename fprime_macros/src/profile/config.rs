//! The shape of a `profile_<name>.toml`: one [`Rule`] per kind of syntax a body can contain.
//!
//! Every key is required, so a profile always says what it does about each kind.

use super::glob::Glob;
use serde::Deserialize;

/// What a profile does about one kind of syntax. A rejected node is not walked into, so one
/// mistake is reported once.
#[derive(Debug, Deserialize)]
pub enum Rule {
    Allow,
    /// Compiles, with this warning.
    Warn {
        message: String,
    },
    /// Does not compile; the message is the error.
    Error {
        message: String,
    },
}

/// The statements a body is a list of.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statements {
    /// A local (let) binding: `let x: u64 = s.parse()?;`.
    pub local: Rule,
    /// An expression, with or without trailing semicolon.
    pub expr: Rule,
    /// An item definition, by kind.
    pub item: Items,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// A constant item: `const MAX: u16 = 65535`.
    pub r#const: Rule,
    /// An enum definition: `enum Foo<A, B> { A(A), B(B) }`.
    pub r#enum: Rule,
    /// An `extern crate` item: `extern crate serde`.
    pub extern_crate: Rule,
    /// A free-standing function: `fn process(n: usize) -> Result<()> { ... }`.
    pub r#fn: Rule,
    /// A block of foreign items: `extern "C" { ... }`.
    pub foreign_mod: Rule,
    /// An impl block providing trait or associated items: `impl<A> Trait for Data<A> { ... }`.
    pub r#impl: Rule,
    /// A module or module declaration: `mod m` or `mod m { ... }`.
    pub r#mod: Rule,
    /// A static item: `static BIKE: Shed = Shed(42)`.
    pub r#static: Rule,
    /// A struct definition: `struct Foo<A> { x: A }`.
    pub r#struct: Rule,
    /// A trait definition: `pub trait Iterator { ... }`.
    pub r#trait: Rule,
    /// A trait alias: `pub trait SharableIterator = Iterator + Sync`.
    pub trait_alias: Rule,
    /// A type alias: `type Result<T> = std::result::Result<T, MyError>`.
    pub r#type: Rule,
    /// A union definition: `union Foo<A, B> { x: A, y: B }`.
    pub r#union: Rule,
    /// A use declaration: `use std::collections::HashMap`.
    pub r#use: Rule,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expressions {
    /// A slice literal expression: `[a, b, c, d]`.
    pub array: Rule,
    /// An assignment expression: `a = compute()`.
    pub assign: Rule,
    /// An async block: `async { ... }`.
    pub r#async: Rule,
    /// An await expression: `fut.await`.
    pub r#await: Rule,
    /// A binary operation: `a + b`, `a += b`.
    pub binary: Binary,
    /// A blocked scope: `{ ... }`.
    pub block: Rule,
    /// A `break`, with an optional label to break and an optional
    /// expression.
    pub r#break: Rule,
    /// A function call expression: `invoke(a, b)`.
    pub call: Select,
    /// A cast expression: `foo as f64`.
    pub cast: Rule,
    /// A closure expression: `|a, b| a + b`.
    pub closure: Rule,
    /// A const block: `const { ... }`.
    pub r#const: Rule,
    /// A `continue`, with an optional label.
    pub r#continue: Rule,
    /// Access of a named struct field (`obj.k`) or unnamed tuple struct
    /// field (`obj.0`).
    pub field: Rule,
    /// A for loop: `for pat in expr { ... }`.
    pub for_loop: Rule,
    /// An expression contained within invisible delimiters.
    pub group: Rule,
    /// An `if` expression with an optional `else` block: `if expr { ... }
    /// else { ... }`.
    pub r#if: Rule,
    /// A square bracketed indexing expression: `vector[2]`.
    pub index: Rule,
    /// The inferred value of a const generic argument, denoted `_`.
    pub infer: Rule,
    /// A `let` guard: `let Some(x) = opt`.
    pub r#let: Rule,
    /// A literal in place of an expression: `1`, `"foo"`.
    pub lit: Rule,
    /// Conditionless loop: `loop { ... }`.
    pub r#loop: Rule,
    /// A `match` expression: `match n { Some(n) => {}, None => {} }`.
    pub r#match: Rule,
    /// A method call expression: `x.foo::<T>(a, b)`.
    pub method_call: MethodCall,
    /// A parenthesized expression: `(a + b)`.
    pub paren: Rule,
    /// A path like `std::mem::replace` possibly containing generic
    /// parameters and a qualified self-type.
    ///
    /// A plain identifier like `x` is a path of length 1.
    pub path: Rule,
    /// A range expression: `1..2`, `1..`, `..2`, `1..=2`, `..=2`.
    pub range: Rule,
    /// Address-of operation: `&raw const place` or `&raw mut place`.
    pub raw_addr: Rule,
    /// A referencing operation: `&a` or `&mut a`.
    pub reference: Rule,
    /// An array literal constructed from one repeated element: `[0u8; N]`.
    pub repeat: Rule,
    /// A `return`, with an optional value to be returned.
    pub r#return: Rule,
    /// A struct literal expression: `Point { x: 1, y: 1 }`.
    ///
    /// The `rest` provides the value of the remaining fields as in `S { a:
    /// 1, b: 1, ..rest }`.
    pub r#struct: Rule,
    /// A try-expression: `expr?`.
    pub r#try: Rule,
    /// A try block: `try { ... }`.
    pub try_block: Rule,
    /// A tuple expression: `(a, b, c, d)`.
    pub tuple: Rule,
    /// A unary operation: `!x`, `*x`.
    pub unary: Unary,
    /// An unsafe block: `unsafe { ... }`.
    pub r#unsafe: Rule,
    /// A while loop: `while expr { ... }`.
    pub r#while: Rule,
    /// A yield expression: `yield expr`.
    pub r#yield: Rule,
}

/// Per-operator rules for [`syn::BinOp`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binary {
    /// `+`
    pub add: Rule,
    /// `-`
    pub sub: Rule,
    /// `*`
    pub mul: Rule,
    /// `/`
    pub div: Rule,
    /// `%`
    pub rem: Rule,
    /// `&&`
    pub and: Rule,
    /// `||`
    pub or: Rule,
    /// `^`
    pub bit_xor: Rule,
    /// `&`
    pub bit_and: Rule,
    /// `|`
    pub bit_or: Rule,
    /// `<<`
    pub shl: Rule,
    /// `>>`
    pub shr: Rule,
    /// `==`
    pub eq: Rule,
    /// `<`
    pub lt: Rule,
    /// `<=`
    pub le: Rule,
    /// `!=`
    pub ne: Rule,
    /// `>=`
    pub ge: Rule,
    /// `>`
    pub gt: Rule,
    /// `+=`
    pub add_assign: Rule,
    /// `-=`
    pub sub_assign: Rule,
    /// `*=`
    pub mul_assign: Rule,
    /// `/=`
    pub div_assign: Rule,
    /// `%=`
    pub rem_assign: Rule,
    /// `^=`
    pub bit_xor_assign: Rule,
    /// `&=`
    pub bit_and_assign: Rule,
    /// `|=`
    pub bit_or_assign: Rule,
    /// `<<=`
    pub shl_assign: Rule,
    /// `>>=`
    pub shr_assign: Rule,
}

/// Per-operator rules for [`syn::UnOp`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unary {
    /// `*x`
    pub deref: Rule,
    /// `!x`
    pub not: Rule,
    /// `-x`
    pub neg: Rule,
}

/// A method call, split by the namespace its receiver is rooted in: `Tlm::Ref.a.B()` is
/// telemetry, `Prm::Ref.a.B()` a parameter, and `Ref.a.B()` or `Cmd::Ref.a.B()` a command.
/// Each is selected by its dotted name, `Ref.a.B`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodCall {
    pub commands: Select,
    pub telemetry: Select,
    pub parameters: Select,
    /// Any other method call, e.g. `x.pow(2)`.
    pub other: Rule,
}

/// A rule per name, chosen by glob.
///
/// ```toml
/// default = { Error = { message = "not in this profile" } }
/// patterns = [
///     { name = "Ref.dpDemo.*", rule = "Allow" },
///     { name = "CdhCore.cmdDisp.CMD_NO_OP*", rule = { Warn = { message = "..." } } },
/// ]
/// ```
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Select {
    /// For a name no pattern matches.
    pub default: Rule,
    /// Tried in order; the first match decides, so put exceptions before the globs they
    /// carve out of.
    #[serde(default)]
    pub patterns: Vec<Match>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Match {
    pub name: Glob,
    pub rule: Rule,
}

impl Select {
    pub fn rule(&self, name: &str) -> &Rule {
        self.patterns
            .iter()
            .find(|pattern| pattern.name.matches(name))
            .map_or(&self.default, |pattern| &pattern.rule)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patterns {
    /// A const block: `const { ... }`.
    pub r#const: Rule,
    /// A pattern that binds a new variable: `ref mut binding @ SUBPATTERN`.
    pub ident: Rule,
    /// A `ref` or `ref mut` binding, checked in addition to `ident`.
    pub by_ref: Rule,
    /// A literal pattern: `0`.
    pub lit: Rule,
    /// A pattern that matches any one of a set of cases.
    pub or: Rule,
    /// A parenthesized pattern: `(A | B)`.
    pub paren: Rule,
    /// A path pattern like `Color::Red`.
    pub path: Rule,
    /// A range pattern: `1..=2`.
    pub range: Rule,
    /// A reference pattern: `&mut var`.
    pub reference: Rule,
    /// The dots in a tuple or slice pattern: `[0, 1, ..]`.
    pub rest: Rule,
    /// A dynamically sized slice pattern: `[a, b, ref i @ .., y, z]`.
    pub slice: Rule,
    /// A struct or struct variant pattern: `Variant { x, y, .. }`.
    pub r#struct: Rule,
    /// A tuple pattern: `(a, b)`.
    pub tuple: Rule,
    /// A tuple struct or tuple variant pattern: `Variant(x, y, .., z)`.
    pub tuple_struct: Rule,
    /// A type ascription pattern: `foo: f64`.
    pub r#type: Rule,
    /// A pattern that matches any value: `_`.
    pub wild: Rule,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Types {
    /// A fixed size array type: `[T; n]`.
    pub array: Rule,
    /// A bare function type: `fn(usize) -> bool`.
    pub bare_fn: Rule,
    /// A type contained within invisible delimiters.
    pub group: Rule,
    /// An `impl Bound1 + Bound2 + Bound3` type.
    pub impl_trait: Rule,
    /// Indication that a type should be inferred by the compiler: `_`.
    pub infer: Rule,
    /// The never type: `!`.
    pub never: Rule,
    /// A parenthesized type equivalent to the inner type.
    pub paren: Rule,
    /// A path like `std::slice::Iter`.
    pub path: Rule,
    /// A raw pointer type: `*const T` or `*mut T`.
    pub ptr: Rule,
    /// A reference type: `&'a T` or `&'a mut T`.
    pub reference: Rule,
    /// A dynamically sized slice type: `[T]`.
    pub slice: Rule,
    /// A trait object type `dyn Bound1 + Bound2 + Bound3`.
    pub trait_object: Rule,
    /// A tuple type: `(A, B, C, String)`.
    pub tuple: Rule,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Every macro invocation, in any position, selected by its path as written:
    /// `messagef`, `fprime_core::messagef`, `macro_rules`.
    pub macros: Select,
    /// What a `#[rule_waive("why")]` costs: `Allow` to accept it silently, `Warn` to list
    /// every waiver in the build output, `Error` to forbid waivers in this profile.
    pub waivers: Rule,
    pub statements: Statements,
    pub expressions: Expressions,
    pub patterns: Patterns,
    pub types: Types,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_matching_pattern_decides() {
        let select: Select = toml::from_str(
            r#"
            default = { Error = { message = "no" } }
            patterns = [
                { name = "Ref.power.PWR_OFF", rule = { Warn = { message = "careful" } } },
                { name = "Ref.*", rule = "Allow" },
            ]
            "#,
        )
        .unwrap();

        assert!(matches!(
            select.rule("Ref.power.PWR_OFF"),
            Rule::Warn { .. }
        ));
        assert!(matches!(select.rule("Ref.power.PWR_ON"), Rule::Allow));
        assert!(matches!(
            select.rule("CdhCore.cmdDisp.CMD_NO_OP"),
            Rule::Error { .. }
        ));
    }
}

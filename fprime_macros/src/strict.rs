//! Strict mode for the sequencing DSL: `#[fprime(strict)]` / `#[fprime_main(strict)]`.
//!
//! Strict mode holds a sequence to a small subset of Rust that reads as a list
//! of steps: commands, telemetry and parameters, `let` and `const` values,
//! arithmetic, and `if`. Loops, `match`, references, closures, macros, `unsafe`
//! and nested items are rejected.
//!
//! The check runs on the body as written, before `infer_path` expands it — the
//! expansion itself uses `unsafe`, references and `const` items.

use proc_macro2::{Span, TokenStream};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ItemFn, Pat, Stmt, Type, UnOp};

/// Every construct in `item`'s body that strict mode rejects, as one error.
///
/// An item that doesn't parse as a function is left for the compiler to report.
pub(crate) fn check(item: &TokenStream) -> syn::Result<()> {
    let Ok(function) = syn::parse2::<ItemFn>(item.clone()) else {
        return Ok(());
    };

    let mut strict = StrictDsl::default();
    for stmt in &function.block.stmts {
        strict.visit_stmt(stmt);
    }

    strict
        .disallowed
        .into_iter()
        .reduce(|mut all, next| {
            all.combine(next);
            all
        })
        .map_or(Ok(()), Err)
}

#[derive(Default)]
struct StrictDsl {
    disallowed: Vec<syn::Error>,
}

impl StrictDsl {
    fn reject(&mut self, span: Span, message: &str) {
        self.disallowed
            .push(syn::Error::new(span, format!("{message} in strict mode")));
    }
}

impl<'ast> Visit<'ast> for StrictDsl {
    fn visit_expr(&mut self, i: &'ast Expr) {
        match i {
            Expr::Array(_) // [0, 1, 2] array values
            | Expr::Assign(_) // a = 1
            | Expr::Binary(_) // a + b, a && b, a += b, etc
            | Expr::Block(_) // { let inner_var = 0; } TODO(tumbar) Maybe remove this?
            | Expr::Call(_) // set_error_mode()
            | Expr::Cast(_) // 12 as u32. Needed to interface different types from telemetry -> commands
            | Expr::Const(_) // const { ... }
            | Expr::Field(_) // Topology.instance.COMMAND()
            | Expr::Group(_) // <internal Rust magic, not user relevant>
            | Expr::If(_) // The ONLY flow control we have
            | Expr::Index(_) // SomeArray[0]
            | Expr::Let(_) // if let Some(f) = VAR {} TODO(tumbar) We probably don't need this
            | Expr::Lit(_) // Literals (floats, strings, ints etc)
            | Expr::MethodCall(_) // Commands, telemetry, parameters etc.
            | Expr::Paren(_) // (a + b)
            | Expr::Path(_) // Some::Qualified::ENUM, infer_paths will write in the qualifier but it's still valid to put it in manually
            | Expr::Repeat(_) // [0u8; N], repeated array values
            | Expr::Return(_) // returning early from sequence
            | Expr::Struct(_) // Struct parameters: S { a: 0, b: 2 }
            | Expr::Tuple(_) // (1, 2, 3) tuple values
            | Expr::Unary(syn::ExprUnary { op: UnOp::Not(_) | UnOp::Neg(_), .. }) // !x, -x
            => visit::visit_expr(self, i),

            Expr::Unary(_) => self.reject(i.span(), "dereferencing with `*` is not allowed"),
            Expr::Reference(_) => self.reject(i.span(), "references (`&`) are not allowed"),
            Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_) => {
                self.reject(i.span(), "loops are not allowed")
            }
            // Labelled blocks make these valid without a loop.
            Expr::Break(_) | Expr::Continue(_) => {
                self.reject(i.span(), "`break` and `continue` are not allowed")
            }
            Expr::Match(_) => self.reject(i.span(), "`match` is not allowed, use `if` instead"),
            Expr::Closure(_) => self.reject(i.span(), "closures are not allowed"),
            Expr::Macro(_) => self.reject(i.span(), "macros are not allowed"),
            Expr::Unsafe(_) => self.reject(i.span(), "`unsafe` is not allowed"),
            Expr::Try(_) | Expr::TryBlock(_) => self.reject(i.span(), "`?` is not allowed"),
            _ => self.reject(i.span(), "this expression is not allowed"),
        }
    }

    fn visit_stmt(&mut self, i: &'ast Stmt) {
        match i {
            Stmt::Local(_) // let a = <expr>
            | Stmt::Item(syn::Item::Const(_)) // const MY_CONSTANT: u32 = <expr>
            | Stmt::Expr(_, _) // general expressions, see above rules
            => visit::visit_stmt(self, i),

            Stmt::Macro(_) => self.reject(i.span(), "macros are not allowed"),
            _ => self.reject(i.span(), "only `const` may be declared inside a sequence"),
        }
    }

    fn visit_pat(&mut self, i: &'ast Pat) {
        match i {
            Pat::Reference(_) => self.reject(i.span(), "references (`&`) are not allowed"),
            Pat::Ident(ident) if ident.by_ref.is_some() => {
                self.reject(i.span(), "references (`ref`) are not allowed")
            }
            _ => visit::visit_pat(self, i),
        }
    }

    fn visit_type(&mut self, i: &'ast Type) {
        match i {
            Type::Reference(_) | Type::Ptr(_) => {
                self.reject(i.span(), "reference types are not allowed")
            }
            _ => visit::visit_type(self, i),
        }
    }

    /// Macros in any other position: patterns, types.
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        self.reject(i.span(), "macros are not allowed");
    }
}

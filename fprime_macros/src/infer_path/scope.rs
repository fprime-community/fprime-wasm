//! Each command call gets the enums its arguments draw on in scope, for the editor.
//!
//! An editor offers what is *in scope*. A constant reachable only by its full path is offered
//! only that way, which is why `PROC_TYPE_NONE` came back as
//! `Defs::Fw::DpCfg::ProcType::PROC_TYPE_NONE` and nothing else. The macro was never the
//! cause — a plain-Rust control with no macro anywhere behaves identically, measured as
//! `control: plain Rust, nothing typed, enum argument` in `tools/completion_cases.json` — so
//! there was never anything for a special DSL syntax to anchor to. The constants simply were
//! not in scope, and this puts them there.
//!
//! Nothing here reaches the wire: a `use` is inert and the block is transparent. The 64
//! release `.wasm` modules are byte-identical with and without it.

use crate::infer_path::scan::{Chain, Lower, Rewrite, scan};
use crate::infer_path::symbols::{Point, Symbols};
use fprime_dictionary::konst::MAX_DEPTH;
use fprime_dictionary::naming::definition_path;
use fprime_dictionary::{Command, TypeDefinition, TypeName};
use proc_macro2::TokenStream;
use quote::quote;

/// Wraps every command call that has an enum somewhere in its arguments.
///
/// Runs between qualification and either pass two: qualification has to see the call still in
/// command position, and the imports have to be outside whatever pass two puts in its place —
/// the const buffer spells its arguments out again, and the cursor lands in that copy.
pub(super) fn scope(symbols: &Symbols, tokens: TokenStream) -> TokenStream {
    scan(symbols, tokens, &Scope { symbols })
}

struct Scope<'a> {
    symbols: &'a Symbols,
}

impl Lower for Scope<'_> {
    fn chain(&self, chain: &Chain<'_>) -> Rewrite {
        let (Point::Command(command), Some(_)) = (&chain.point, &chain.args) else {
            return Rewrite::Skip;
        };

        let enums = enums_of(self.symbols, command);
        if enums.is_empty() {
            return Rewrite::Skip;
        }

        let imports = enums.iter().map(|name| {
            let path = definition_path(name);
            quote! {
                #[allow(unused_imports)]
                use #path::*;
            }
        });
        let call = chain.written_call();

        // Parenthesised, not a bare block: a `{` where a statement begins is parsed as a block
        // rather than an expression, which would stop `Ref.dpDemo.Dp(..).check();` being a
        // method call on the command's response.
        //
        // The imports come first and depend only on which command this is, so they are the
        // same tokens whether or not rust-analyzer put a cursor marker in the call — which is
        // what keeps the two expansions agreeing up to that cursor. See
        // `expansions_agree_up_to_the_cursor`.
        Rewrite::Call(quote! {
            ({
                #(#imports)*
                #call
            })
        })
    }
}

/// Every enum a bare constant in `command`'s arguments could belong to: the parameter types
/// themselves, the elements of an array, the members of a struct, each followed through any
/// alias. In parameter order and without repeats, so the imports are deterministic.
///
/// Per call, not per function. `DISABLED` is declared by four enums in the reference
/// dictionary, and one glob covering a whole body makes every mention of it ambiguous — which
/// is why globs were rejected the first time this was tried. Two enums in *one* call that
/// share a constant name are fine: colliding globs are only an error where the ambiguous name
/// is used, and qualification has already rewritten every name it recognised.
fn enums_of(symbols: &Symbols, command: &Command) -> Vec<String> {
    let mut found = Vec::new();

    for param in &command.formal_params {
        collect_enums(symbols, &param.type_name, &mut found, 0);
    }

    found
}

fn collect_enums(symbols: &Symbols, ty: &TypeName, found: &mut Vec<String>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }

    match symbols.dictionary.definition(ty) {
        Some(TypeDefinition::Enum(ty)) => {
            if !found.contains(&ty.qualified_name) {
                found.push(ty.qualified_name.clone());
            }
        }
        Some(TypeDefinition::Array(ty)) => {
            collect_enums(symbols, &ty.element_type, found, depth + 1)
        }
        Some(TypeDefinition::Struct(ty)) => {
            for member in &ty.members {
                collect_enums(symbols, &member.type_name, found, depth + 1);
            }
        }
        _ => {}
    }
}

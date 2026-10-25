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
//!
//! A name may also carry as much of its dictionary qualifier as the author cares to write, so
//! `PROC_TYPE_NONE`, `ProcType::PROC_TYPE_NONE` and `Fw::DpCfg::ProcType::PROC_TYPE_NONE` are
//! all that same constant, and a struct argument may be `SeqArgs { .. }` or
//! `Svc::SeqArgs { .. }`. A qualifier that is not the type's own names nothing.
//!
//! Two pipelines run over one dictionary index, each two passes, qualification first. See
//! `.notes/const-command-encoding.md`.
//!
//! * [`scan`] finds the dictionary names in a token stream, and is the only place raw tokens
//!   are read.
//! * [`symbols`] is what a scan resolves them against.
//! * [`name`] decides how much of a name's qualifier the author wrote.
//! * [`qualify`] turns a bare name into a `crate::Defs::..` path — pass one of both.
//! * [`encode`] folds a call whose arguments are all constants into its `Fw::ComBuffer` —
//!   pass two of `#[fprime]`.
//! * [`describe`] turns a point into the `Desc` descriptor a test matches on — pass two of
//!   `#[fprime_test]`.

mod describe;
mod encode;
mod name;
mod qualify;
mod scan;
mod symbols;

use describe::describe;
use encode::encode;
use name::COMPLETION_MARKER;
use proc_macro2::{Span, TokenStream, TokenTree};
use qualify::qualify;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use symbols::Symbols;

const MISSING_DICTIONARY: &str = concat!(
    "the F Prime DSL needs a dictionary to resolve names against, but `",
    "FPRIME_DICTIONARY",
    "` is not set.\n",
    "Call `fprime_build::generate(\"<topology>Dictionary.json\")` from this crate's `build.rs`."
);

/// Rewrite the bare enumerated constants in `item` into fully qualified paths.
pub(crate) fn rewrite(item: TokenStream) -> syn::Result<TokenStream> {
    let symbols = crate_symbols(signature_span(&item))?;

    Ok(encode(symbols, qualify(symbols, item)))
}

/// The `#[fprime_test]` pipeline: qualify, then descriptor-rewrite.
pub(crate) fn rewrite_test_body(body: TokenStream, span: Span) -> syn::Result<TokenStream> {
    let symbols = crate_symbols(span)?;

    // A half-written argument while an editor is typing is not a mistake to report.
    let completing = has_completion_marker(&body);

    Ok(describe(symbols, qualify(symbols, body), completing))
}

/// The symbols for the dictionary this crate's `build.rs` named, or an error at `span`.
fn crate_symbols(span: Span) -> syn::Result<&'static Symbols> {
    let Ok(dictionary) = std::env::var(fprime_dictionary::DICTIONARY_ENV) else {
        return Err(syn::Error::new(span, MISSING_DICTIONARY));
    };

    symbols(Path::new(&dictionary)).map_err(|err| syn::Error::new(span, err))
}

/// Whether an editor is partway through typing somewhere in `tokens`.
fn has_completion_marker(tokens: &TokenStream) -> bool {
    tokens.clone().into_iter().any(|tree| match tree {
        TokenTree::Ident(ident) => ident.to_string().contains(COMPLETION_MARKER),
        TokenTree::Group(group) => has_completion_marker(&group.stream()),
        _ => false,
    })
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
fn symbols(dictionary: &Path) -> Result<&'static Symbols, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, &'static Symbols>>> = OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(|err| err.into_inner());

    if let Some(symbols) = cache.get(dictionary) {
        return Ok(symbols);
    }

    let parsed = fprime_dictionary::try_parse(dictionary)?;
    let symbols: &'static Symbols = Box::leak(Box::new(Symbols::new(parsed)));
    cache.insert(dictionary.to_path_buf(), symbols);

    Ok(symbols)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// The dictionary the workspace tests share.
    const DICTIONARY: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fprime_dictionary/src/test/RefTopologyDictionary.json"
    );

    fn test_symbols() -> syn::Result<&'static Symbols> {
        symbols(Path::new(DICTIONARY)).map_err(|err| syn::Error::new(Span::call_site(), err))
    }

    /// Apply the DSL to a sequence body and render the result as tokens.
    pub(crate) fn rewrite_block(body: &str) -> syn::Result<String> {
        Ok(qualify(test_symbols()?, syn::parse_str(body)?).to_string())
    }

    /// Both passes, in the order `rewrite` runs them: qualify, then const-encode.
    pub(crate) fn const_encode_block(body: &str) -> syn::Result<String> {
        let symbols = test_symbols()?;
        let qualified = qualify(symbols, syn::parse_str(body)?);

        Ok(encode(symbols, qualified).to_string())
    }

    /// Renders a body without rewriting it, for writing expectations as ordinary Rust.
    pub(crate) fn render_block(body: &str) -> syn::Result<String> {
        Ok(syn::parse_str::<TokenStream>(body)?.to_string())
    }

    /// The `#[fprime_test]` pipeline exactly as the macro runs it, marker detection
    /// included — which is what [`descriptor_block`] takes as a parameter instead.
    pub(crate) fn test_body(body: &str) -> syn::Result<String> {
        let symbols = test_symbols()?;
        let tokens: TokenStream = syn::parse_str(body)?;
        let completing = has_completion_marker(&tokens);
        let qualified = qualify(symbols, tokens);

        Ok(describe(symbols, qualified, completing).to_string())
    }

    /// Both passes in the order `#[fprime_test]` runs them: qualify, then descriptor.
    ///
    /// `completing` stands in for an editor mid-keystroke, which the marker itself signals in
    /// the real thing — see [`rewrite_test_body`].
    pub(crate) fn descriptor_block(body: &str, completing: bool) -> syn::Result<String> {
        let symbols = test_symbols()?;
        let qualified = qualify(symbols, syn::parse_str(body)?);

        Ok(describe(symbols, qualified, completing).to_string())
    }
}

//! `#[fprime_test]`: expands to a `#[test]` that runs one sequence and checks what it did.
//!
//! ```ignore
//! #[fprime_test(sequence = "safing")]
//! fn retries_power_off_once(t: Test) {
//!     t.expect_command(Ref.power.PWR_OFF()).responds(EXECUTION_ERROR);
//!     t.expect_exit(0);
//! }
//! ```
//!
//! `limits = "..."` names a `sequencer.toml` other than the project's, for a deployment that
//! configures more than one `WasmSequencer` instance:
//!
//! ```ignore
//! #[fprime_test(sequence = "safing", limits = "sequencer-payload.toml")]
//! ```

use crate::infer_path;
use proc_macro2::{Literal, Span, TokenStream, TokenTree};
use quote::quote;
use std::path::Path;
use syn::spanned::Spanned;

/// What to say when the attribute is not `key = "value"` pairs, or says nothing at all.
const USAGE: &str = "#[fprime_test] needs the sequence it is about, and optionally the \
                     configuration to run it under: `#[fprime_test(sequence = \"safing\", \
                     limits = \"sequencer-b.toml\")]`";

/// One `key = "..."` the author wrote: its text, and the literal whose span a goto link
/// borrows. See [`file_link`].
#[derive(Debug)]
struct Arg {
    text: String,
    literal: Literal,
}

/// What the attribute said.
#[derive(Debug)]
struct Attribute {
    /// `sequence = "..."`, the only required argument.
    sequence: Arg,
    /// `limits = "..."`, a `sequencer.toml` relative to the crate root.
    limits: Option<Arg>,
}

/// Why a value has to be a string, per argument — a bare number here is usually someone
/// reaching for a limit rather than the file that holds it.
fn must_be_a_string(key: &str) -> &'static str {
    match key {
        "sequence" => "`sequence` must be a string, naming the sequence this test is about",
        _ => "`limits` must be a string, naming a sequencer.toml relative to the crate root",
    }
}

/// The `key = "..."` pairs, in any order, with a trailing comma allowed.
fn parse(attr: TokenStream) -> syn::Result<Attribute> {
    // Nowhere better to point when the shape itself is wrong.
    let whole = attr.span();
    let mut trees = attr.into_iter();
    let mut sequence: Option<Arg> = None;
    let mut limits: Option<Arg> = None;

    loop {
        let (key, value) = match (trees.next(), trees.next(), trees.next()) {
            (None, _, _) => break,
            (
                Some(TokenTree::Ident(key)),
                Some(TokenTree::Punct(equals)),
                Some(TokenTree::Literal(value)),
            ) if equals.as_char() == '=' => (key, value),
            _ => return Err(syn::Error::new(whole, USAGE)),
        };

        let name = key.to_string();
        let slot = match name.as_str() {
            "sequence" => &mut sequence,
            "limits" => &mut limits,
            other => {
                return Err(syn::Error::new_spanned(
                    key,
                    format!("`{other}` is not an #[fprime_test] argument. {USAGE}"),
                ));
            }
        };
        if slot.is_some() {
            return Err(syn::Error::new_spanned(
                key,
                format!("`{name}` is given twice"),
            ));
        }
        let syn::Lit::Str(text) = syn::Lit::new(value.clone()) else {
            return Err(syn::Error::new_spanned(value, must_be_a_string(&name)));
        };
        *slot = Some(Arg {
            text: text.value(),
            literal: value,
        });

        match trees.next() {
            None => break,
            Some(TokenTree::Punct(comma)) if comma.as_char() == ',' => {}
            _ => return Err(syn::Error::new(whole, USAGE)),
        }
    }

    let Some(sequence) = sequence else {
        return Err(syn::Error::new(whole, USAGE));
    };
    Ok(Attribute { sequence, limits })
}

/// Makes goto-definition on a string in the attribute open the file that string names.
///
/// A span pointing into another file cannot be minted: a macro's spans are only ever borrowed
/// from its input. So the path literal of an `include_str!` is given the span of the author's
/// `"safing"`, and rust-analyzer's include-path goto resolves that to the file. Two
/// constraints hold this exact shape in place, both of them measured by
/// `tools/goto_probe.py`:
///
/// * The whole path has to be in that one literal, because rust-analyzer resolves the literal
///   it found the span on and not what the path expression composes to.
///   `concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/safing.rs")` is the tidier thing to emit,
///   and it compiles, but the goto goes dead.
/// * That literal has to be absolute. A relative path would be resolved against the file the
///   author is writing in, and `Span::local_file()` is `None` under rust-analyzer's proc-macro
///   server — so the link would work under `cargo test` and never in the editor, which is the
///   only place it is for.
///
/// `.len()` in a `const` keeps the file's text out of the test binary: it is read at compile
/// time and folded to a number. `const _` is anonymous, so one link per argument is fine.
fn file_link(literal: &Literal, file: &str) -> TokenStream {
    let mut path = Literal::string(file);
    path.set_span(literal.span());

    quote! {
        const _: usize = include_str!(#path).len();
    }
}

/// The crate being compiled: the sequence crate. Both rustc and rust-analyzer's proc-macro
/// server set it.
fn manifest_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("CARGO_MANIFEST_DIR").map(std::path::PathBuf::from)
}

/// Links `sequence = "safing"` to `src/bin/safing.rs`.
fn source_link(sequence: &Arg) -> TokenStream {
    let source = manifest_dir().and_then(|manifest| sequence_source(&manifest, &sequence.text));

    let Some(source) = source else {
        // Nothing to point at, so nothing is emitted: a sequence declared with its own
        // `[[bin]] path`, or a name that is simply wrong. The wrong name is left to
        // `fprime_test::locate`, which reports it at run time alongside the names that exist.
        return TokenStream::new();
    };
    file_link(&sequence.literal, &source)
}

/// Links `limits = "sequencer-b.toml"` to that file, resolved against the crate root — the
/// same place `sequencer.toml` lives, and the same join `fprime_test` makes at run time.
///
/// A path that is not there fails here rather than at run time. Unlike `sequence`, which may
/// name a bin declared elsewhere, this literal *is* the file: there is nothing else it could
/// mean, and a typo would otherwise fail every test that names it.
fn limits_link(limits: &Arg) -> syn::Result<TokenStream> {
    let Some(manifest) = manifest_dir() else {
        // Nothing to resolve against. Left to run time, which reports the same thing.
        return Ok(TokenStream::new());
    };

    let path = manifest.join(&limits.text);
    if !path.is_file() {
        return Err(syn::Error::new_spanned(
            &limits.literal,
            format!(
                "no configuration at {}. `limits` names a sequencer.toml relative to the crate \
                 root, alongside the default one",
                path.display()
            ),
        ));
    }
    Ok(file_link(&limits.literal, &path.to_string_lossy()))
}

/// Where `fprime-wasm add` puts a sequence's source, if there is a file there.
///
/// `manifest` is `CARGO_MANIFEST_DIR`: the crate being compiled, which is the sequence crate.
/// Both rustc and rust-analyzer's proc-macro server set it. Mirrors
/// `fprime_test::project::Project::sequence_source`, which a proc macro cannot depend on.
fn sequence_source(manifest: &Path, name: &str) -> Option<String> {
    let source = manifest.join("src").join("bin").join(format!("{name}.rs"));

    source
        .is_file()
        .then(|| source.to_string_lossy().into_owned())
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let attribute = parse(attr)?;
    let function: syn::ItemFn = syn::parse2(item)?;
    let name = function.sig.ident.clone();

    if function.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "an #[fprime_test] function takes exactly one argument, the `Test` it builds: \
             `fn my_test(t: Test)`",
        ));
    }
    let argument = function.sig.inputs.first().expect("checked above").clone();

    let body = infer_path::rewrite_test_body(function.block.to_token_stream_stream(), name.span())?;
    let attributes = function.attrs.clone();
    let test_name = name.to_string();
    let sequence_literal = Literal::string(&attribute.sequence.text);
    let source_link = source_link(&attribute.sequence);

    // The name as written, so the failure report calls the file what the author called it.
    let (limits, limits_link) = match &attribute.limits {
        Some(limits) => {
            let named = Literal::string(&limits.text);
            (quote! { Some(#named) }, limits_link(limits)?)
        }
        None => (quote! { None }, TokenStream::new()),
    };

    Ok(quote! {
        #(#attributes)*
        #[test]
        fn #name() {
            // Turns a missing `use <crate>::*;` into one clear error instead of many
            // `cannot find value`s, one per dictionary path.
            #[allow(unused_imports)]
            use crate::Desc as __fprime_test_needs_the_sequence_crate_glob_import;

            #source_link
            #limits_link

            fn __fprime_body(#argument) #body

            ::fprime_test::run(
                ::fprime_test::Case {
                    sequence: #sequence_literal,
                    test: #test_name,
                    manifest_dir: env!("CARGO_MANIFEST_DIR"),
                    // Set by `fprime_build` as a `cargo::rustc-env` for every target in the package.
                    dictionary: env!("FPRIME_DICTIONARY"),
                    limits: #limits,
                },
                __fprime_body,
            );
        }
    })
}

/// `syn::Block` to tokens, kept local so the trait import does not leak.
trait BlockTokens {
    fn to_token_stream_stream(&self) -> TokenStream;
}

impl BlockTokens for syn::Block {
    fn to_token_stream_stream(&self) -> TokenStream {
        use quote::ToTokens;
        let mut tokens = TokenStream::new();
        self.to_tokens(&mut tokens);
        tokens
    }
}

/// Span of the whole attribute, for an error with nowhere better to point.
#[allow(dead_code)]
fn call_site() -> Span {
    Span::call_site()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_name_requires_string() {
        let attribute = parse(quote! { sequence = "safing" }).expect("valid");
        assert_eq!(attribute.sequence.text, "safing");
        assert!(attribute.limits.is_none());

        let err = parse(quote! {}).expect_err("empty is not valid");
        assert!(err.to_string().contains("sequence = \"safing\""), "{err}");

        let err = parse(quote! { sequence = 3 }).expect_err("a number is not a name");
        assert!(err.to_string().contains("must be a string"), "{err}");

        let err = parse(quote! { name = "safing" }).expect_err("wrong key");
        assert!(err.to_string().contains("sequence"), "{err}");
    }

    /// `limits` is optional, and order is the author's business.
    #[test]
    fn limits_is_optional_and_order_free() {
        let attribute =
            parse(quote! { sequence = "safing", limits = "sequencer-b.toml" }).expect("valid");
        assert_eq!(attribute.sequence.text, "safing");
        assert_eq!(
            attribute.limits.expect("limits was given").text,
            "sequencer-b.toml"
        );

        let attribute =
            parse(quote! { limits = "sequencer-b.toml", sequence = "safing" }).expect("valid");
        assert_eq!(attribute.sequence.text, "safing");
        assert!(attribute.limits.is_some());

        // A trailing comma is what an editor leaves behind when a line is added.
        let attribute = parse(quote! { sequence = "safing", }).expect("valid");
        assert_eq!(attribute.sequence.text, "safing");
    }

    /// `limits` without a sequence is still not a test of anything.
    #[test]
    fn limits_alone_is_not_enough() {
        let err = parse(quote! { limits = "sequencer-b.toml" }).expect_err("no sequence");
        assert!(err.to_string().contains("sequence = \"safing\""), "{err}");
    }

    /// A number here is someone reaching for the limit rather than the file holding it.
    #[test]
    fn limits_names_a_file_not_a_value() {
        let err = parse(quote! { sequence = "safing", limits = 10000 })
            .expect_err("a number is not a file");
        assert!(err.to_string().contains("sequencer.toml"), "{err}");
    }

    #[test]
    fn an_argument_given_twice_is_an_error() {
        let err = parse(quote! { sequence = "safing", sequence = "startup" })
            .expect_err("which one would win?");
        assert!(err.to_string().contains("given twice"), "{err}");
    }

    #[test]
    fn a_stray_token_between_arguments_is_an_error() {
        let err = parse(quote! { sequence = "safing" limits = "sequencer-b.toml" })
            .expect_err("a missing comma");
        assert!(err.to_string().contains("#[fprime_test]"), "{err}");
    }

    /// The link is what makes the string a link; without a file there is nothing to point at.
    #[test]
    fn limits_that_is_not_there_fails_at_compile_time() {
        let attribute = parse(quote! { sequence = "safing", limits = "no-such-sequencer.toml" })
            .expect("valid");
        let err = limits_link(&attribute.limits.expect("limits was given"))
            .expect_err("the file is not in this crate");
        let message = err.to_string();
        assert!(message.contains("no-such-sequencer.toml"), "{message}");
        assert!(message.contains("crate root"), "{message}");
    }

    /// A file that *is* there links to its absolute path, for the reasons in `file_link`.
    #[test]
    fn limits_links_to_an_absolute_path() {
        // Every crate has one of these, so no fixture is needed.
        let attribute =
            parse(quote! { sequence = "safing", limits = "Cargo.toml" }).expect("valid");
        let link = limits_link(&attribute.limits.expect("limits was given"))
            .expect("Cargo.toml is there")
            .to_string();

        assert!(link.contains("include_str"), "{link}");
        let manifest = manifest_dir().expect("cargo sets it");
        assert!(
            link.contains(&manifest.join("Cargo.toml").to_string_lossy().into_owned()),
            "{link}"
        );
    }

    #[test]
    fn source_is_found_under_src_bin() {
        let manifest = std::env::temp_dir().join(format!(
            "fprime-macros-source-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(manifest.join("src/bin")).expect("temp dir");
        std::fs::write(manifest.join("src/bin/safing.rs"), b"fn main() {}").expect("write");

        let source = sequence_source(&manifest, "safing").expect("the file is there");
        assert!(source.ends_with("src/bin/safing.rs"), "{source}");
        // Absolute, because a relative path would be resolved against the test file rather
        // than the crate root — see `source_link`.
        assert!(Path::new(&source).is_absolute(), "{source}");

        assert_eq!(sequence_source(&manifest, "startup"), None);
        std::fs::remove_dir_all(&manifest).ok();
    }

    #[test]
    fn a_sequence_with_no_source_links_to_nothing() {
        // This crate has no `src/bin`, so nothing can be pointed at, and the expansion has to
        // stay valid regardless.
        let link = source_link(
            &parse(quote! { sequence = "safing" })
                .expect("valid")
                .sequence,
        );
        assert!(link.is_empty(), "{link}");
    }
}

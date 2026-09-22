//! A golden record of what the DSL expands every shape in the dictionary into.
//!
//! The `#[fprime]` half of `infer_path` is already proved by other means: 17 exact-token
//! tests pin qualification, and the 64 release `.wasm` modules built from `crates/bench`,
//! `crates/example` and `crates/interp` are byte-identical or they are not. The
//! `#[fprime_test]` half has neither — a descriptor expansion compiles into a host test
//! binary, so there is nothing to checksum, and the bare-descriptor arm, both channel and
//! parameter points, and most of the diagnostics had no test at all.
//!
//! So this captures both pipelines over one corpus and diffs the result. A checksum
//! mismatch says "something moved"; a golden diff says what.
//!
//! The sequence corpus is `crates/bench/src/bin/`, read at test time rather than
//! transcribed: those 44 bins already are one example of every shape the generator has a
//! distinct path for, fallbacks included, and reading them means the two cannot drift. The
//! test corpus is written out below, because no bench bin contains a `t.expect_command(..)`.
//!
//! Re-bless with `FPRIME_UPDATE_REF=1 cargo test -p fprime_macros`, the same escape hatch
//! `fprime_dictionary` and `fprime_build` use for their reference files.

use crate::infer_path::test_support::{const_encode_block, test_body};
use pretty_assertions::assert_eq;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Test bodies, which no bench bin covers. Each names the path through `descriptor_for`
/// that it is here for.
const TEST_BODIES: &[(&str, &str)] = &[
    (
        "const call",
        "t.expect_command(CdhCore.cmdDisp.CMD_NO_OP());",
    ),
    (
        "const call with a string",
        r#"t.expect_command(CdhCore.cmdDisp.CMD_NO_OP_STRING("hello"));"#,
    ),
    (
        "bare command",
        "t.expect_command(CdhCore.cmdDisp.CMD_NO_OP);",
    ),
    ("bare command, never", "t.never_command(Ref.wasmSeq.LOAD);"),
    (
        "bare channel",
        "t.expect_telemetry_read(CdhCore.events.EventsDropped);",
    ),
    (
        "channel with a value",
        "t.initial_telemetry(CdhCore.events.EventsDropped, 5);",
    ),
    (
        "bare parameter",
        "t.initial_parameter(Ref.wasmSeq.INSTRUCTION_FUEL, 20);",
    ),
    (
        "struct argument, partly qualified",
        "t.never_command(Ref.typeDemo.CHOICE_PAIR(Ref::ChoicePair {
             firstChoice: Choice::RED,
             secondChoice: TWO,
         }));",
    ),
    // --- the reporting path. Every one of these is a diagnostic that had no test.
    (
        "runtime argument",
        "t.expect_command(Ref.wasmSeq.LOAD(filename));",
    ),
    (
        "wrong arity",
        "t.expect_command(Ref.dpDemo.Dp(IMMEDIATE, 0));",
    ),
    (
        "a parameter a test cannot write out",
        r#"t.expect_command(Ref.typeDemo.SEND_NAMES(["a", "b"]));"#,
    ),
    (
        "a struct member a test cannot write out",
        r#"t.expect_command(Ref.typeDemo.SEND_COLOR_INFO(ColorInfoStruct {
             color: RED,
             name: "crimson",
         }));"#,
    ),
    (
        "array of the wrong length",
        "t.expect_command(Ref.typeDemo.CHOICES([ONE, TWO, ONE]));",
    ),
    (
        "array length that is not written out",
        "t.expect_command(Ref.typeDemo.CHOICES([ONE; count]));",
    ),
    (
        "struct completed from elsewhere",
        "t.expect_command(Ref.typeDemo.CHOICE_PAIR(ChoicePair {
             firstChoice: RED,
             ..other
         }));",
    ),
    (
        "struct with a member missing",
        "t.expect_command(Ref.typeDemo.CHOICE_PAIR(ChoicePair { firstChoice: RED }));",
    ),
    (
        "struct named by a qualifier that is not its own",
        "t.expect_command(Ref.typeDemo.CHOICE_PAIR(Other::ChoicePair {
             firstChoice: RED,
             secondChoice: TWO,
         }));",
    ),
    (
        "a bool that is not written out",
        "t.expect_command(Ref.typeDemo.SEND_BOOL(1));",
    ),
    (
        "a number that is not written out",
        "t.expect_command(CdhCore.cmdDisp.CMD_TEST_CMD_1(count + 1, 0.0, 0));",
    ),
    (
        "an enum constant the dictionary does not have",
        "t.expect_command(Ref.dpDemo.SelectColor(MAUVE));",
    ),
];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate is in a workspace")
        .to_path_buf()
}

/// Every `#[fprime_main]` bin in `crates/bench`, by name, sorted so the golden is stable.
fn sequences() -> Vec<(String, String)> {
    let bins = repo().join("crates/bench/src/bin");
    let mut found: Vec<(String, String)> = fs::read_dir(&bins)
        .unwrap_or_else(|err| panic!("{}: {err}", bins.display()))
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            let name = path
                .file_stem()
                .expect("a file with an extension has a stem")
                .to_string_lossy()
                .into_owned();
            (name, item(&path))
        })
        .collect();

    found.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(
        found.len() > 40,
        "expected the bench corpus, found {found:?}"
    );

    found
}

/// The `pub fn main() { .. }` item out of a bench bin: everything after the attribute, so
/// the inner attributes and the `use` are left behind but the whole body is kept.
fn item(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let at = text
        .find("#[fprime_main]")
        .unwrap_or_else(|| panic!("{}: no #[fprime_main]", path.display()));

    text[at + "#[fprime_main]".len()..].trim().to_string()
}

/// Renders an expansion so a golden diff reads like code rather than one long line.
///
/// Written by hand rather than reached for from a crate: several corpus entries do not
/// parse as Rust — that is what they are here for — and a `compile_error!` message carries
/// braces and newlines of its own that string munging would corrupt.
fn render(tokens: TokenStream) -> String {
    let mut out = String::new();
    write_stream(tokens, 0, &mut out);

    out
}

fn write_stream(tokens: TokenStream, depth: usize, out: &mut String) {
    let mut line = String::new();

    for tree in tokens {
        match tree {
            // A brace is a block: its contents get their own lines, one per statement.
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
                push(&mut line, "{");
                flush(&mut line, depth, out);
                write_stream(group.stream(), depth + 1, out);
                push(&mut line, "}");
            }
            // Everything else stays inline: an argument list on its own line reads worse.
            TokenTree::Group(group) => {
                let (open, close) = delimiters(group.delimiter());
                push(&mut line, open);
                line.push_str(&inline(group.stream()));
                line.push_str(close);
            }
            TokenTree::Punct(punct) if punct.as_char() == ';' => {
                line.push(';');
                flush(&mut line, depth, out);
            }
            other => push(&mut line, &other.to_string()),
        }
    }

    flush(&mut line, depth, out);
}

/// A stream on one line, groups included.
fn inline(tokens: TokenStream) -> String {
    let mut line = String::new();

    for tree in tokens {
        match tree {
            TokenTree::Group(group) => {
                let (open, close) = delimiters(group.delimiter());
                push(&mut line, open);
                line.push_str(&inline(group.stream()));
                line.push_str(close);
            }
            other => push(&mut line, &other.to_string()),
        }
    }

    line
}

fn delimiters(delimiter: Delimiter) -> (&'static str, &'static str) {
    match delimiter {
        Delimiter::Parenthesis => ("(", ")"),
        Delimiter::Bracket => ("[", "]"),
        Delimiter::Brace => ("{", "}"),
        Delimiter::None => ("", ""),
    }
}

/// Enough spacing sense to render `crate::Konst::X` rather than `crate : : Konst : : X`.
/// Only the record's legibility rests on this — a `TokenStream` has no opinion.
fn push(line: &mut String, text: &str) {
    const NO_SPACE_AFTER: &[char] = &['.', ':', '&', '<', '#', '!', '(', '['];
    const NO_SPACE_BEFORE: &[char] = &['.', ',', ';', ':', '>', '!', '(', '[', ')', ']'];

    let joined = line
        .chars()
        .last()
        .is_some_and(|last| NO_SPACE_AFTER.contains(&last))
        || text.starts_with(NO_SPACE_BEFORE);

    if !line.is_empty() && !joined {
        line.push(' ');
    }
    line.push_str(text);
}

fn flush(line: &mut String, depth: usize, out: &mut String) {
    if !line.trim().is_empty() {
        out.push_str(&"    ".repeat(depth));
        out.push_str(line.trim());
        out.push('\n');
    }
    line.clear();
}

/// One corpus entry's heading and rendered expansion.
fn section(kind: &str, name: &str, expansion: &str) -> String {
    let tokens: TokenStream = syn::parse_str(expansion)
        .unwrap_or_else(|err| panic!("{kind} {name}: the expansion does not lex: {err}"));

    format!("// ===== {kind}: {name}\n{}\n", render(tokens))
}

fn golden() -> String {
    let mut out = String::from(
        "// Generated by `cargo test -p fprime_macros`; re-bless with FPRIME_UPDATE_REF=1.\n\
         // See fprime_macros/src/test/dsl_corpus.rs for what each section is here for.\n",
    );

    for (name, body) in sequences() {
        let expansion =
            const_encode_block(&body).unwrap_or_else(|err| panic!("sequence {name}: {err}"));
        out.push('\n');
        out.push_str(&section("sequence", &name, &expansion));

        // The same shape through the descriptor pass. A sequence body is not what an author
        // would write in a test, but running every shape through it is what reaches the
        // bare-point arm, both channel and parameter points, and the `()` after a channel
        // that `spanned_desc_path` names as a common mistake.
        let descriptor = test_body(&body).unwrap_or_else(|err| panic!("descriptor {name}: {err}"));
        out.push_str(&section("descriptor", &name, &descriptor));
    }

    for (name, body) in TEST_BODIES {
        let expansion = test_body(body).unwrap_or_else(|err| panic!("test body {name}: {err}"));
        out.push('\n');
        out.push_str(&section("test body", name, &expansion));
    }

    out
}

/// Compares every shape's expansion against the checked-in reference.
#[test]
fn dsl_expansions_match_reference() {
    let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test/dsl_corpus.ref.rs");
    let output = golden();

    match env::var("FPRIME_UPDATE_REF") {
        Ok(_) => fs::write(&reference, output).expect("failed to write dsl_corpus.ref.rs"),
        Err(_) => {
            let blessed = fs::read_to_string(&reference).expect("failed to read dsl_corpus.ref.rs");
            assert_eq!(blessed, output)
        }
    }
}

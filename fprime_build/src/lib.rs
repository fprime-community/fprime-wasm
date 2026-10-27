use crate::tree::{CodeTree, Definition};
use fprime_dictionary::TypeDefinition;
use proc_macro2::TokenStream;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::{env, fs};

mod commands;
mod constants;
mod desc;
mod konst;
mod parameters;
mod telemetry;
mod tree;
mod types;
mod util;
mod values;

/// The flight API a sequence is built against: types, accessors, const encoders.
pub const DICTIONARY: &str = "dictionary.rs";

/// The test descriptors file, excluded from the flight translation unit.
pub const DESCRIPTORS: &str = "descriptors.rs";

/// Serialize a token stream into a string
fn render_tokens(ts: TokenStream) -> String {
    let s = ts.to_string();
    match syn::parse_file(&ts.to_string()) {
        Ok(parsed) => prettyplease::unparse(&parsed),
        Err(err) => format!("{}\n// {}\n", s, err.to_string()),
    }
}

pub(crate) fn generate_to_file<W: ?Sized + Write>(
    dict: &fprime_dictionary::Dictionary,
    writer: &mut BufWriter<W>,
) {
    if !matches!(
        dict.type_definitions.get("Fw.CmdResponse"),
        Some(TypeDefinition::Enum(_))
    ) {
        panic!("Fw.CmdResponse enum not found in dictionary");
    }

    let mut definitions = vec![];

    // Sorted for stable output across runs; the map itself is unordered.
    let mut type_names: Vec<&String> = dict.type_definitions.keys().collect();
    type_names.sort();

    for name in type_names {
        let (qualifier, tokens) = types::type_definition(&dict.type_definitions[name]);
        definitions.push(Definition { qualifier, tokens });
    }

    for c in &dict.constants {
        let (qualifier, tokens) = constants::constant(c, &dict);
        definitions.push(Definition { qualifier, tokens });
    }

    let mut cmds = vec![];
    let mut tlms = vec![];
    let mut prms = vec![];
    let mut konsts = vec![];

    for cmd in &dict.commands {
        let (qualifier, tokens) = commands::command(cmd);
        cmds.push(Definition { qualifier, tokens });

        if let Some((qualifier, tokens)) = konst::command(&dict, cmd) {
            konsts.push(Definition { qualifier, tokens });
        }
    }

    for tlm in &dict.telemetry_channels {
        let (qualifier, tokens) = telemetry::telemetry_channel(tlm);
        tlms.push(Definition { qualifier, tokens });
    }

    for prm in &dict.parameters {
        let (qualifier, tokens) = parameters::parameter(prm);
        prms.push(Definition { qualifier, tokens });
    }

    let tokens: TokenStream = util::global_helpers(dict)
        .into_iter()
        .chain(Into::<CodeTree>::into(definitions).module_nesting("Dict"))
        .chain(Into::<CodeTree>::into(cmds).struct_nesting("Cmd"))
        .chain(Into::<CodeTree>::into(tlms).struct_nesting("Tlm"))
        .chain(Into::<CodeTree>::into(prms).struct_nesting("Prm"))
        .chain(Into::<CodeTree>::into(konsts).module_nesting("Konst"))
        .collect();

    writer
        .write(render_tokens(tokens).as_bytes())
        .expect("failed to write to file");

    writer.flush().expect("failed to flush file")
}

/// The `Desc` tree: one `const` per dictionary point, for `#[fprime_test]` to resolve against,
/// under `Desc::Cmd`, `Desc::Tlm` and `Desc::Prm` as the flight API's `Cmd`, `Tlm` and `Prm`.
pub(crate) fn generate_descriptors_to_file<W: ?Sized + Write>(
    dict: &fprime_dictionary::Dictionary,
    writer: &mut BufWriter<W>,
) {
    let mut descs = vec![];

    for cmd in &dict.commands {
        let (qualifier, tokens) = desc::command(cmd);
        descs.push(Definition { qualifier, tokens });
    }
    for tlm in &dict.telemetry_channels {
        let (qualifier, tokens) = desc::telemetry_channel(dict, tlm);
        descs.push(Definition { qualifier, tokens });
    }
    for prm in &dict.parameters {
        let (qualifier, tokens) = desc::parameter(dict, prm);
        descs.push(Definition { qualifier, tokens });
    }
    if let Some((qualifier, tokens)) = desc::responses(dict) {
        descs.push(Definition { qualifier, tokens });
    }

    let descs: CodeTree = descs.into();
    let tokens = descs.module_nesting("Desc");

    writer
        .write_all(render_tokens(tokens).as_bytes())
        .expect("failed to write the descriptors");
    writer.flush().expect("failed to flush the descriptors")
}

pub fn generate(dictionary_json: &str) {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join(DICTIONARY);
    println!("cargo::rerun-if-changed=build.rs");

    let dictionary_path = fs::canonicalize(dictionary_json).unwrap_or_else(|err| {
        panic!(
            "failed to resolve dictionary '{}': {}",
            dictionary_json, err
        )
    });

    let dict = fprime_dictionary::parse(&dictionary_path);

    let file = fs::File::create(dest_path).expect("failed to open destination file");
    let mut writer = BufWriter::new(file);

    generate_to_file(&dict, &mut writer);

    // Written unconditionally; whether it is *compiled* is the including crate's `cfg`.
    let descriptors = Path::new(&out_dir).join(DESCRIPTORS);
    let file = fs::File::create(descriptors).expect("failed to open the descriptors file");
    let mut writer = BufWriter::new(file);
    generate_descriptors_to_file(&dict, &mut writer);

    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed={}", dictionary_json);
    println!(
        "cargo::rustc-env={}={}",
        fprime_dictionary::DICTIONARY_ENV,
        dictionary_path.display()
    );
}

#[cfg(test)]
mod test {
    mod test;
}

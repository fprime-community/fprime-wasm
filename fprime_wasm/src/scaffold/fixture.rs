//! A plan and its rendered files, shared by the tests in this module.

use super::{DEFAULT_STACK_SIZE, DependencySpec, Plan};
use std::path::PathBuf;
use toml_edit::DocumentMut;

pub fn plan() -> Plan {
    Plan {
        name: "ref-sequences".into(),
        sequence: "startup".into(),
        dictionary: PathBuf::from("dictionary/RefTopologyDictionary.json"),
        dependency: DependencySpec::Version("1.2.3".into()),
        stack_size: DEFAULT_STACK_SIZE,
    }
}

/// The rendered contents of one generated file.
pub fn file(plan: &Plan, path: &str) -> String {
    plan.files()
        .expect("templates render")
        .into_iter()
        .find(|file| file.path == *path)
        .unwrap_or_else(|| panic!("{path} should be generated"))
        .contents
}

/// The generated manifest, parsed rather than substring-matched, so assertions are
/// about structure and a malformed template cannot pass by containing the right
/// text.
pub fn manifest(plan: &Plan) -> DocumentMut {
    file(plan, "Cargo.toml")
        .parse()
        .expect("the generated Cargo.toml should be valid TOML")
}

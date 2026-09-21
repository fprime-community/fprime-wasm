//! What `init` generates, as values rather than files.

use super::manifest::{DependencySpec, repoint_at_checkout};
use super::template;
use anyhow::{Context, Result};
use fprime_test::project::TARGET;
use std::path::{Path, PathBuf};

/// A file to create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: PathBuf,
    pub contents: String,
    pub executable: bool,
}

/// What `init` was asked to build.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Crate name, as `[package] name`.
    pub name: String,
    /// First sequence to create.
    pub sequence: String,
    /// Dictionary path, relative to the crate root.
    pub dictionary: PathBuf,
    /// How to depend on `fprime_core` and `fprime_build`.
    pub dependency: DependencySpec,
    pub stack_size: usize,
}

impl Plan {
    /// Cargo's library name: `-` replaced with `_`.
    pub fn lib_name(&self) -> String {
        self.name.replace('-', "_")
    }

    /// A value for every placeholder in [`template::KEYS`].
    fn values(&self) -> Vec<(&'static str, String)> {
        vec![
            ("crate_name", self.name.clone()),
            ("lib_name", self.lib_name()),
            ("sequence", self.sequence.clone()),
            ("target", TARGET.to_string()),
            ("stack_size", self.stack_size.to_string()),
            ("dictionary", self.dictionary.display().to_string()),
            // Avoids an invalid escape in the embedded Rust string literal.
            (
                "dictionary_slashes",
                self.dictionary.display().to_string().replace('\\', "/"),
            ),
            // Always a version; `repoint_at_checkout` patches it for the path case.
            (
                "crate_version",
                match &self.dependency {
                    DependencySpec::Version(version) => version.clone(),
                    DependencySpec::Path(_) => "0.0.0".to_string(),
                },
            ),
        ]
    }

    /// Every file `init` creates, in a stable order.
    pub fn files(&self) -> Result<Vec<File>> {
        let values = self.values();
        let mut files: Vec<File> = template::PROJECT
            .iter()
            .map(|entry| {
                let path = template::render(entry.path, &values)
                    .with_context(|| format!("in the path of template {}", entry.path))?;
                let contents = template::render(entry.body, &values)
                    .with_context(|| format!("in template {}", entry.path))?;
                Ok(File {
                    // Platform separator, not the `/` templates use.
                    path: path.split('/').collect(),
                    contents,
                    executable: entry.executable,
                })
            })
            .collect::<Result<_>>()?;

        if let DependencySpec::Path(checkout) = &self.dependency {
            let manifest = files
                .iter_mut()
                .find(|file| file.path == Path::new("Cargo.toml"))
                .context("the project templates no longer include a Cargo.toml")?;
            manifest.contents = repoint_at_checkout(&manifest.contents, checkout)?;
        }

        Ok(files)
    }
}

/// One sequence's source, for `add` and for the starter one `init` writes.
pub fn sequence(lib_name: &str, name: &str) -> Result<String> {
    render_for_sequence(template::SEQUENCE.body, lib_name, name).context("in the sequence template")
}

/// One sequence's tests, written beside its source by both `add` and `init`.
pub fn test(lib_name: &str, name: &str) -> Result<String> {
    render_for_sequence(template::TEST.body, lib_name, name).context("in the test template")
}

/// The two keys a per-sequence template uses.
fn render_for_sequence(body: &str, lib_name: &str, name: &str) -> Result<String> {
    let values = vec![
        ("lib_name", lib_name.to_string()),
        ("sequence", name.to_string()),
    ];
    template::render(body, &values)
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{file, manifest, plan};
    use super::*;
    use fprime_test::project::WASM_FEATURE;

    #[test]
    fn generates_expected_files() {
        let paths: Vec<String> = plan()
            .files()
            .expect("templates render")
            .iter()
            .map(|file| file.path.display().to_string().replace('\\', "/"))
            .collect();
        assert_eq!(
            paths,
            vec![
                "Cargo.toml",
                ".cargo/config.toml",
                ".cargo/wasm-link",
                "sequencer.toml",
                "build.rs",
                "src/lib.rs",
                "src/bin/startup.rs",
                "tests/startup.rs",
                ".gitignore",
                ".vscode/settings.json",
                ".vscode/extensions.json",
                "README.md",
            ]
        );
    }

    #[test]
    fn template_paths_become_nested_paths() {
        let files = plan().files().expect("templates render");
        let config = files
            .iter()
            .find(|f| f.path.ends_with("config.toml"))
            .expect("a cargo config");
        assert_eq!(config.path, PathBuf::from(".cargo").join("config.toml"));
        assert_eq!(config.path.components().count(), 2);
    }

    #[test]
    fn sequence_imports_underscored_lib_name() {
        assert_eq!(plan().lib_name(), "ref_sequences");
        assert!(
            file(&plan(), "src/bin/startup.rs").contains("use ref_sequences::*;"),
            "the sequence must import the crate under Cargo's library name"
        );
    }

    #[test]
    fn manifest_names_crate_and_sequence() {
        let manifest = manifest(&plan());
        assert_eq!(manifest["package"]["name"].as_str(), Some("ref-sequences"));
        let bins = manifest["bin"]
            .as_array_of_tables()
            .expect("[[bin]] should be an array of tables");
        assert_eq!(bins.len(), 1);
        assert_eq!(
            bins.get(0).expect("one bin")["name"].as_str(),
            Some("startup")
        );
    }

    #[test]
    fn manifest_disables_bin_harnesses() {
        let manifest = manifest(&plan());
        let bin = manifest["bin"]
            .as_array_of_tables()
            .expect("an array of tables")
            .get(0)
            .expect("one bin")
            .clone();
        assert_eq!(bin["test"].as_bool(), Some(false));
        assert_eq!(bin["bench"].as_bool(), Some(false));
        assert_eq!(manifest["lib"]["test"].as_bool(), Some(false));
        assert_eq!(manifest["lib"]["bench"].as_bool(), Some(false));
    }

    #[test]
    fn sequences_gated_behind_wasm_feature() {
        let manifest = manifest(&plan());
        assert!(
            manifest["features"][WASM_FEATURE].as_array().is_some(),
            "the `{WASM_FEATURE}` feature must be declared"
        );
        // Not a default, or a host `cargo build` would try to link the bins.
        assert!(
            manifest["features"].get("default").is_none(),
            "`{WASM_FEATURE}` must not be a default feature"
        );

        let required: Vec<String> = manifest["bin"]
            .as_array_of_tables()
            .expect("an array of tables")
            .get(0)
            .expect("one bin")["required-features"]
            .as_array()
            .expect("required-features should be an array")
            .iter()
            .map(|feature| feature.as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(required, vec![WASM_FEATURE.to_string()]);
    }

    #[test]
    fn manifest_depends_on_test_dsl_dev_only() {
        let manifest = manifest(&plan());
        assert_eq!(
            manifest["dev-dependencies"]["fprime_test"].as_str(),
            Some("1.2.3")
        );
        // `fprime_test` is a `std` host crate; it must not reach the sequences.
        assert!(manifest["dependencies"].get("fprime_test").is_none());
    }

    #[test]
    fn manifest_sets_size_tuned_release_profile() {
        let profile = manifest(&plan())["profile"]["release"].clone();
        assert_eq!(profile["panic"].as_str(), Some("abort"));
        assert_eq!(profile["opt-level"].as_str(), Some("s"));
        assert_eq!(profile["lto"].as_bool(), Some(true));
        assert_eq!(profile["strip"].as_bool(), Some(true));
    }

    #[test]
    fn manifest_pins_crate_versions() {
        let manifest = manifest(&plan());
        assert_eq!(
            manifest["dependencies"]["fprime_core"].as_str(),
            Some("1.2.3")
        );
        assert_eq!(
            manifest["build-dependencies"]["fprime_build"].as_str(),
            Some("1.2.3")
        );
    }

    /// `[build] target` would break `cargo test`; `fprime-wasm build` passes `--target` instead.
    #[test]
    fn cargo_config_omits_default_target() {
        let config: toml_edit::DocumentMut = file(&plan(), ".cargo/config.toml")
            .parse()
            .expect("the generated cargo config should be valid TOML");
        assert!(
            config
                .get("build")
                .and_then(|build| build.get("target"))
                .is_none(),
            "`[build] target` would break `cargo test` in a scaffolded project:\n{config}"
        );
    }

    #[test]
    fn cargo_config_sets_wasm_link_args() {
        let config: toml_edit::DocumentMut = file(&plan(), ".cargo/config.toml")
            .parse()
            .expect("the generated cargo config should be valid TOML");

        let flags: Vec<String> = config["target"][TARGET]["rustflags"]
            .as_array()
            .expect("rustflags should be an array")
            .iter()
            .map(|flag| flag.as_str().unwrap_or_default().to_string())
            .collect();
        assert!(
            flags.contains(&"-Clink-arg=--page-size=1".to_string()),
            "{flags:?}"
        );
        assert!(
            flags.contains(&"-Clink-arg=-zstack-size=512".to_string()),
            "{flags:?}"
        );
        assert!(flags.contains(&"-Ctarget-cpu=mvp".to_string()), "{flags:?}");

        // `build-std` needs nightly, so enabling it would break `cargo build` on
        // stable in a scaffolded project.
        assert!(config.get("unstable").is_none(), "build-std must stay off");
    }

    #[test]
    fn cargo_config_wires_release_linker() {
        let config: toml_edit::DocumentMut = file(&plan(), ".cargo/config.toml")
            .parse()
            .expect("valid TOML");
        assert_eq!(
            config["target"][TARGET]["linker"].as_str(),
            Some(template::WASM_LINK.path)
        );
    }

    #[test]
    fn release_linker_pins_feature_set() {
        let script = file(&plan(), ".cargo/wasm-link");
        for flag in [
            "--mvp-features",
            "--enable-mutable-globals",
            "--enable-custom-page-sizes",
        ] {
            assert!(script.contains(flag), "{flag} missing from:\n{script}");
        }
        assert!(script.contains("rust-lld"), "{script}");
        assert!(script.contains("*/release/*"), "{script}");
    }

    #[test]
    fn missing_optimiser_does_not_fail_build() {
        let script = file(&plan(), ".cargo/wasm-link");
        let (_, after) = script
            .split_once("command -v wasm-opt")
            .expect("the script should probe for wasm-opt");
        let branch = after
            .split_once("fi\n")
            .expect("the probe should be an if block")
            .0;
        assert!(
            branch.contains("exit 0"),
            "a missing wasm-opt must warn and continue:\n{branch}"
        );
    }

    #[test]
    fn only_release_linker_is_executable() {
        for file in plan().files().expect("templates render") {
            let expected = file.path == PathBuf::from(".cargo").join("wasm-link");
            assert_eq!(
                file.executable,
                expected,
                "{} should{} be executable",
                file.path.display(),
                if expected { "" } else { " not" }
            );
        }
    }

    #[test]
    fn custom_stack_size_reaches_link_args() {
        let mut plan = plan();
        plan.stack_size = 2048;
        assert!(
            file(&plan, ".cargo/config.toml").contains("-zstack-size=2048"),
            "the configured stack size must be what the linker gets"
        );
    }

    #[test]
    fn build_rs_points_at_dictionary() {
        assert!(
            file(&plan(), "build.rs")
                .contains("fprime_build::generate(\"dictionary/RefTopologyDictionary.json\")"),
            "build.rs must generate from the dictionary the plan names"
        );
    }

    #[test]
    fn build_rs_uses_forward_slashes() {
        let mut plan = plan();
        plan.dictionary = PathBuf::from("dictionary").join("Ref.json");
        let build_rs = file(&plan, "build.rs");
        assert!(build_rs.contains("dictionary/Ref.json"), "{build_rs}");
        assert!(!build_rs.contains('\\'), "{build_rs}");
    }

    #[test]
    fn library_includes_dictionary_and_runtime() {
        let lib = file(&plan(), "src/lib.rs");
        assert!(lib.contains("#![no_std]"), "{lib}");
        assert!(
            lib.contains("include!(concat!(env!(\"OUT_DIR\"), \"/dictionary.rs\"))"),
            "{lib}"
        );
        assert!(lib.contains("pub use Defs::*;"), "{lib}");
        assert!(lib.contains("pub use fprime_core::*;"), "{lib}");
    }

    #[test]
    fn library_gates_test_descriptors_from_flight() {
        let lib = file(&plan(), "src/lib.rs");
        let (before, after) = lib
            .split_once("include!(concat!(env!(\"OUT_DIR\"), \"/descriptors.rs\"))")
            .expect("the library should include the generated descriptors");
        assert!(
            before
                .trim_end()
                .ends_with("#[cfg(not(target_family = \"wasm\"))]"),
            "the descriptors include must be gated to a host target:\n{lib}"
        );
        // And the flight API's own include must NOT be gated, or a sequence has no API at all.
        assert!(
            !after.contains("dictionary.rs"),
            "the flight API include must come first and stay ungated:\n{lib}"
        );
        let (flight, _) = lib
            .split_once("include!(concat!(env!(\"OUT_DIR\"), \"/dictionary.rs\"))")
            .expect("the library should include the generated dictionary");
        assert!(
            !flight.contains("#[cfg("),
            "the flight API include must not be behind a cfg:\n{lib}"
        );
    }

    #[test]
    fn sequence_is_no_std_no_main_with_entry_point() {
        let source = sequence("sequences", "safing").expect("renders");
        assert!(source.contains("#![no_std]"), "{source}");
        assert!(source.contains("#![no_main]"), "{source}");
        assert!(source.contains("#[fprime_main]"), "{source}");
        assert!(source.contains("pub fn main() {"), "{source}");
        assert!(source.contains("use sequences::*;"), "{source}");
        assert!(source.contains("safing"), "{source}");
        // Braces are literal; a stray `{{` would mean an unescaped template.
        assert!(!source.contains("{{"), "{source}");
    }

    #[test]
    fn init_and_add_render_same_sequence() {
        let mut plan = plan();
        plan.name = "sequences".into();
        plan.sequence = "safing".into();
        assert_eq!(
            file(&plan, "src/bin/safing.rs"),
            sequence("sequences", "safing").expect("renders")
        );
        assert_eq!(
            file(&plan, "tests/safing.rs"),
            test("sequences", "safing").expect("renders")
        );
    }

    #[test]
    fn sequence_test_names_sequence_and_imports_crate() {
        let source = test("sequences", "safing").expect("renders");
        assert!(
            source.contains("#[fprime_test(sequence = \"safing\")]"),
            "{source}"
        );
        assert!(source.contains("use fprime_test::*;"), "{source}");
        assert!(source.contains("use sequences::*;"), "{source}");
        assert!(!source.contains("#![no_std]"), "{source}");
        assert!(!source.contains("{{"), "{source}");
    }
}

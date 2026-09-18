//! What `init` generates, as values rather than files.
//!
//! Separate from [`super::write`] so the generated contents can be asserted
//! without touching a filesystem.

use super::manifest::{DependencySpec, repoint_at_checkout};
use super::template;
use crate::project::TARGET;
use anyhow::{Context, Result};
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
    /// Library name Cargo exposes, which is what a sequence binary imports: Cargo
    /// replaces `-` with `_`, so `ref-sequences` is imported as `ref_sequences`.
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
            // `build.rs` embeds this in a Rust string literal, where a Windows
            // separator would be an invalid escape.
            (
                "dictionary_slashes",
                self.dictionary.display().to_string().replace('\\', "/"),
            ),
            // The template declares both dependencies as plain versions; a
            // checkout is applied afterwards by `repoint_at_checkout`, which is
            // why this is a version even in the path case.
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
    ///
    /// Fails only on a broken template — reported rather than panicked on, since a
    /// release binary asserting on its own templates helps nobody.
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
                    // Split so the separator is the platform's, not the `/` the
                    // templates are written with.
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
    // Supplying only the two keys this template uses cannot under-supply silently:
    // `render` rejects any placeholder it was not given a value for.
    let values = vec![
        ("lib_name", lib_name.to_string()),
        ("sequence", name.to_string()),
    ];
    template::render(template::SEQUENCE.body, &values).context("in the sequence template")
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{file, manifest, plan};
    use super::*;

    #[test]
    fn generates_the_expected_set_of_files() {
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
                ".gitignore",
                "README.md",
            ]
        );
    }

    /// A template path is written with `/` and has to become a real nested path,
    /// not one file with a slash in its name.
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

    /// Cargo underscores the library name, so a sequence in `ref-sequences` has to
    /// `use ref_sequences::*`.
    #[test]
    fn a_sequence_imports_the_underscored_library_name() {
        assert_eq!(plan().lib_name(), "ref_sequences");
        assert!(
            file(&plan(), "src/bin/startup.rs").contains("use ref_sequences::*;"),
            "the sequence must import the crate under Cargo's library name"
        );
    }

    #[test]
    fn the_manifest_names_the_crate_and_its_first_sequence() {
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

    /// The two settings without which `cargo build` fails on a `#![no_main]` bin.
    #[test]
    fn the_manifest_switches_off_the_bin_harnesses() {
        let manifest = manifest(&plan());
        let bin = manifest["bin"]
            .as_array_of_tables()
            .expect("an array of tables")
            .get(0)
            .expect("one bin")
            .clone();
        assert_eq!(bin["test"].as_bool(), Some(false));
        assert_eq!(bin["bench"].as_bool(), Some(false));
        // The library target too: there is nothing to test in a no_std Wasm crate.
        assert_eq!(manifest["lib"]["test"].as_bool(), Some(false));
        assert_eq!(manifest["lib"]["bench"].as_bool(), Some(false));
    }

    /// Unwinding machinery would dwarf the sequence itself.
    #[test]
    fn the_manifest_sets_a_size_tuned_release_profile() {
        let profile = manifest(&plan())["profile"]["release"].clone();
        assert_eq!(profile["panic"].as_str(), Some("abort"));
        assert_eq!(profile["opt-level"].as_str(), Some("s"));
        assert_eq!(profile["lto"].as_bool(), Some(true));
        assert_eq!(profile["strip"].as_bool(), Some(true));
    }

    #[test]
    fn the_manifest_pins_both_crates_to_the_requested_version() {
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

    /// `--page-size=1` is what makes guest memory sizeable in bytes; without it
    /// every sequence demands a 64 KiB page.
    #[test]
    fn the_cargo_config_sets_the_target_and_the_link_arguments() {
        let config: toml_edit::DocumentMut = file(&plan(), ".cargo/config.toml")
            .parse()
            .expect("the generated cargo config should be valid TOML");
        assert_eq!(config["build"]["target"].as_str(), Some(TARGET));

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

    /// The config has to name the script the same plan writes; asserted against the
    /// template rather than a literal, so renaming one and not the other fails here
    /// instead of at someone's first release build.
    #[test]
    fn the_cargo_config_wires_the_release_linker() {
        let config: toml_edit::DocumentMut = file(&plan(), ".cargo/config.toml")
            .parse()
            .expect("valid TOML");
        assert_eq!(
            config["target"][TARGET]["linker"].as_str(),
            Some(template::WASM_LINK.path)
        );
    }

    /// `wasm-opt` introduces post-MVP instructions while optimising even when the
    /// compiler emitted none, and the module then fails when the sequencer loads it.
    /// Pinning the optimiser to what `spacewasm` implements is the whole point of the
    /// script.
    #[test]
    fn the_release_linker_pins_the_interpreters_feature_set() {
        let script = file(&plan(), ".cargo/wasm-link");
        for flag in [
            "--mvp-features",
            "--enable-mutable-globals",
            "--enable-custom-page-sizes",
        ] {
            assert!(script.contains(flag), "{flag} missing from:\n{script}");
        }
        assert!(script.contains("rust-lld"), "{script}");
        // Optimising a debug build would make it unreadable in a debugger.
        assert!(script.contains("*/release/*"), "{script}");
    }

    /// A missing optimiser must not fail the build: the module is linked and valid,
    /// just larger, and `verify` measures what is actually on disk.
    #[test]
    fn a_missing_optimiser_does_not_fail_the_build() {
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
    fn only_the_release_linker_is_executable() {
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
    fn a_custom_stack_size_reaches_the_link_arguments() {
        let mut plan = plan();
        plan.stack_size = 2048;
        assert!(
            file(&plan, ".cargo/config.toml").contains("-zstack-size=2048"),
            "the configured stack size must be what the linker gets"
        );
    }

    #[test]
    fn build_rs_points_at_the_dictionary() {
        assert!(
            file(&plan(), "build.rs")
                .contains("fprime_build::generate(\"dictionary/RefTopologyDictionary.json\")"),
            "build.rs must generate from the dictionary the plan names"
        );
    }

    /// A Windows-style path would land in the generated Rust as an invalid escape.
    #[test]
    fn build_rs_uses_forward_slashes() {
        let mut plan = plan();
        plan.dictionary = PathBuf::from("dictionary").join("Ref.json");
        let build_rs = file(&plan, "build.rs");
        assert!(build_rs.contains("dictionary/Ref.json"), "{build_rs}");
        assert!(!build_rs.contains('\\'), "{build_rs}");
    }

    #[test]
    fn the_library_includes_the_generated_dictionary_and_the_runtime() {
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
    fn a_sequence_is_no_std_no_main_with_the_entry_point_attribute() {
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

    /// `init` and `add` must render the same file, or the starter sequence would
    /// drift from every later one.
    #[test]
    fn init_and_add_render_the_same_sequence() {
        let mut plan = plan();
        plan.name = "sequences".into();
        plan.sequence = "safing".into();
        assert_eq!(
            file(&plan, "src/bin/safing.rs"),
            sequence("sequences", "safing").expect("renders")
        );
    }
}

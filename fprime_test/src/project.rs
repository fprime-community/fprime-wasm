//! Finding a sequence crate and editing its manifest.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, value};

/// The Wasm target sequences build for.
pub const TARGET: &str = "wasm32v1-none";

/// Env var naming which profile a sequence test loads its module from.
pub const TEST_PROFILE_ENV: &str = "FPRIME_TEST_PROFILE";

/// Feature gate required on every sequence `[[bin]]`.
pub const WASM_FEATURE: &str = "wasm";

/// A Cargo crate holding sequences.
pub struct Project {
    /// Directory holding `Cargo.toml`.
    root: PathBuf,
    manifest: DocumentMut,
}

impl Project {
    /// Loads the crate containing `start`, walking up until a `Cargo.toml` with a
    /// `[package]` table turns up.
    pub fn find(start: &Path) -> Result<Project> {
        let start = std::fs::canonicalize(start)
            .with_context(|| format!("could not resolve {}", start.display()))?;

        for directory in start.ancestors() {
            let path = directory.join("Cargo.toml");
            if !path.is_file() {
                continue;
            }
            let manifest = read_manifest(&path)?;
            if manifest.get("package").is_some() {
                return Ok(Project {
                    root: directory.to_path_buf(),
                    manifest,
                });
            }
        }

        bail!(
            "no Cargo crate found in {} or any parent directory. Run `fprime-wasm init` to create \
             one",
            start.display()
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("Cargo.toml")
    }

    pub fn name(&self) -> Result<&str> {
        self.manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(Item::as_str)
            .context("the crate's Cargo.toml has no `package.name`")
    }

    /// Names of the sequences declared in `[[bin]]` entries.
    pub fn sequences(&self) -> Vec<String> {
        self.manifest
            .get("bin")
            .and_then(Item::as_array_of_tables)
            .map(|bins| {
                bins.iter()
                    .filter_map(|bin| bin.get("name").and_then(Item::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Path a sequence's source lives at.
    pub fn sequence_source(&self, name: &str) -> PathBuf {
        self.root.join("src").join("bin").join(format!("{name}.rs"))
    }

    /// Path a sequence's tests live at.
    pub fn test_source(&self, name: &str) -> PathBuf {
        self.test_source_dir().join(format!("{name}.rs"))
    }

    /// Directory holding the sequence tests.
    pub fn test_source_dir(&self) -> PathBuf {
        self.root.join("tests")
    }

    /// Declares a `[[bin]]` for `name`; call [`Project::save`] to persist. Returns `false`
    /// if `name` was already declared.
    pub fn declare_sequence(&mut self, name: &str) -> bool {
        if self.sequences().iter().any(|existing| existing == name) {
            return false;
        }

        let mut bin = Table::new();
        bin.set_implicit(false);
        bin["name"] = value(name);
        let mut features = toml_edit::Array::new();
        features.push(WASM_FEATURE);
        bin["required-features"] = value(features);
        bin["test"] = value(false);
        bin["bench"] = value(false);

        let bins = self
            .manifest
            .entry("bin")
            .or_insert_with(|| Item::ArrayOfTables(Default::default()));
        if let Item::ArrayOfTables(bins) = bins {
            bins.push(bin);
            true
        } else {
            false
        }
    }

    /// True when `[[bin]]` exists but isn't an array of tables.
    pub fn has_unusable_bin_key(&self) -> bool {
        self.manifest
            .get("bin")
            .is_some_and(|bin| bin.as_array_of_tables().is_none())
    }

    pub fn save(&self) -> Result<()> {
        let path = self.manifest_path();
        std::fs::write(&path, self.manifest.to_string())
            .with_context(|| format!("could not write {}", path.display()))
    }

    /// Directory the `.wasm` artefacts land in for `profile`.
    pub fn artifact_dir(&self, profile: &str) -> Option<PathBuf> {
        if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
            let candidate = PathBuf::from(target).join(TARGET).join(profile);
            return candidate.is_dir().then_some(candidate);
        }
        self.root.ancestors().find_map(|directory| {
            let candidate = directory.join("target").join(TARGET).join(profile);
            candidate.is_dir().then_some(candidate)
        })
    }
}

/// The `.wasm` modules in `directory`, sorted. Used both to check every sequence a build
/// produced and to name the ones a test could have meant.
pub fn modules(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut modules: Vec<PathBuf> = std::fs::read_dir(directory)
        .with_context(|| format!("could not list {}", directory.display()))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect();
    modules.sort();
    Ok(modules)
}

fn read_manifest(path: &Path) -> Result<DocumentMut> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;
    text.parse::<DocumentMut>()
        .with_context(|| format!("{} is not valid TOML", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(manifest: &str) -> Project {
        Project {
            root: PathBuf::from("/tmp/sequences"),
            manifest: manifest.parse().expect("valid TOML"),
        }
    }

    const BASE: &str = r#"
[package]
name = "sequences"
version = "0.1.0"
edition = "2024"

[dependencies]
fprime_core = "1.0.0"

[[bin]]
name = "startup"
test = false
bench = false
"#;

    #[test]
    fn reads_package_name_and_sequences() {
        let project = project(BASE);
        assert_eq!(project.name().expect("a name"), "sequences");
        assert_eq!(project.sequences(), vec!["startup"]);
    }

    #[test]
    fn missing_name_is_error() {
        let project = project("[package]\nversion = \"0.1.0\"\n");
        assert!(project.name().is_err());
    }

    #[test]
    fn declares_sequence_with_harnesses_off() {
        let mut project = project(BASE);
        assert!(project.declare_sequence("safing"));
        assert_eq!(project.sequences(), vec!["startup", "safing"]);

        let manifest = project.manifest.to_string();
        let declaration = manifest
            .rsplit("[[bin]]")
            .next()
            .expect("a trailing declaration");
        assert!(declaration.contains("name = \"safing\""), "{declaration}");
        assert!(declaration.contains("test = false"), "{declaration}");
        assert!(declaration.contains("bench = false"), "{declaration}");
        assert!(
            declaration.contains(&format!("required-features = [\"{WASM_FEATURE}\"]")),
            "{declaration}"
        );
    }

    #[test]
    fn locates_test_source_under_tests_dir() {
        let project = project(BASE);
        assert_eq!(
            project.test_source("safing"),
            PathBuf::from("/tmp/sequences/tests/safing.rs")
        );
        assert_eq!(
            project.test_source_dir(),
            PathBuf::from("/tmp/sequences/tests")
        );
    }

    #[test]
    fn declaring_existing_sequence_is_noop() {
        let mut project = project(BASE);
        let before = project.manifest.to_string();
        assert!(!project.declare_sequence("startup"));
        assert_eq!(project.manifest.to_string(), before);
    }

    #[test]
    fn preserves_rest_of_manifest() {
        let mut project = project(
            r#"
# Sequences for the Ref deployment.
[package]
name    = "sequences"          # deliberate spacing
version = "0.1.0"
edition = "2024"

[dependencies]
fprime_core = "1.0.0"
"#,
        );
        project.declare_sequence("safing");
        let manifest = project.manifest.to_string();
        assert!(manifest.contains("# Sequences for the Ref deployment."));
        assert!(manifest.contains("name    = \"sequences\"          # deliberate spacing"));
    }

    #[test]
    fn no_bins_reports_empty() {
        let project = project("[package]\nname = \"s\"\n");
        assert!(project.sequences().is_empty());
        assert!(!project.has_unusable_bin_key());
    }

    #[test]
    fn detects_non_table_bin_key() {
        let project = project("bin = \"nonsense\"\n\n[package]\nname = \"s\"\n");
        assert!(project.has_unusable_bin_key());
    }

    #[test]
    fn declaring_into_unusable_bin_key_fails_safely() {
        let mut project = project("bin = \"nonsense\"\n\n[package]\nname = \"s\"\n");
        assert!(!project.declare_sequence("safing"));
        assert!(project.manifest.to_string().contains("bin = \"nonsense\""));
    }

    #[test]
    fn locates_sequence_source_under_src_bin() {
        let project = project(BASE);
        assert_eq!(
            project.sequence_source("safing"),
            PathBuf::from("/tmp/sequences/src/bin/safing.rs")
        );
    }
}

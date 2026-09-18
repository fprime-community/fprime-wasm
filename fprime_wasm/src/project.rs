//! Finding a sequence crate and editing its manifest.
//!
//! A sequence project is an ordinary Cargo crate with one twist: every sequence
//! is its own `[[bin]]`, so it links to its own `.wasm`, and each has to be
//! declared with `test = false` and `bench = false`. Cargo would otherwise try to
//! build a test harness for a `#![no_main]` binary, which does not compile.
//! Keeping those declarations right by hand is exactly the sort of thing `add`
//! exists to avoid.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, value};

/// The Wasm target the sequences build for. `wasm32v1-none` is the `no_std`
/// WebAssembly 1.0 target, which is what `spacewasm` implements.
pub const TARGET: &str = "wasm32v1-none";

/// A Cargo crate holding sequences.
pub struct Project {
    /// Directory holding `Cargo.toml`.
    root: PathBuf,
    manifest: DocumentMut,
}

impl Project {
    /// Load the crate containing `start`, walking up until a `Cargo.toml` with a
    /// `[package]` table turns up.
    ///
    /// Ancestors are searched so that the command works from a subdirectory, the
    /// way `cargo` itself does. A `Cargo.toml` with only `[workspace]` is skipped:
    /// that is the workspace root, not the crate a sequence belongs to.
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

    /// Declare a `[[bin]]` for `name`.
    ///
    /// Does not write the manifest; call [`Project::save`]. Returns `false` if the
    /// sequence was already declared, so `add` can repair a half-finished state —
    /// a declared bin with no source, or the reverse — without duplicating the
    /// entry.
    pub fn declare_sequence(&mut self, name: &str) -> bool {
        if self.sequences().iter().any(|existing| existing == name) {
            return false;
        }

        let mut bin = Table::new();
        bin.set_implicit(false);
        bin["name"] = value(name);
        // A `#![no_main]` binary has no test or bench harness to build, and Cargo
        // fails rather than skipping them.
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
            // `bin` is present but is not an array of tables, which is not a
            // manifest we can extend. Reported by `add` as a conflict.
            false
        }
    }

    /// True when `[[bin]]` exists but is not an array of tables, so
    /// [`Project::declare_sequence`] cannot add to it.
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
    ///
    /// `CARGO_TARGET_DIR` wins if set, as it does for Cargo. Otherwise the first
    /// existing `target/<triple>/<profile>` at or above the crate root is used,
    /// which is what makes this work in a workspace, where the artefacts are in
    /// the workspace's `target` and not the crate's.
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
    fn reads_the_package_name_and_sequences() {
        let project = project(BASE);
        assert_eq!(project.name().expect("a name"), "sequences");
        assert_eq!(project.sequences(), vec!["startup"]);
    }

    #[test]
    fn a_manifest_without_a_name_is_an_error() {
        let project = project("[package]\nversion = \"0.1.0\"\n");
        assert!(project.name().is_err());
    }

    #[test]
    fn declares_a_new_sequence_with_the_harnesses_off() {
        let mut project = project(BASE);
        assert!(project.declare_sequence("safing"));
        assert_eq!(project.sequences(), vec!["startup", "safing"]);

        let manifest = project.manifest.to_string();
        let declaration = manifest
            .rsplit("[[bin]]")
            .next()
            .expect("a trailing declaration");
        assert!(declaration.contains("name = \"safing\""), "{declaration}");
        // Without these, Cargo tries to build a test harness for a `#![no_main]`
        // binary and the build fails.
        assert!(declaration.contains("test = false"), "{declaration}");
        assert!(declaration.contains("bench = false"), "{declaration}");
    }

    /// `add` is expected to be re-runnable: declaring the same sequence twice
    /// would produce a manifest Cargo rejects.
    #[test]
    fn declaring_an_existing_sequence_changes_nothing() {
        let mut project = project(BASE);
        let before = project.manifest.to_string();
        assert!(!project.declare_sequence("startup"));
        assert_eq!(project.manifest.to_string(), before);
    }

    /// The rest of the manifest — comments, ordering, formatting — has to survive
    /// an edit, or `add` rewrites the author's file out from under them.
    #[test]
    fn preserves_the_rest_of_the_manifest() {
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
    fn a_manifest_with_no_bins_reports_none() {
        let project = project("[package]\nname = \"s\"\n");
        assert!(project.sequences().is_empty());
        assert!(!project.has_unusable_bin_key());
    }

    /// `bin = "..."` is not something we can push a table onto. Better to say so
    /// than to silently do nothing.
    #[test]
    fn detects_a_bin_key_that_is_not_an_array_of_tables() {
        let project = project("bin = \"nonsense\"\n\n[package]\nname = \"s\"\n");
        assert!(project.has_unusable_bin_key());
    }

    #[test]
    fn declaring_into_an_unusable_bin_key_fails_rather_than_corrupting() {
        let mut project = project("bin = \"nonsense\"\n\n[package]\nname = \"s\"\n");
        assert!(!project.declare_sequence("safing"));
        assert!(project.manifest.to_string().contains("bin = \"nonsense\""));
    }

    #[test]
    fn locates_a_sequence_source_under_src_bin() {
        let project = project(BASE);
        assert_eq!(
            project.sequence_source("safing"),
            PathBuf::from("/tmp/sequences/src/bin/safing.rs")
        );
    }
}

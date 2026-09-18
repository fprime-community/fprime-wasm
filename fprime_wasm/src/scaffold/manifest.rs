//! How a generated manifest depends on the `fprime_*` crates.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// What a generated manifest should depend on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySpec {
    /// A published version, the normal case.
    Version(String),
    /// A path into a checkout, for working on the crates themselves.
    Path(PathBuf),
}

impl DependencySpec {
    /// How this reads in a manifest, for messages and for the scaffolded README.
    pub fn describe(&self) -> String {
        match self {
            DependencySpec::Version(version) => version.clone(),
            DependencySpec::Path(path) => format!("path {}", path.display()),
        }
    }
}

/// Point the generated manifest's two dependencies at a checkout of this
/// repository.
///
/// Edits the parsed document instead of substituting a `{ path = "..." }` fragment
/// into the template: a path is arbitrary text and TOML reinterprets parts of it —
/// `\U` in `C:\Users\...` is a unicode escape, so the manifest would not parse at
/// all. `toml_edit` escapes the string properly and the template stays plain TOML.
pub(super) fn repoint_at_checkout(manifest: &str, checkout: &Path) -> Result<String> {
    let mut document: toml_edit::DocumentMut = manifest
        .parse()
        .context("the generated Cargo.toml is not valid TOML")?;

    // The spec names one crate; its sibling sits beside it in the same checkout.
    let root = checkout.parent().unwrap_or(Path::new(".."));
    for (table, crate_name) in [
        ("dependencies", "fprime_core"),
        ("build-dependencies", "fprime_build"),
    ] {
        let mut dependency = toml_edit::InlineTable::new();
        dependency.insert("path", root.join(crate_name).display().to_string().into());
        document[table][crate_name] = toml_edit::value(dependency);
    }

    Ok(document.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{manifest, plan};
    use super::*;

    #[test]
    fn describes_dependency_specs() {
        assert_eq!(DependencySpec::Version("1.2.3".into()).describe(), "1.2.3");
        assert_eq!(
            DependencySpec::Path(PathBuf::from("/checkout")).describe(),
            "path /checkout"
        );
    }

    /// A path spec names one crate; the sibling must be resolved beside it, or
    /// `fprime_build` would point at `fprime_core`'s directory.
    #[test]
    fn a_path_dependency_resolves_each_crate_beside_the_other() {
        let mut plan = plan();
        plan.dependency = DependencySpec::Path(PathBuf::from("../../fprime_core"));
        let manifest = manifest(&plan);
        assert_eq!(
            manifest["dependencies"]["fprime_core"]["path"].as_str(),
            Some("../../fprime_core")
        );
        assert_eq!(
            manifest["build-dependencies"]["fprime_build"]["path"].as_str(),
            Some("../../fprime_build")
        );
        // No version is left beside the path: a `0.0.0` there would be a
        // requirement no published crate satisfies.
        assert!(
            manifest["dependencies"]["fprime_core"]
                .get("version")
                .is_none()
        );
    }

    /// The bug that moved this off string substitution: a spliced
    /// `{ path = "..." }` made a backslash invalid TOML, so `--local` on Windows
    /// produced a manifest Cargo could not parse.
    ///
    /// The characters matter, not the platform — so the path contains a backslash
    /// while still having a parent on this host, and the assertion is that the
    /// value round-trips rather than that it equals a platform-specific spelling.
    #[test]
    fn a_backslash_in_a_path_dependency_is_escaped_not_spliced() {
        for awkward in [r"od\d", "od\"d", r"od\Users"] {
            let checkout = PathBuf::from("/tmp").join(awkward).join("fprime_core");
            let mut plan = plan();
            plan.dependency = DependencySpec::Path(checkout.clone());

            // Reaching this at all is the regression: `manifest` parses the result.
            let manifest = manifest(&plan);
            assert_eq!(
                manifest["dependencies"]["fprime_core"]["path"].as_str(),
                Some(checkout.display().to_string().as_str()),
                "{awkward}: the path must survive a parse round-trip unchanged"
            );
            assert_eq!(
                manifest["build-dependencies"]["fprime_build"]["path"].as_str(),
                Some(
                    PathBuf::from("/tmp")
                        .join(awkward)
                        .join("fprime_build")
                        .display()
                        .to_string()
                        .as_str()
                ),
                "{awkward}: the sibling path must be escaped too"
            );
        }
    }
}

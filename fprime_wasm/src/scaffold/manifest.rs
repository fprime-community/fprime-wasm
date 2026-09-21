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

/// Points the generated manifest's two dependencies at a checkout of this repository.
///
/// Edits the parsed document rather than splicing a `{ path = "..." }` string, so a
/// backslash in the path is escaped correctly.
pub(super) fn repoint_at_checkout(manifest: &str, checkout: &Path) -> Result<String> {
    let mut document: toml_edit::DocumentMut = manifest
        .parse()
        .context("the generated Cargo.toml is not valid TOML")?;

    // The spec names one crate; its siblings sit beside it in the same checkout.
    let root = checkout.parent().unwrap_or(Path::new(".."));
    for (table, crate_name) in [
        ("dependencies", "fprime_core"),
        ("build-dependencies", "fprime_build"),
        ("dev-dependencies", "fprime_test"),
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

    /// The sibling crate resolves beside the named one, not in its directory.
    #[test]
    fn path_dependency_resolves_sibling_crates() {
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

    /// Regression: a spliced `{ path = "..." }` made a backslash invalid TOML.
    #[test]
    fn backslash_in_path_dependency_is_escaped() {
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

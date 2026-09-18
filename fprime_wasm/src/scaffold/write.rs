//! Putting generated files on disk, and adding a sequence to a project.

use super::name::valid_name;
use super::plan::{File, sequence};
use crate::project::Project;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// What happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    Created,
    /// Left as it was. `init` and `add` never overwrite: a sequence someone has
    /// edited is worth more than a template.
    Skipped,
}

/// Create `file` under `root` unless it is already there.
pub fn write(root: &Path, file: &File) -> Result<Written> {
    let path = root.join(&file.path);
    if path.exists() {
        return Ok(Written::Skipped);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    std::fs::write(&path, &file.contents)
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(Written::Created)
}

/// Outcome of adding a sequence, so the caller can describe what it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Added {
    pub source: Written,
    pub declared: bool,
}

/// Add a sequence to `project`: write its source and declare its `[[bin]]`.
///
/// Both halves are idempotent, so re-running after an interruption completes the
/// missing half instead of duplicating the other.
pub fn add_sequence(project: &mut Project, name: &str) -> Result<Added> {
    valid_name(name)?;
    if project.has_unusable_bin_key() {
        bail!(
            "{} has a `bin` key that is not a list of `[[bin]]` tables; fix it before adding a \
             sequence",
            project.manifest_path().display()
        );
    }

    let lib_name = project.name()?.replace('-', "_");
    let source = write(
        project.root(),
        &File {
            path: PathBuf::from("src").join("bin").join(format!("{name}.rs")),
            contents: sequence(&lib_name, name)?,
        },
    )?;

    let declared = project.declare_sequence(name);
    if declared {
        project.save()?;
    }

    Ok(Added { source, declared })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_file_and_then_leaves_it_alone() {
        let root = std::env::temp_dir().join(format!(
            "fprime-wasm-scaffold-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let file = File {
            path: PathBuf::from("src").join("bin").join("startup.rs"),
            contents: "original".into(),
        };

        assert_eq!(write(&root, &file).expect("first write"), Written::Created);
        let edited = File {
            contents: "edited by hand".into(),
            ..file.clone()
        };
        std::fs::write(root.join(&file.path), &edited.contents).expect("edit");

        // The whole point: a second run must not clobber someone's work.
        assert_eq!(write(&root, &file).expect("second write"), Written::Skipped);
        assert_eq!(
            std::fs::read_to_string(root.join(&file.path)).expect("read back"),
            "edited by hand"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}

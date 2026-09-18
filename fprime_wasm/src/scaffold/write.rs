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
    if file.executable {
        set_executable(&path)?;
    }
    Ok(Written::Created)
}

/// Cargo runs `.cargo/wasm-link` directly, so a file left at 0644 fails the build
/// with a permission error rather than anything that names the cause.
#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .with_context(|| format!("could not read the mode of {}", path.display()))?
        .permissions();
    // Added to whatever the umask allowed, rather than set outright, so a restrictive
    // umask is respected for the read and write bits.
    permissions.set_mode(permissions.mode() | 0o111);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("could not make {} executable", path.display()))
}

/// No such bit off Unix, where a `#!/bin/sh` linker cannot run anyway.
#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
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
            executable: false,
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

    fn temporary(label: u32) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fprime-wasm-scaffold-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    /// Cargo runs this one rather than reading it, so the bit is the difference
    /// between a working release build and a permission error.
    #[cfg(unix)]
    #[test]
    fn an_executable_file_is_written_with_the_bit_set() {
        use std::os::unix::fs::PermissionsExt;
        let root = temporary(line!());

        for (name, executable) in [("wasm-link", true), ("config.toml", false)] {
            let file = File {
                path: PathBuf::from(".cargo").join(name),
                contents: "#!/bin/sh\n".into(),
                executable,
            };
            assert_eq!(write(&root, &file).expect("write"), Written::Created);
            let mode = std::fs::metadata(root.join(&file.path))
                .expect("written")
                .permissions()
                .mode();
            assert_eq!(
                mode & 0o111 != 0,
                executable,
                "{name} has mode {mode:o}, executable = {executable}"
            );
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn writes_a_file_and_then_leaves_it_alone() {
        let root = temporary(line!());
        let file = File {
            path: PathBuf::from("src").join("bin").join("startup.rs"),
            contents: "original".into(),
            executable: false,
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

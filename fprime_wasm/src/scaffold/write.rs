//! Putting generated files on disk, and adding a sequence to a project.

use super::name::valid_name;
use super::plan::{File, sequence};
use anyhow::{Context, Result, bail};
use fprime_test::project::Project;
use std::path::{Path, PathBuf};

/// What happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    Created,
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

/// Cargo runs `.cargo/wasm-link` directly, so it needs the executable bit.
#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .with_context(|| format!("could not read the mode of {}", path.display()))?
        .permissions();
    // Added to the existing mode, so the umask's read/write bits are respected.
    permissions.set_mode(permissions.mode() | 0o111);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("could not make {} executable", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Outcome of adding a sequence, so the caller can describe what it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Added {
    pub source: Written,
    pub test: Written,
    pub declared: bool,
}

/// Adds a sequence to `project`: writes its source and tests, and declares its `[[bin]]`.
/// Idempotent — a re-run completes whichever parts are missing.
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

    // Written even for a pre-existing sequence missing its test file.
    let test = write(
        project.root(),
        &File {
            path: PathBuf::from("tests").join(format!("{name}.rs")),
            contents: super::plan::test(&lib_name, name)?,
            executable: false,
        },
    )?;

    let declared = project.declare_sequence(name);
    if declared {
        project.save()?;
    }

    Ok(Added {
        source,
        test,
        declared,
    })
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

    #[cfg(unix)]
    #[test]
    fn executable_file_is_written_with_bit_set() {
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
    fn add_sequence_writes_source_tests_and_bin_entry() {
        let root = temporary(line!());
        std::fs::create_dir_all(&root).expect("temp dir");
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"sequences\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("manifest");

        let mut project = Project::find(&root).expect("a project");
        let added = add_sequence(&mut project, "safing").expect("added");
        assert_eq!(added.source, Written::Created);
        assert_eq!(added.test, Written::Created);
        assert!(added.declared);

        let test_path = root.join("tests").join("safing.rs");
        assert!(test_path.is_file(), "the tests should be beside the source");
        std::fs::write(&test_path, "// what I decided it should do").expect("edit");

        // Re-run: nothing new, and the edit survives.
        let mut project = Project::find(&root).expect("a project");
        let again = add_sequence(&mut project, "safing").expect("added again");
        assert_eq!(again.source, Written::Skipped);
        assert_eq!(again.test, Written::Skipped);
        assert!(!again.declared, "the [[bin]] must not be declared twice");
        assert_eq!(
            std::fs::read_to_string(&test_path).expect("read back"),
            "// what I decided it should do"
        );
        assert_eq!(project.sequences(), vec!["safing"]);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_leaves_existing_file_alone() {
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

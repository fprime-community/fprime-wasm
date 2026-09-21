//! Finding the compiled module a test is about.
//!
//! A test names its sequence (`#[fprime_test(sequence = "safing")]`) rather than a path, so
//! this resolves that to `target/<triple>/<profile>/safing.wasm`, via
//! [`Project::artifact_dir`].

use crate::project::{Project, TARGET, TEST_PROFILE_ENV};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// The profile to load from: what `fprime-wasm test` asked for, else release — what flies,
/// and what a plain `cargo test` should test.
pub fn profile() -> String {
    std::env::var(TEST_PROFILE_ENV).unwrap_or_else(|_| "release".to_string())
}

/// The `.wasm` for `sequence`, as bytes. `manifest_dir` is the test's own
/// `CARGO_MANIFEST_DIR`, the sequence crate.
pub fn module(manifest_dir: &Path, sequence: &str) -> Result<(PathBuf, Vec<u8>)> {
    let path = module_path(manifest_dir, sequence)?;
    let bytes = std::fs::read(&path)
        .with_context(|| format!("could not read the sequence module {}", path.display()))?;
    Ok((path, bytes))
}

fn module_path(manifest_dir: &Path, sequence: &str) -> Result<PathBuf> {
    let project = Project::find(manifest_dir)?;
    let profile = profile();

    let Some(directory) = project.artifact_dir(&profile) else {
        bail!(
            "no {profile} build found for {TARGET}. Run `fprime-wasm test`, which builds the \
             sequences first, or `fprime-wasm build{}`",
            debug_flag(&profile)
        );
    };

    let path = directory.join(format!("{sequence}.wasm"));
    if !path.is_file() {
        let available = available(&directory);
        bail!(
            "no sequence named `{sequence}` in {}{}. Check the `sequence` in #[fprime_test], or \
             run `fprime-wasm build{}`",
            directory.display(),
            match available.is_empty() {
                true => String::new(),
                false => format!(" (found: {})", available.join(", ")),
            },
            debug_flag(&profile)
        );
    }
    Ok(path)
}

/// `--debug` only when that is the profile being looked for.
fn debug_flag(profile: &str) -> &'static str {
    match profile {
        "debug" => " --debug",
        _ => "",
    }
}

/// Sequence names with a module in `directory`, sorted, for a "did you mean" list.
fn available(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = crate::project::modules(directory)
        .unwrap_or_default()
        .iter()
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_release() {
        // Not asserted through the environment: these tests run in one process and
        // mutating a shared variable would race the rest of the suite.
        assert_eq!(
            std::env::var(TEST_PROFILE_ENV).unwrap_or_else(|_| "release".to_string()),
            profile()
        );
        assert_eq!(debug_flag("release"), "");
        assert_eq!(debug_flag("debug"), " --debug");
    }

    #[test]
    fn available_names_are_file_stems() {
        let directory = std::env::temp_dir().join(format!(
            "fprime-test-available-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&directory).expect("temp dir");
        std::fs::write(directory.join("safing.wasm"), b"\0asm").expect("write");
        std::fs::write(directory.join("startup.wasm"), b"\0asm").expect("write");
        // Not a module, so not a sequence anyone could name.
        std::fs::write(directory.join("notes.txt"), b"x").expect("write");

        assert_eq!(available(&directory), vec!["safing", "startup"]);
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn empty_directory_offers_no_names() {
        assert!(available(Path::new("/nonexistent-fprime-test-dir")).is_empty());
    }
}

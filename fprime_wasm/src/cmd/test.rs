//! `fprime-wasm test`: build the sequences, then run a host `cargo test`.

use super::build;
use super::cargo::{Cargo, Profile};
use super::cli::Test;
use anyhow::{Context, Result};
use fprime_test::project::{Project, TEST_PROFILE_ENV};

/// Returns whether the tests passed.
pub fn run(args: &Test) -> Result<bool> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    let project = Project::find(&cwd)?;
    let profile = Profile::from_debug_flag(args.debug);

    // Build first; a stale module would test old code.
    if !args.no_build && !build::build(&project, profile, &[])? {
        return Ok(false);
    }

    if !project.test_source_dir().is_dir() {
        println!(
            "No tests yet. `fprime-wasm add <name>` writes `{}` alongside the sequence.",
            project
                .test_source("<name>")
                .strip_prefix(project.root())
                .unwrap_or(&project.test_source("<name>"))
                .display()
        );
        return Ok(true);
    }

    Cargo::new(project.root(), "test")
        // Env var, not a cargo flag: only the loaded module follows `--debug`.
        .env(TEST_PROFILE_ENV, profile.directory())
        .args(&args.filters)
        .args(&args.cargo)
        .run()
}

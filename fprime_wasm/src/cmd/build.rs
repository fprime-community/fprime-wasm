//! `fprime-wasm build`: compile the sequences to Wasm.

use super::cargo::{Cargo, Profile};
use super::cli::Build;
use anyhow::{Context, Result};
use fprime_test::project::{Project, TARGET, WASM_FEATURE};

/// Returns whether cargo succeeded.
pub fn run(args: &Build) -> Result<bool> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    let project = Project::find(&cwd)?;
    build(&project, Profile::from_debug_flag(args.debug), &args.cargo)
}

/// Compiles the sequences; shared with `test`, which needs them built first.
pub fn build(project: &Project, profile: Profile, extra: &[String]) -> Result<bool> {
    Cargo::new(project.root(), "build")
        .profile(profile)
        .arg("--target")
        .arg(TARGET)
        .arg("--features")
        .arg(WASM_FEATURE)
        .args(extra)
        .run()
}

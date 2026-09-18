//! `fprime-wasm add`: add a sequence to the crate in the current directory.

use super::cli::Add;
use anyhow::{Context, Result};
use fprime_wasm::project::Project;
use fprime_wasm::scaffold::{self, Written};
use std::path::Path;

pub fn run(args: &Add) -> Result<()> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    let mut project = Project::find(&cwd)?;
    let outcome = scaffold::add_sequence(&mut project, &args.name)?;

    let source = project
        .sequence_source(&args.name)
        .strip_prefix(project.root())
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| project.sequence_source(&args.name));

    match outcome.source {
        Written::Created => println!("  create {}", source.display()),
        Written::Skipped => println!("    keep {} (already present)", source.display()),
    }
    if outcome.declared {
        println!("  update Cargo.toml [[bin]] {}", args.name);
    } else {
        println!("    keep Cargo.toml ({} already declared)", args.name);
    }

    if outcome.source == Written::Skipped && !outcome.declared {
        println!();
        println!("Nothing to do: `{}` is already a sequence.", args.name);
    } else {
        println!();
        println!("Edit {} and run `cargo build --release`.", source.display());
    }
    Ok(())
}

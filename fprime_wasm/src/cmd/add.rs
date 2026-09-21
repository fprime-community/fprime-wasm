//! `fprime-wasm add`: add a sequence to the crate in the current directory.

use super::cli::Add;
use anyhow::{Context, Result};
use fprime_test::project::Project;
use fprime_wasm::scaffold::{self, Written};
use std::path::Path;

pub fn run(args: &Add) -> Result<()> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    let mut project = Project::find(&cwd)?;
    let outcome = scaffold::add_sequence(&mut project, &args.name)?;

    let source = relative(&project, project.sequence_source(&args.name));
    let test = relative(&project, project.test_source(&args.name));

    for (path, written) in [(&source, outcome.source), (&test, outcome.test)] {
        match written {
            Written::Created => println!("  create {}", path.display()),
            Written::Skipped => println!("    keep {} (already present)", path.display()),
        }
    }
    if outcome.declared {
        println!("  update Cargo.toml [[bin]] {}", args.name);
    } else {
        println!("    keep Cargo.toml ({} already declared)", args.name);
    }

    let untouched =
        outcome.source == Written::Skipped && outcome.test == Written::Skipped && !outcome.declared;
    println!();
    if untouched {
        println!("Nothing to do: `{}` is already a sequence.", args.name);
    } else {
        println!("Next:");
        println!("  edit {}", source.display());
        println!("  edit {} to say what it should do", test.display());
        println!("  fprime-wasm test");
    }
    Ok(())
}

/// A path as the author would recognise it, relative to the crate they are in.
fn relative(project: &Project, path: std::path::PathBuf) -> std::path::PathBuf {
    path.strip_prefix(project.root())
        .map(Path::to_path_buf)
        .unwrap_or(path)
}

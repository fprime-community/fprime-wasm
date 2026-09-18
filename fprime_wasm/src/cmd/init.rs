//! `fprime-wasm init`: fill a directory with a sequence project.

use super::cli::Init;
use super::dictionary;
use anyhow::{Context, Result, bail};
use fprime_wasm::project::TARGET;
use fprime_wasm::scaffold::{self, DependencySpec, Plan, Written};
use std::path::Path;

/// Version of the `fprime_*` crates a scaffolded project depends on.
///
/// The whole workspace releases in lockstep from one tag, so the tool and the crates
/// it scaffolds against always share a version. `0.0.0` is the unreleased placeholder
/// that `release.yml` substitutes, so seeing it means this is a build from a checkout
/// rather than an installed release.
const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// This crate's own directory at compile time, used to fall back to path dependencies
/// when running an unreleased build out of a checkout.
const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub fn run(args: &Init) -> Result<()> {
    let directory = &args.directory;
    std::fs::create_dir_all(directory)
        .with_context(|| format!("could not create {}", directory.display()))?;
    let root = std::fs::canonicalize(directory)
        .with_context(|| format!("could not resolve {}", directory.display()))?;

    if root.join("Cargo.toml").exists() {
        bail!(
            "{} is already a Cargo crate. Use `fprime-wasm add <name>` to add a sequence to it",
            root.display()
        );
    }

    let name = match &args.name {
        Some(name) => {
            scaffold::valid_name(name)?;
            name.clone()
        }
        None => scaffold::crate_name_from_directory(&root).with_context(|| {
            format!(
                "could not derive a crate name from {}; pass --name",
                root.display()
            )
        })?,
    };
    scaffold::valid_name(&args.sequence)?;

    let plan = Plan {
        name,
        sequence: args.sequence.clone(),
        dictionary: dictionary::resolve(&root, args.dictionary.as_deref())?,
        dependency: dependency_spec(args.crate_version.as_deref(), args.local.as_deref())?,
        stack_size: args.stack_size,
    };

    // Rendered up front so a broken template fails before any file is written, rather
    // than leaving a half-created project behind.
    let files = plan.files()?;

    println!("Creating {} in {}", plan.name, root.display());
    for file in &files {
        let outcome = scaffold::write(&root, file)?;
        println!(
            "  {} {}",
            match outcome {
                Written::Created => "create",
                Written::Skipped => "  keep",
            },
            file.path.display()
        );
    }

    println!();
    println!("Next:");
    println!("  rustup target add {TARGET}");
    println!("  cd {} && cargo build --release", directory.display());
    println!("  fprime-wasm verify");
    Ok(())
}

/// How a scaffolded project should depend on the `fprime_*` crates.
///
/// An unreleased build cannot honestly emit its own version — `0.0.0` is not on
/// crates.io — so it falls back to path dependencies into the checkout it was built
/// from. That keeps `cargo run -p fprime-wasm -- init` working while developing, and an
/// installed release still pins a real version.
fn dependency_spec(version: Option<&str>, local: Option<&Path>) -> Result<DependencySpec> {
    if let Some(local) = local {
        let root = std::fs::canonicalize(local)
            .with_context(|| format!("could not resolve {}", local.display()))?;
        if !root.join("fprime_core").join("Cargo.toml").is_file() {
            bail!(
                "{} does not look like an fprime-wasm checkout: no fprime_core/Cargo.toml",
                root.display()
            );
        }
        return Ok(DependencySpec::Path(root.join("fprime_core")));
    }
    if let Some(version) = version {
        return Ok(DependencySpec::Version(version.to_string()));
    }
    if CRATE_VERSION == "0.0.0" {
        // `fprime_wasm/` sits beside `fprime_core/` in the workspace.
        let workspace = Path::new(MANIFEST_DIR)
            .parent()
            .context("this crate has no parent directory")?;
        eprintln!(
            "warning: this is an unreleased build of fprime-wasm, so there is no published \
             version to depend on."
        );
        eprintln!(
            "         Scaffolding against the checkout at {}. Pass --crate-version to pin a \
             release instead.",
            workspace.display()
        );
        return Ok(DependencySpec::Path(workspace.join("fprime_core")));
    }
    Ok(DependencySpec::Version(CRATE_VERSION.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--local` must point at a real checkout, or the scaffolded manifest would
    /// reference a path that does not resolve.
    #[test]
    fn a_local_dependency_must_be_a_checkout() {
        let empty =
            std::env::temp_dir().join(format!("fprime-wasm-not-a-checkout-{}", std::process::id()));
        std::fs::create_dir_all(&empty).expect("temp dir");
        assert!(dependency_spec(None, Some(&empty)).is_err());
        std::fs::remove_dir_all(&empty).ok();

        // The workspace this crate lives in is one.
        let workspace = Path::new(MANIFEST_DIR).parent().expect("a parent");
        let spec = dependency_spec(None, Some(workspace)).expect("the workspace is a checkout");
        assert!(matches!(spec, DependencySpec::Path(_)));
    }

    #[test]
    fn an_explicit_version_is_used_verbatim() {
        assert_eq!(
            dependency_spec(Some("1.2.3"), None).expect("valid"),
            DependencySpec::Version("1.2.3".into())
        );
    }
}

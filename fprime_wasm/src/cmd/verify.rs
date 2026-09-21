//! `fprime-wasm verify`: load compiled sequences on the on-board interpreter and size
//! them. Nothing is executed — `fprime-wasm test` is what runs a sequence.

use super::build;
use super::cargo::Profile;
use super::cli::Verify;
use anyhow::{Context, Result, bail};
use fprime_test::config;
use fprime_test::interpreter::Limits;
use fprime_test::project::{self, Project, TARGET};
use fprime_wasm::verify;
use std::path::PathBuf;

/// Returns whether every module passed.
pub fn run(args: &Verify) -> Result<bool> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    // A named path doesn't need a project; only discovery does.
    let project = Project::find(&cwd);

    let modules = if args.modules.is_empty() {
        let found = project.as_ref().map_err(|err| {
            anyhow::anyhow!("{err:#}. Alternatively, name the .wasm modules to check")
        })?;
        // Build first; a stale module would verify old code.
        if !args.no_build && !build::build(found, Profile::from_debug_flag(args.debug), &[])? {
            return Ok(false);
        }
        discover(args, found)?
    } else {
        args.modules.clone()
    };

    let loaded = config::load(args.limits.as_deref(), project.as_ref().ok())?;
    let limits = loaded.limits;

    let mut checked: Vec<verify::Verified> = Vec::new();
    let mut failures: Vec<(PathBuf, Vec<String>)> = Vec::new();
    // Modules with no measurements at all; reported apart from `checked` in the JSON.
    let mut unreadable: Vec<(PathBuf, String)> = Vec::new();

    for module in &modules {
        // Continue past failures; each module gets a fresh allocator.
        match verify::verify(module, &limits) {
            Ok(one) => {
                if !one.passed() {
                    failures.push((one.path.clone(), one.failures()));
                }
                checked.push(one);
            }
            Err(err) => {
                // Modules that will not load can't appear in the report; named here
                // instead, to stderr so `--json` stays clean.
                eprintln!("{}: could not be loaded", module.display());
                for line in format!("{err:#}").lines() {
                    eprintln!("  {line}");
                }
                let message = format!("{err:#}");
                unreadable.push((module.clone(), message.clone()));
                failures.push((module.clone(), vec![message]));
            }
        }
    }

    if args.json {
        let run = verify::report::Run::new(&checked, &unreadable, &limits);
        // Pretty-printed so CI diffs are readable.
        println!(
            "{}",
            serde_json::to_string_pretty(&run).context("could not serialise the report")?
        );
        return Ok(failures.is_empty());
    }

    report(args, &checked, &modules, &failures, &limits, &loaded.source);
    Ok(failures.is_empty())
}

/// Every `.wasm` in the project's build output, when none were named.
fn discover(args: &Verify, project: &Project) -> Result<Vec<PathBuf>> {
    let profile = if args.debug { "debug" } else { "release" };
    // `cargo build` alone wouldn't produce Wasm; the target isn't pinned in .cargo/config.toml.
    let build = format!(
        "fprime-wasm build{}",
        if args.debug { " --debug" } else { "" }
    );
    let directory = project.artifact_dir(profile).with_context(|| {
        format!(
            "no {profile} build found for {TARGET}. Run `{build}` first, or name the .wasm \
             modules to check"
        )
    })?;
    let discovered = project::modules(&directory)?;
    if discovered.is_empty() {
        bail!(
            "no .wasm modules in {}. Run `{build}` first",
            directory.display()
        );
    }
    Ok(discovered)
}

/// The table report: limits once, a row per module, then whatever needs saying.
fn report(
    args: &Verify,
    checked: &[verify::Verified],
    modules: &[PathBuf],
    failures: &[(PathBuf, Vec<String>)],
    limits: &Limits,
    source: &config::Source,
) {
    println!("{}", verify::limits_line(limits, &source.label()));
    println!();
    print!("{}", verify::summary(checked));

    // Detail is opt-in; a full crate's worth would otherwise be huge.
    if args.verbose {
        for one in checked {
            println!();
            print!("{}", verify::detail(one));
        }
    }

    println!();
    if failures.is_empty() {
        match checked.len() {
            1 => println!("The module fits."),
            count => println!("All {count} modules fit."),
        }
    } else {
        match modules.len() {
            1 => println!("Problems:"),
            total => println!("{} of {total} modules have problems:", failures.len()),
        }
        for (path, reasons) in failures {
            println!(
                "  {}",
                path.file_stem()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
            );
            for reason in reasons {
                println!("    {reason}");
            }
        }
        if !args.verbose {
            println!();
            println!("Re-run with --verbose for the full figures.");
        }
    }

    // Said once, and only when something was actually sized: the two figures this report
    // cannot carry, and where they come from.
    if !checked.is_empty() {
        println!();
        println!(
            "Loading only: stackSize and the guest stack are measured by running a sequence, \
             which `fprime-wasm test` does."
        );
    }
}

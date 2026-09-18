//! `fprime-wasm verify`: run compiled sequences on the on-board interpreter and size
//! them.

use super::cli::Verify;
use super::{dictionary, hex};
use anyhow::{Context, Result, bail};
use fprime_wasm::harness::Responses;
use fprime_wasm::project::{Project, TARGET};
use fprime_wasm::verify;
use std::path::PathBuf;

/// Returns whether every module passed.
pub fn run(args: &Verify) -> Result<bool> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    // A path may be given from anywhere, so a project is only needed when something has
    // to be discovered.
    let project = Project::find(&cwd);

    let modules = if args.modules.is_empty() {
        discover(args, project.as_ref())?
    } else {
        args.modules.clone()
    };

    let dictionary = dictionary::load(args.dictionary.as_deref(), project.as_ref().ok())?;
    let limits = args.limits.to_limits();
    let responses = Responses {
        args: hex::bytes(args.args.as_deref().unwrap_or("")).context("--args must be hex bytes")?,
        telemetry: hex::keyed(&args.telemetry, "--tlm")?,
        parameters: hex::keyed(&args.parameters, "--prm")?,
        serial: hex::keyed(&args.serial, "--serial")?,
        event_message_max: args.limits.event_message_max,
        serial_ports: args.limits.serial_ports,
    };

    let mut checked: Vec<verify::Verified> = Vec::new();
    let mut failures: Vec<(PathBuf, Vec<String>)> = Vec::new();
    // Inputs with no measurements at all, which the JSON reports apart from the measured
    // modules.
    let mut unreadable: Vec<(PathBuf, String)> = Vec::new();

    for module in &modules {
        // Each module is measured on a freshly configured allocator, so a failure on one
        // does not distort the next; keep going and report all of them.
        match verify::verify(module, &limits, responses.clone()) {
            Ok(one) => {
                if !one.passed() {
                    failures.push((one.path.clone(), one.failures()));
                }
                checked.push(one);
            }
            Err(err) => {
                // A module that could not be decoded has no measurements, so it cannot
                // appear in the report; it is named here instead. To stderr so `--json`
                // keeps a clean stdout.
                eprintln!("{}: could not be checked", module.display());
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
        let run = verify::report::Run::new(
            &checked,
            &unreadable,
            &limits,
            dictionary.as_ref(),
            args.trace,
        );
        // Pretty-printed: these get committed as CI artefacts and diffed, and a
        // single-line document diffs as one changed line.
        println!(
            "{}",
            serde_json::to_string_pretty(&run).context("could not serialise the report")?
        );
        return Ok(failures.is_empty());
    }

    report(args, &checked, &modules, &failures, &limits, &dictionary);
    Ok(failures.is_empty())
}

/// Every `.wasm` in the project's build output, when none were named.
fn discover(args: &Verify, project: Result<&Project, &anyhow::Error>) -> Result<Vec<PathBuf>> {
    let project = project.map_err(|err| {
        anyhow::anyhow!("{err:#}. Alternatively, name the .wasm modules to check")
    })?;
    let profile = if args.debug { "debug" } else { "release" };
    let directory = project.artifact_dir(profile).with_context(|| {
        format!(
            "no {profile} build found for {TARGET}. Run `cargo build{}` first, or name the .wasm \
             modules to check",
            if args.debug { "" } else { " --release" }
        )
    })?;
    let discovered = verify::discover(&directory)?;
    if discovered.is_empty() {
        bail!(
            "no .wasm modules in {}. Run `cargo build{}` first",
            directory.display(),
            if args.debug { "" } else { " --release" }
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
    limits: &fprime_wasm::harness::Limits,
    dictionary: &Option<fprime_dictionary::Dictionary>,
) {
    // The limits are the same for every module, so they are stated once here and left
    // out of the rows.
    println!("{}", verify::limits_line(limits));
    println!();
    print!("{}", verify::summary(checked));

    // Detail is opt-in: for a run over a crate's worth of sequences it is the difference
    // between a screenful and a thousand lines.
    if args.verbose || args.trace {
        for one in checked {
            println!();
            print!("{}", verify::detail(one, dictionary.as_ref(), args.trace));
        }
    }

    // Warnings always; the notes about how `verify` fed the sequence only when asked
    // for, since they fire on nearly every run.
    let warnings = verify::issues(checked, args.verbose);
    if !warnings.is_empty() {
        println!();
        print!("{warnings}");
    }
    let info = verify::info_count(checked);
    if info > 0 && !args.verbose {
        println!();
        // Phrased as a label so it reads correctly for any count: a leading number would
        // need both the noun and the verb to agree.
        println!(
            "Unset channels and parameters read as zero ({info}); --verbose lists them, \
             --tlm/--prm set a value."
        );
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
}

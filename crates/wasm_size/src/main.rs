//! Measure the size of every wasm module this workspace builds, and compare two
//! measurements.
//!
//! ```text
//! wasm_size measure [--target-dir DIR] [--json OUT]
//! wasm_size compare BASE.json HEAD.json
//! ```
//!
//! `measure` prints a markdown table and, with `--json`, writes the machine
//! readable form that `compare` consumes. CI measures the pull request and the
//! merge base and compares the two.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod report;
mod wasm;

use report::{Binary, Report};

/// Crates whose bins are measured. `bench` is the shape-by-shape benchmark;
/// `example` is tracked too because it is the closest thing to a real sequence.
const SUBJECTS: [&str; 2] = ["bench", "example"];

const TARGET: &str = "wasm32v1-none";

const USAGE: &str = "\
usage:
    wasm_size measure [--target-dir DIR] [--json OUT]
    wasm_size compare [--base-label NAME] BASE.json HEAD.json
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let result = match args.first().map(String::as_str) {
        Some("measure") => measure(&args[1..]),
        Some("compare") => compare(&args[1..]),
        Some("--help" | "-h") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => Err(USAGE.to_string()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("wasm_size: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Read `--flag value` pairs, rejecting anything unrecognised rather than
/// ignoring it: a mistyped flag in CI should fail loudly.
fn options(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut args = args.iter();

    while let Some(flag) = args.next() {
        if !allowed.contains(&flag.as_str()) {
            return Err(format!("unexpected argument `{flag}`\n\n{USAGE}"));
        }

        let value = args
            .next()
            .ok_or_else(|| format!("`{flag}` needs a value\n\n{USAGE}"))?;

        out.insert(flag.clone(), value.clone());
    }

    Ok(out)
}

fn measure(args: &[String]) -> Result<(), String> {
    let options = options(args, &["--target-dir", "--json"])?;

    let root = workspace_root();

    // Absolute, because the cargo child runs with its working directory set to
    // the workspace root while the section reader runs in ours: a relative
    // `--target-dir` would otherwise name two different directories.
    let target_dir = match options.get("--target-dir") {
        None => root.join("target"),
        Some(dir) => {
            std::path::absolute(dir).map_err(|err| format!("could not resolve `{dir}`: {err}"))?
        }
    };

    build(&root, &target_dir)?;

    let report = collect(&target_dir.join(TARGET).join("release"))?;

    if let Some(path) = options.get("--json") {
        let json = serde_json::to_string_pretty(&report)
            .map_err(|err| format!("could not serialize the report: {err}"))?;

        std::fs::write(path, json + "\n")
            .map_err(|err| format!("could not write `{path}`: {err}"))?;
    }

    print!("{}", report.markdown());

    Ok(())
}

fn compare(args: &[String]) -> Result<(), String> {
    let mut label = "BASE".to_string();
    let mut files = vec![];
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base-label" => {
                label = args
                    .next()
                    .ok_or_else(|| format!("`--base-label` needs a value\n\n{USAGE}"))?
                    .clone();
            }
            // Same reasoning as `options`: an unrecognised flag is a mistake to
            // report, not a filename to try to open.
            flag if flag.starts_with("--") => {
                return Err(format!("unexpected argument `{flag}`\n\n{USAGE}"));
            }
            file => files.push(file.to_string()),
        }
    }

    let [base, head] = files.as_slice() else {
        return Err(format!("compare needs two files\n\n{USAGE}"));
    };

    print!(
        "{}",
        report::markdown(&report::compare(&read(base)?, &read(head)?), &label)
    );

    Ok(())
}

fn read(path: &str) -> Result<Report, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|err| format!("could not read `{path}`: {err}"))?;

    serde_json::from_str(&contents).map_err(|err| format!("could not parse `{path}`: {err}"))
}

/// The workspace root: the nearest ancestor whose manifest declares a
/// `[workspace]`.
///
/// Searched rather than counted as a fixed number of parents, because getting it
/// wrong is not a loud failure. The default `--target-dir` hangs off this path,
/// so a root one level too deep writes `crates/target`, which the `crates/*`
/// member glob then claims as a workspace member with no manifest, and every
/// later cargo invocation in the checkout fails.
fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

    manifest
        .ancestors()
        .find(|dir| {
            std::fs::read_to_string(dir.join("Cargo.toml"))
                .is_ok_and(|manifest| manifest.contains("[workspace]"))
        })
        .unwrap_or_else(|| {
            panic!(
                "no `[workspace]` manifest at or above `{}`",
                manifest.display()
            )
        })
        .to_path_buf()
}

fn build(root: &Path, target_dir: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let mut command = Command::new(cargo);
    command
        .current_dir(root)
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg(TARGET)
        .arg("--target-dir")
        .arg(target_dir);

    for subject in SUBJECTS {
        command.arg("-p").arg(subject);
    }

    // `RUSTFLAGS` *replaces* `[target.wasm32v1-none] rustflags` from the root
    // `.cargo/config.toml` rather than adding to it. Inheriting one from the
    // parent process would silently drop `-zstack-size=512` and `--page-size=1`,
    // which inflates linear memory roughly a thousandfold and makes every
    // measurement incomparable. Cargo also sets the encoded form when it invokes
    // a build script or a `cargo run` target, so both have to go.
    command.env_remove("RUSTFLAGS");
    command.env_remove("CARGO_ENCODED_RUSTFLAGS");

    let status = command
        .status()
        .map_err(|err| format!("could not run cargo: {err}"))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!("cargo build failed with {status}")),
    }
}

fn collect(release_dir: &Path) -> Result<Report, String> {
    let entries = std::fs::read_dir(release_dir)
        .map_err(|err| format!("could not read `{}`: {err}", release_dir.display()))?;

    let mut binaries = vec![];

    for entry in entries {
        let path = entry
            .map_err(|err| format!("could not read `{}`: {err}", release_dir.display()))?
            .path();

        if path.extension().is_none_or(|extension| extension != "wasm") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("`{}` has no usable name", path.display()))?;

        let bytes = std::fs::read(&path)
            .map_err(|err| format!("could not read `{}`: {err}", path.display()))?;

        let sections = wasm::sections(&bytes)
            .ok_or_else(|| format!("`{}` is not a wasm module", path.display()))?;

        binaries.push(Binary::from_sections(name, sections));
    }

    if binaries.is_empty() {
        return Err(format!("no wasm modules in `{}`", release_dir.display()));
    }

    Ok(Report::new(binaries))
}

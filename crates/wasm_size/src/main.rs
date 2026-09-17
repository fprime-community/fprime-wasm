//! Measure the size of every wasm module this workspace builds, and compare two
//! measurements.
//!
//! ```text
//! wasm_size measure [--target-dir DIR] [--json OUT]
//! wasm_size compare [--base-label NAME] [--source-base URL] BASE.json HEAD.json
//! ```
//!
//! `measure` builds the wasm crates and prints a markdown table; with `--json` it
//! also writes the machine readable form that `compare` consumes. CI measures the
//! pull request and the merge base and compares the two.
//!
//! What it measures is already optimised: the release link step pipes each module
//! through `wasm-opt`, by way of the linker wrapper named in `.cargo/config.toml`.
//! So a release build leaves the flyable artefact in the target directory and this
//! only has to read it.

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
    wasm_size compare [--base-label NAME] [--source-base URL] BASE.json HEAD.json
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

    let report = collect(&target_dir.join(TARGET).join("release"), &root)?;

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
    let mut source_base = None;
    let mut files = vec![];
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("`{flag}` needs a value\n\n{USAGE}"))
        };

        match arg.as_str() {
            "--base-label" => label = value("--base-label")?,
            // Trailing slash trimmed so the caller can pass either form: the
            // paths joined onto this are relative and carry their own separator.
            "--source-base" => {
                source_base = Some(value("--source-base")?.trim_end_matches('/').to_string());
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

    let style = report::Style {
        base_label: &label,
        source_base: source_base.as_deref(),
    };

    let (base, head) = (read(base)?, read(head)?);

    print!(
        "{}",
        report::markdown(&report::compare(&base, &head), &style)
    );

    // Every delta in the table is then mostly the optimiser's doing, which is not
    // what a reader of a size report on a pull request assumes it is reading. Only
    // when both sides actually reported: an older measurement says nothing either
    // way, and treating that as "not optimised" would cry wolf on every comparison
    // against a revision predating the field.
    if let (Some(base), Some(head)) = (base.optimized, head.optimized)
        && base != head
    {
        let (with, without) = match head {
            true => ("HEAD", label.as_str()),
            false => (label.as_str(), "HEAD"),
        };

        println!(
            "\n> [!NOTE]\n\
             > `{with}` was measured after `wasm-opt` and `{without}` was not, so these\n\
             > deltas are dominated by the optimiser rather than by the change itself."
        );
    }

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

/// Where a measured bin's source lives, relative to the workspace root, for the
/// table to link to.
///
/// Found by looking rather than recorded by cargo, which does not report it.
/// `None` when nothing matches — a bin laid out some other way costs its link and
/// nothing else, so this must not be an error.
fn source(root: &Path, name: &str) -> Option<String> {
    SUBJECTS.iter().find_map(|subject| {
        let path = format!("crates/{subject}/src/bin/{name}.rs");

        root.join(&path).is_file().then_some(path)
    })
}

/// Every wasm module in `dir`, sorted so a run does not depend on directory order.
fn wasm_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|err| format!("could not read `{}`: {err}", dir.display()))?;

    let mut paths = vec![];

    for entry in entries {
        let path = entry
            .map_err(|err| format!("could not read `{}`: {err}", dir.display()))?
            .path();

        if path
            .extension()
            .is_some_and(|extension| extension == "wasm")
        {
            paths.push(path);
        }
    }

    paths.sort();

    Ok(paths)
}

fn collect(release_dir: &Path, root: &Path) -> Result<Report, String> {
    let mut binaries = vec![];

    for path in wasm_files(release_dir)? {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("`{}` has no usable name", path.display()))?;

        let bytes = std::fs::read(&path)
            .map_err(|err| format!("could not read `{}`: {err}", path.display()))?;

        let sections = wasm::sections(&bytes)
            .ok_or_else(|| format!("`{}` is not a wasm module", path.display()))?;

        binaries.push(Binary::from_sections(name, sections, source(root, name)));
    }

    if binaries.is_empty() {
        return Err(format!("no wasm modules in `{}`", release_dir.display()));
    }

    Ok(Report::new(binaries, true))
}

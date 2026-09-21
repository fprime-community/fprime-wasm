//! Finding, validating and installing a deployment's JSON dictionary.
//!
//! `init` copies one into the project, since `build.rs` generates the whole command,
//! channel and parameter surface from it. A sequence test reads the same copy, to name the
//! commands and channels a failure mentions.

use anyhow::{Context, Result, bail};
use fprime_wasm::scaffold::DICTIONARY_DIR;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

/// Settle on a dictionary for a new project, copying it in if it came from outside.
///
/// Returns the path relative to the crate root, which is what `build.rs` needs.
pub fn resolve(root: &Path, supplied: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = supplied {
        return install(root, path);
    }

    // A project being re-initialised, or one whose dictionary was dropped in by hand,
    // already has one; do not ask again.
    if let Some(existing) = existing(root) {
        println!(
            "Using the dictionary already present: {}",
            existing.display()
        );
        return Ok(existing);
    }

    if !std::io::stdin().is_terminal() {
        bail!(
            "no dictionary given and none found under {DICTIONARY_DIR}/. Pass --dictionary <path> \
             to the deployment's JSON dictionary (a deployment build writes it as \
             <Deployment>TopologyDictionary.json)"
        );
    }

    println!("A sequence project needs its deployment's JSON dictionary: it is what the commands,");
    println!(
        "telemetry channels and parameters are generated from. A deployment build writes it as"
    );
    println!("<Deployment>TopologyDictionary.json.");
    loop {
        print!("Path to the dictionary: ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line)? == 0 {
            bail!("no dictionary given");
        }
        let answer = line.trim();
        if answer.is_empty() {
            continue;
        }
        match install(root, Path::new(answer)) {
            Ok(path) => return Ok(path),
            Err(err) => eprintln!("  {err:#}"),
        }
    }
}

/// Validate a dictionary and place it under the crate root.
fn install(root: &Path, source: &Path) -> Result<PathBuf> {
    // Parsed before anything is copied: a dictionary that does not deserialise would
    // fail in `build.rs` instead, where the error is harder to place.
    let dictionary =
        fprime_dictionary::try_parse(source).map_err(|err| anyhow::anyhow!("{err}"))?;
    println!(
        "Dictionary for {} ({} commands, {} channels, {} parameters)",
        dictionary.metadata.deployment_name,
        dictionary.commands.len(),
        dictionary.telemetry_channels.len(),
        dictionary.parameters.len()
    );

    let file_name = source
        .file_name()
        .context("the dictionary path has no file name")?;
    let relative = PathBuf::from(DICTIONARY_DIR).join(file_name);
    let destination = root.join(&relative);

    // Already in place, e.g. re-running `init` with the same argument.
    if let Ok(canonical) = std::fs::canonicalize(source)
        && std::fs::canonicalize(&destination).is_ok_and(|existing| existing == canonical)
    {
        return Ok(relative);
    }

    std::fs::create_dir_all(destination.parent().expect("relative to a directory"))?;
    std::fs::copy(source, &destination)
        .with_context(|| format!("could not copy the dictionary to {}", destination.display()))?;
    Ok(relative)
}

/// A `.json` already sitting in the project's dictionary directory.
fn existing(root: &Path) -> Option<PathBuf> {
    let directory = root.join(DICTIONARY_DIR);
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&directory)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    candidates.sort();
    let first = candidates.into_iter().next()?;
    Some(PathBuf::from(DICTIONARY_DIR).join(first.file_name()?))
}

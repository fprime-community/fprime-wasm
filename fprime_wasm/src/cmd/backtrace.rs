//! `fprime-wasm backtrace`: PCs or positions in a sequence, back to the lines they came from.
//!
//! The symbol file and its resolution are `spacewasm_tools`'; this finds a sequence's
//! `.sym`, and replays the load against `fprime_v1` and `sequencer.toml`, which is what
//! decided the IR layout a PC is an address in.

use super::cli::Backtrace;
use anyhow::{Context, Result, anyhow, bail};
use fprime_test::project::{Project, TARGET};
use spacewasm_tools::{Resolved, Symbols, resolve_offsets, resolve_pcs};
use std::path::{Path, PathBuf};

pub fn run(args: &Backtrace) -> Result<bool> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;
    let project = Project::find(&cwd);
    let path = symbol_file(args, project.as_ref().ok())?;
    let symbols = Symbols::read(&path)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("{} is not a usable symbol file", path.display()))?;

    // A symbol file from another build would name plausible, wrong lines.
    let module = args
        .module
        .clone()
        .unwrap_or_else(|| path.with_extension("wasm"));
    match std::fs::read(&module) {
        Ok(flown) if !symbols.describes(&flown) => bail!(
            "{} is not the module {} was built with. Rebuild both together, and name the \
             symbols that came with the module that flew",
            module.display(),
            path.display()
        ),
        Ok(_) => {}
        // Beside the `.sym` is only a default; without it the embedded copy stands.
        Err(_) if args.module.is_none() => {}
        Err(err) => bail!("could not read {}: {err}", module.display()),
    }

    let resolved = match args.pc.is_empty() {
        true => resolve_offsets(&symbols, &args.offset),
        false => {
            let pcs = args
                .pc
                .iter()
                .map(|&pc| u32::try_from(pc).with_context(|| format!("{pc:#x} is not a 32-bit PC")))
                .collect::<Result<Vec<_>>>()?;
            let limits =
                fprime_test::config::load(args.limits.as_deref(), project.as_ref().ok())?.limits;
            // The replay is this tool's decoder, whose IR an older one need not share; and
            // one that old reports no PCs to begin with.
            if limits.fprime.older_interpreter().is_some() {
                bail!(
                    "{} reports no PCs, and its IR is not this tool's to replay. Resolve \
                     byte offsets with `--offset` instead",
                    limits.fprime
                );
            }
            // One replay maps every PC: the layout is the same for each.
            let map = fprime_test::interpreter::ir_map(symbols.module(), &limits)?;
            resolve_pcs(&symbols, &map, &pcs)
        }
    }
    .map_err(|err| anyhow!("{err}"))?;

    print!("{}", render(&resolved));
    Ok(resolved.iter().all(|entry| !entry.frames.is_empty()))
}

/// One block per position; numbered when there is more than one, as a backtrace.
fn render(resolved: &[Resolved]) -> String {
    let numbered = resolved.len() > 1;
    let mut out = String::new();
    for (depth, entry) in resolved.iter().enumerate() {
        if numbered {
            out.push_str(&format!("#{depth} "));
        }
        out.push_str(&format!("{:#x}", entry.offset));
        if entry.frames.is_empty() {
            out.push_str(": no source recorded\n");
            continue;
        }
        out.push_str(":\n");
        for frame in &entry.frames {
            let function = frame.function.as_deref().unwrap_or("??");
            match &frame.location {
                Some(location) => out.push_str(&format!("  {function}\n      at {location}\n")),
                None => out.push_str(&format!("  {function}\n")),
            }
        }
    }
    out
}

/// `sequence` as a path if it is one, else that sequence's `.sym` in the build output.
fn symbol_file(args: &Backtrace, project: Option<&Project>) -> Result<PathBuf> {
    let named = Path::new(&args.sequence);
    if named
        .extension()
        .is_some_and(|extension| extension == "sym")
    {
        return Ok(named.to_path_buf());
    }

    let project = project.with_context(|| {
        format!(
            "`{}` is not a .sym file, and there is no crate here to find a sequence in",
            args.sequence
        )
    })?;
    let profile = if args.debug { "debug" } else { "release" };
    let directory = project.artifact_dir(profile).with_context(|| {
        format!("no {profile} build found for {TARGET}. Run `fprime-wasm build`")
    })?;
    let path = directory.join(format!("{}.sym", args.sequence));
    if !path.is_file() {
        bail!(
            "no symbols for `{}` in {}. Run `fprime-wasm build`. A project made before symbol \
             files existed also needs the `linker` and `[env]` of a fresh `fprime-wasm init`'s \
             `.cargo/config.toml`",
            args.sequence,
            directory.display()
        );
    }
    Ok(path)
}

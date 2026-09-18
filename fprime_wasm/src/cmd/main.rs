//! The `fprime-wasm` command. Everything it does is in [`fprime_wasm`]; these
//! modules are the argument surface and the printing.
//!
//! * [`cli`] — the flags, and the only place clap appears
//! * [`init`], [`add`], [`verify`] — one module per subcommand
//! * [`dictionary`] — finding, validating and installing a deployment's dictionary,
//!   which both `init` and `verify` need
//! * [`hex`] — the `<key>=<hex>` value syntax the `verify` flags take

mod add;
mod cli;
mod dictionary;
mod hex;
mod init;
mod verify;

use clap::Parser;
use cli::{Cli, Command};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init(args) => init::run(&args).map(|()| true),
        Command::Add(args) => add::run(&args).map(|()| true),
        Command::Verify(args) => verify::run(&args),
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        // A completed check that found problems: reported already, no error to add.
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

//! The `fprime-wasm` CLI: argument parsing and printing; behaviour lives in [`fprime_wasm`].

mod add;
mod build;
mod cargo;
mod cli;
mod dictionary;
mod init;
mod test;
mod verify;

use clap::Parser;
use cli::{Cli, Command};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init(args) => init::run(&args).map(|()| true),
        Command::Add(args) => add::run(&args).map(|()| true),
        Command::Build(args) => build::run(&args),
        Command::Test(args) => test::run(&args),
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

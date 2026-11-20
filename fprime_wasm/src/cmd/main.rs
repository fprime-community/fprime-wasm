//! The `fprime-wasm` CLI: argument parsing and printing; behaviour lives in [`fprime_wasm`].

mod add;
mod backtrace;
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
    // A scaffolded project names this binary as Cargo's `linker`, so `spacewasm` need not
    // be installed as well, and the link validates against the interpreter `verify` runs.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if spacewasm_tools::is_link(&args) {
        return link(&args);
    }

    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init(args) => init::run(&args).map(|()| true),
        Command::Add(args) => add::run(&args).map(|()| true),
        Command::Build(args) => build::run(&args),
        Command::Test(args) => test::run(&args),
        Command::Verify(args) => verify::run(&args),
        Command::Backtrace(args) => backtrace::run(&args),
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

/// Link for the F Prime `[env] FPRIME_VERSION` names, which Cargo passes down through rustc.
fn link(args: &[std::ffi::OsString]) -> ExitCode {
    let release = match fprime_test::fprime::Fprime::configured(None) {
        Ok(fprime) => fprime.and_then(|fprime| fprime.older_interpreter()),
        Err(err) => {
            eprintln!("error: {err:#}");
            return ExitCode::FAILURE;
        }
    };
    match spacewasm_tools::link_for(args, release) {
        Ok(true) => ExitCode::SUCCESS,
        // The linker failed and has said why.
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

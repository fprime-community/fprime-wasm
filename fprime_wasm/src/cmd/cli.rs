//! The command line: every flag, and the only place clap appears.

use clap::{Args, Parser, Subcommand};
use fprime_wasm::scaffold::{DEFAULT_SEQUENCE, DEFAULT_STACK_SIZE};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "fprime-wasm",
    version,
    about = "Create and inspect F Prime Wasm sequence projects",
    long_about = "Scaffold a crate of F Prime sequences, add sequences to it, and size a compiled \
                  sequence against the on-board Wasm interpreter."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Fill a directory with a sequence project.
    Init(Init),
    /// Add a sequence to the crate in the current directory.
    Add(Add),
    /// Compile the sequences to Wasm.
    Build(Build),
    /// Run the sequence tests.
    Test(Test),
    /// Load compiled sequences on the on-board interpreter and size them.
    Verify(Verify),
}

#[derive(Args)]
pub struct Init {
    /// Directory to fill. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub directory: PathBuf,

    /// Crate name. Defaults to the directory's name.
    #[arg(long)]
    pub name: Option<String>,

    /// First sequence to create.
    #[arg(long, default_value = DEFAULT_SEQUENCE)]
    pub sequence: String,

    /// The deployment's JSON dictionary. Copied into the project. Prompted for if
    /// the project does not already have one.
    #[arg(long)]
    pub dictionary: Option<PathBuf>,

    /// Guest stack to reserve, in bytes.
    #[arg(long, default_value_t = DEFAULT_STACK_SIZE)]
    pub stack_size: usize,

    /// Depend on the `fprime_*` crates at this version instead of the tool's own.
    #[arg(long, conflicts_with = "local")]
    pub crate_version: Option<String>,

    /// Depend on the `fprime_*` crates by path, from a checkout of this repository.
    #[arg(long)]
    pub local: Option<PathBuf>,
}

#[derive(Args)]
pub struct Add {
    /// Sequence name. Becomes `src/bin/<name>.rs` and a `[[bin]]` entry.
    pub name: String,
}

#[derive(Args)]
pub struct Build {
    /// Build without optimisation, for a readable module in a debugger.
    #[arg(long)]
    pub debug: bool,

    /// Extra arguments for `cargo build`.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo: Vec<String>,
}

#[derive(Args)]
pub struct Test {
    /// Only run tests whose name contains this. Repeatable.
    pub filters: Vec<String>,

    /// Test against the debug build of the sequences rather than the release build.
    #[arg(long)]
    pub debug: bool,

    /// Use the sequences already built, instead of building them first.
    #[arg(long)]
    pub no_build: bool,

    /// Extra arguments for `cargo test`.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo: Vec<String>,
}

#[derive(Args)]
pub struct Verify {
    /// Modules to check. Defaults to every `.wasm` in the crate's build output.
    pub modules: Vec<PathBuf>,

    /// Check the debug build rather than the release build.
    #[arg(long)]
    pub debug: bool,

    /// Use the sequences already built, instead of building them first. Ignored
    /// when modules are named directly.
    #[arg(long)]
    pub no_build: bool,

    /// Expand each module: every budget, and the guest and interpreter figures.
    #[arg(long, short)]
    pub verbose: bool,

    /// Report as JSON on stdout instead of tables. Diagnostics stay on stderr, so the
    /// output can be piped straight into `jq`.
    #[arg(long)]
    pub json: bool,

    /// Limits to measure against. Defaults to `sequencer.toml` at the crate root,
    /// else a stock sequencer.
    #[arg(long, value_name = "PATH")]
    pub limits: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_is_internally_consistent() {
        // Catches conflicting flags, duplicate names and bad defaults.
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}

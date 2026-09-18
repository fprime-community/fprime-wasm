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
    /// Run compiled sequences on the on-board interpreter and size them.
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
pub struct Verify {
    /// Modules to check. Defaults to every `.wasm` in the crate's build output.
    pub modules: Vec<PathBuf>,

    /// Check the debug build rather than the release build.
    #[arg(long)]
    pub debug: bool,

    /// A dictionary, used to name the commands, channels and parameters a sequence
    /// touches. Found automatically if the project has one.
    #[arg(long)]
    pub dictionary: Option<PathBuf>,

    /// Expand each module: every budget, the guest and interpreter figures, and the
    /// commands, channels and parameters it touched.
    #[arg(long, short)]
    pub verbose: bool,

    /// Print every host call the sequence made. Implies --verbose.
    #[arg(long)]
    pub trace: bool,

    /// Report as JSON on stdout instead of tables. Diagnostics stay on stderr, so the
    /// output can be piped straight into `jq`. `--trace` adds the host calls.
    #[arg(long)]
    pub json: bool,

    /// Sequence arguments, as hex bytes, delivered through `fprime_v1.args`.
    #[arg(long, value_name = "HEX")]
    pub args: Option<String>,

    /// Value a telemetry channel reads as, as `<id>=<hex>`. Repeatable.
    #[arg(long = "tlm", value_name = "ID=HEX")]
    pub telemetry: Vec<String>,

    /// Value a parameter reads as, as `<id>=<hex>`. Repeatable.
    #[arg(long = "prm", value_name = "ID=HEX")]
    pub parameters: Vec<String>,

    /// Message a serial port receives, as `<index>=<hex>`. Delivered once. Repeatable.
    #[arg(long = "serial", value_name = "INDEX=HEX")]
    pub serial: Vec<String>,

    /// Limits to measure against. Defaults to `sequencer.toml` at the crate root,
    /// else a stock sequencer.
    #[arg(long, value_name = "PATH")]
    pub limits: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cli_is_internally_consistent() {
        // Catches conflicting flags, duplicate names and bad defaults, which clap only
        // validates when asked.
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}

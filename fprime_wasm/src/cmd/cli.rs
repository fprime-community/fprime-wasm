//! The command line: every flag, and the only place clap appears.

use clap::{Args, Parser, Subcommand};
use fprime_test::fprime::Fprime;
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
    /// Resolve a backtrace, or positions in a sequence, to source lines.
    Backtrace(Backtrace),
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

    /// The F Prime the sequences fly on, e.g. `4.4.0`. Its `spacewasm` release decides
    /// which instructions the compiler and `wasm-opt` may emit. Defaults to the newest
    /// this tool knows.
    #[arg(long, value_name = "VERSION", value_parser = parse_fprime)]
    pub fprime_version: Option<Fprime>,

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

#[derive(Args)]
#[command(group(clap::ArgGroup::new("position").required(true).args(["offset", "pc"])))]
pub struct Backtrace {
    /// A sequence of the crate in the current directory, or a `.sym` file.
    pub sequence: String,

    /// Byte offsets into the uplinked `.wasm`, from its first byte. Hex with `0x`. Several,
    /// comma-separated, are a backtrace: innermost first.
    #[arg(long, value_parser = parse_number, value_delimiter = ',', num_args = 1..)]
    pub offset: Vec<u64>,

    /// Interpreter program counters, in IR words from the module's first, as
    /// `spacewasm_backtrace` reports them. Hex with `0x`. Several, comma-separated, are a
    /// backtrace: innermost first, then each caller's `call`. A sequence paused in a host
    /// call reports that `call` first.
    #[arg(long, value_parser = parse_number, value_delimiter = ',', num_args = 1..)]
    pub pc: Vec<u64>,

    /// The `.wasm` that flew, checked against the one the symbols were built with.
    /// Defaults to the one beside the `.sym`.
    #[arg(long, value_name = "PATH")]
    pub module: Option<PathBuf>,

    /// Look in the debug build rather than the release build.
    #[arg(long)]
    pub debug: bool,

    /// Limits the PC was taken under, which decide the IR layout. Defaults as for
    /// `verify`.
    #[arg(long, value_name = "PATH")]
    pub limits: Option<PathBuf>,
}

fn parse_fprime(text: &str) -> Result<Fprime, String> {
    Fprime::parse(text).map_err(|err| format!("{err:#}"))
}

fn parse_number(text: &str) -> Result<u64, String> {
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    parsed.map_err(|err| format!("`{text}` is not a number: {err}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn numbers_parse_in_decimal_and_hex() {
        assert_eq!(super::parse_number("170"), Ok(170));
        assert_eq!(super::parse_number("0xaa"), Ok(170));
        assert!(super::parse_number("aa").is_err());
    }

    #[test]
    fn pcs_take_a_list_or_repeat() {
        use clap::Parser;
        let parse = |args: &[&str]| match Cli::try_parse_from(args).expect("parses").command {
            Command::Backtrace(backtrace) => backtrace.pc,
            _ => unreachable!(),
        };
        let listed = parse(&["fprime-wasm", "backtrace", "seq", "--pc", "0x10,4,0x30"]);
        assert_eq!(listed, [0x10, 4, 0x30]);
        let repeated = parse(&["fprime-wasm", "backtrace", "seq", "--pc", "1", "--pc", "2"]);
        assert_eq!(repeated, [1, 2]);
    }

    #[test]
    fn init_takes_an_fprime_version() {
        use clap::Parser;
        let init = |args: &[&str]| match Cli::try_parse_from(args).map(|cli| cli.command) {
            Ok(Command::Init(init)) => Ok(init.fprime_version),
            Ok(_) => unreachable!(),
            Err(err) => Err(err),
        };
        let fprime = init(&["fprime-wasm", "init", "--fprime-version", "4.4.0"])
            .expect("parses")
            .expect("given");
        assert_eq!(fprime.spacewasm.to_string(), "0.7.1");
        assert!(init(&["fprime-wasm", "init", "--fprime-version", "4.3.0"]).is_err());
    }

    #[test]
    fn a_position_is_required() {
        use clap::Parser;
        assert!(Cli::try_parse_from(["fprime-wasm", "backtrace", "seq"]).is_err());
    }

    use super::*;

    #[test]
    fn cli_is_internally_consistent() {
        // Catches conflicting flags, duplicate names and bad defaults.
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}

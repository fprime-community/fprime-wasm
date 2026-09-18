//! The command line: every flag, and the only place clap appears.

use clap::{Args, Parser, Subcommand};
use fprime_wasm::harness::Limits;
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

    #[command(flatten)]
    pub limits: LimitArgs,
}

/// The `Svc::WasmSequencer` configuration a module is measured against. Defaults are
/// the on-board defaults, so `verify` with no flags answers "does this fit a stock
/// sequencer?".
#[derive(Args)]
pub struct LimitArgs {
    /// `WASM_SEQ_SPACEWASM_PAGE_SIZE`, in bytes.
    #[arg(long, default_value_t = Limits::default().page_size)]
    pub page_size: usize,

    /// `Config::heapPages`.
    #[arg(long, default_value_t = Limits::default().heap_pages)]
    pub heap_pages: u32,

    /// `Config::guestMemorySize`, in bytes.
    #[arg(long, default_value_t = Limits::default().guest_memory)]
    pub guest_memory: u64,

    /// `Config::stackSize`, in 32-bit words.
    #[arg(long, default_value_t = Limits::default().stack_size)]
    pub stack_size: usize,

    /// `Config::maxCodePages`.
    #[arg(long, default_value_t = Limits::default().max_code_pages)]
    pub max_code_pages: usize,

    /// `Config::maxGuestModules`.
    #[arg(long, default_value_t = Limits::default().max_guest_modules)]
    pub max_guest_modules: u8,

    /// `Wasm.GUEST_EVENT_MESSAGE_SIZE`, past which a guest event message is
    /// truncated. This binds before `FW_LOG_STRING_MAX_SIZE`.
    #[arg(long, default_value_t = fprime_wasm::abi::EVENT_MESSAGE_MAX)]
    pub event_message_max: usize,

    /// `Wasm.MAX_SERIAL_IN_PORTS` / `MAX_SERIAL_OUT_PORTS`.
    #[arg(long, default_value_t = fprime_wasm::abi::MAX_SERIAL_PORTS)]
    pub serial_ports: i32,

    /// Stop a sequence that has executed this many instructions.
    #[arg(long, default_value_t = Limits::default().max_instructions)]
    pub max_instructions: u64,

    /// Instructions between operand-stack samples. One is exact.
    #[arg(long, default_value_t = Limits::default().stack_sample)]
    pub stack_sample: usize,
}

impl LimitArgs {
    pub fn to_limits(&self) -> Limits {
        Limits {
            page_size: self.page_size,
            heap_pages: self.heap_pages,
            guest_memory: self.guest_memory,
            stack_size: self.stack_size,
            max_code_pages: self.max_code_pages,
            max_guest_modules: self.max_guest_modules,
            max_instructions: self.max_instructions,
            stack_sample: self.stack_sample,
        }
    }
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

    /// The limit flags exist to mirror `WasmSequencer::Config`; their defaults must
    /// not drift from the harness defaults they document.
    #[test]
    fn limit_defaults_match_the_harness() {
        use clap::CommandFactory;
        let cli = Cli::command();
        let verify = cli
            .get_subcommands()
            .find(|command| command.get_name() == "verify")
            .expect("a verify subcommand");
        let default_of = |name: &str| {
            verify
                .get_arguments()
                .find(|argument| argument.get_id() == name)
                .unwrap_or_else(|| panic!("--{name} should exist"))
                .get_default_values()
                .first()
                .expect("a default")
                .to_string_lossy()
                .into_owned()
        };

        let limits = Limits::default();
        assert_eq!(default_of("page_size"), limits.page_size.to_string());
        assert_eq!(default_of("heap_pages"), limits.heap_pages.to_string());
        assert_eq!(default_of("guest_memory"), limits.guest_memory.to_string());
        assert_eq!(default_of("stack_size"), limits.stack_size.to_string());
        assert_eq!(
            default_of("max_code_pages"),
            limits.max_code_pages.to_string()
        );
        assert_eq!(
            default_of("max_guest_modules"),
            limits.max_guest_modules.to_string()
        );
    }

    #[test]
    fn limit_arguments_convert_to_harness_limits() {
        // `try_parse_from`, not `parse_from`: clap *exits the process* on an
        // unrecognised subcommand, which takes the whole test binary with it and
        // reports as an opaque abort rather than a failed assertion.
        let cli = Cli::try_parse_from([
            "fprime-wasm",
            "verify",
            "--page-size",
            "16384",
            "--heap-pages",
            "12",
            "--guest-memory",
            "4096",
            "--stack-size",
            "2048",
            "--max-code-pages",
            "64",
            "--max-guest-modules",
            "2",
        ])
        .expect("the verify subcommand should parse");
        let Command::Verify(verify) = cli.command else {
            panic!("expected the verify subcommand");
        };
        let limits = verify.limits.to_limits();
        assert_eq!(limits.page_size, 16384);
        assert_eq!(limits.heap_pages, 12);
        assert_eq!(limits.guest_memory, 4096);
        assert_eq!(limits.stack_size, 2048);
        assert_eq!(limits.max_code_pages, 64);
        assert_eq!(limits.max_guest_modules, 2);
    }
}

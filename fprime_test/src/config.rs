//! `sequencer.toml`: the configuration a project is loaded and run under.

use crate::abi;
use crate::interpreter::Limits;
use crate::project::Project;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Name at the crate root.
pub const FILE: &str = "sequencer.toml";

/// Which configuration a run used, for the report header: a mistyped file name would
/// otherwise report stock defaults as if they were the deployment's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// As the user would recognise it: what `--limits` was given, or the bare [`FILE`]
    /// when it was found at the project root. Not necessarily a path to open.
    File(PathBuf),
    Defaults,
}

impl Source {
    /// Short label for the header.
    pub fn label(&self) -> String {
        match self {
            Source::File(path) => path.display().to_string(),
            Source::Defaults => "defaults".to_string(),
        }
    }
}

/// A configuration and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub limits: Limits,
    pub source: Source,
}

/// The configuration to measure against: whatever was asked for, else the project's,
/// else a stock sequencer.
///
/// An explicit path that cannot be read is fatal — it is an instruction. A project
/// with no [`FILE`] is not: projects created before it existed still verify.
pub fn load(explicit: Option<&Path>, project: Option<&Project>) -> Result<Loaded> {
    match (explicit, project) {
        (Some(path), _) => read(path, path),
        (None, Some(project)) => {
            let candidate = project.root().join(FILE);
            if !candidate.is_file() {
                return Ok(defaults());
            }
            read(&candidate, Path::new(FILE))
        }
        (None, None) => Ok(defaults()),
    }
}

/// A configuration named relative to `root`, which is the crate root — what
/// `#[fprime_test(limits = "...")]` selects, for a deployment that configures more than one
/// `WasmSequencer` instance.
///
/// Fatal when it is not there, as `explicit` is in [`load`]: naming one is an instruction. The
/// macro checks the same join at compile time, so this only fires if the file went away
/// between building and running.
pub fn load_named(root: &Path, named: &Path) -> Result<Loaded> {
    read(&root.join(named), named)
}

fn defaults() -> Loaded {
    Loaded {
        limits: Limits::default(),
        source: Source::Defaults,
    }
}

/// `path` is what to open; `named` is what to call it in the report.
fn read(path: &Path, named: &Path) -> Result<Loaded> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let limits = parse(&text).with_context(|| format!("in {}", named.display()))?;
    Ok(Loaded {
        limits,
        source: Source::File(named.to_path_buf()),
    })
}

pub fn parse(text: &str) -> Result<Limits> {
    let file: File = toml_edit::de::from_str(text)?;
    Ok(Limits {
        page_size: file.constants.page_size,
        heap_pages: file.config.heap_pages,
        guest_memory: file.config.guest_memory,
        stack_size: file.config.stack_size,
        max_code_pages: file.config.max_code_pages,
        max_guest_modules: file.config.max_guest_modules,
        event_message_max: file.constants.event_message_max,
        serial_ports: file.constants.serial_ports,
        max_instructions: file.test.max_instructions,
        stack_sample: file.test.stack_sample,
    })
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct File {
    config: Config,
    constants: Constants,
    test: Test,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    heap_pages: u32,
    guest_memory: u64,
    stack_size: usize,
    max_code_pages: usize,
    max_guest_modules: u8,
}

impl Default for Config {
    fn default() -> Self {
        let limits = Limits::default();
        Config {
            heap_pages: limits.heap_pages,
            guest_memory: limits.guest_memory,
            stack_size: limits.stack_size,
            max_code_pages: limits.max_code_pages,
            max_guest_modules: limits.max_guest_modules,
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Constants {
    page_size: usize,
    event_message_max: usize,
    serial_ports: i32,
}

impl Default for Constants {
    fn default() -> Self {
        Constants {
            page_size: Limits::default().page_size,
            event_message_max: abi::EVENT_MESSAGE_MAX,
            serial_ports: abi::MAX_SERIAL_PORTS,
        }
    }
}

/// Knobs only a run uses: `verify` never executes a sequence.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Test {
    max_instructions: u64,
    stack_sample: usize,
}

impl Default for Test {
    fn default() -> Self {
        let limits = Limits::default();
        Test {
            max_instructions: limits.max_instructions,
            stack_sample: limits.stack_sample,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file `fprime-wasm init` writes is checked against this in
    /// `fprime_wasm::scaffold::template`, which is where the template lives.
    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(parse("").expect("empty is valid"), Limits::default());
    }

    /// A deployment that differs in one setting should have to write down only that
    /// one.
    #[test]
    fn a_partial_file_changes_only_what_it_names() {
        let limits = parse("[config]\nguest_memory = 4096\n").expect("valid");
        assert_eq!(limits.guest_memory, 4096);
        assert_eq!(
            limits.heap_pages,
            Limits::default().heap_pages,
            "an unnamed key must keep its default"
        );
        assert_eq!(limits.event_message_max, abi::EVENT_MESSAGE_MAX);
    }

    /// The two knobs a run needs and a load does not.
    #[test]
    fn the_test_section_sets_the_run_knobs() {
        let limits = parse("[test]\nmax_instructions = 5\nstack_sample = 4\n").expect("valid");
        assert_eq!(limits.max_instructions, 5);
        assert_eq!(limits.stack_sample, 4);
        // The section it replaced must not still be accepted, or a file keeping the old
        // name would silently stop configuring anything.
        assert!(parse("[verify]\nmax_instructions = 5\n").is_err());
    }

    /// The whole reason for `deny_unknown_fields`: a misspelled key that was silently
    /// ignored would measure against stock defaults while looking configured.
    #[test]
    fn a_misspelled_key_is_an_error_that_names_it() {
        let err = parse("[config]\nguest_memoy = 4096\n").expect_err("should not parse");
        assert!(err.to_string().contains("guest_memoy"), "{err}");
    }

    #[test]
    fn an_unknown_section_is_an_error() {
        assert!(parse("[limits]\nguest_memory = 1\n").is_err());
    }

    #[test]
    fn a_value_of_the_wrong_type_is_an_error() {
        let err = parse("[config]\nguest_memory = \"lots\"\n").expect_err("should not parse");
        assert!(err.to_string().contains("guest_memory"), "{err}");
    }

    /// `--limits` is an instruction, so a path that is not there must fail rather
    /// than quietly fall back.
    #[test]
    fn an_explicit_path_that_is_missing_is_fatal() {
        let missing = std::env::temp_dir().join("fprime-wasm-no-such-sequencer.toml");
        assert!(load(Some(&missing), None).is_err());
    }

    /// `#[fprime_test(limits = "...")]`: joined against the crate root, but reported by the
    /// name the author wrote — an absolute path in a failure header would be noise.
    #[test]
    fn a_named_file_is_read_from_the_root_and_labelled_as_written() {
        let root = std::env::temp_dir().join(format!(
            "fprime-test-named-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&root).expect("temp dir");
        std::fs::write(
            root.join("sequencer-b.toml"),
            "[config]\nguest_memory = 4096\n",
        )
        .expect("write");

        let loaded = load_named(&root, Path::new("sequencer-b.toml")).expect("it is there");
        assert_eq!(loaded.limits.guest_memory, 4096);
        assert_eq!(loaded.source.label(), "sequencer-b.toml");

        // Missing is fatal, unlike the project's own file: naming one is an instruction.
        assert!(load_named(&root, Path::new("sequencer-c.toml")).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn no_project_and_no_flag_is_the_defaults() {
        let loaded = load(None, None).expect("defaults are always available");
        assert_eq!(loaded.limits, Limits::default());
        assert_eq!(loaded.source, Source::Defaults);
        assert_eq!(loaded.source.label(), "defaults");
    }
}

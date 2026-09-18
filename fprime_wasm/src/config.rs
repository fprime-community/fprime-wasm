//! `sequencer.toml`: the configuration a project is measured against.

use crate::abi;
use crate::harness::Limits;
use crate::project::Project;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Name at the crate root.
pub const FILE: &str = "sequencer.toml";

/// A deployment's sequencer configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sequencer {
    pub limits: Limits,
    /// `Wasm.GUEST_EVENT_MESSAGE_SIZE`.
    pub event_message_max: usize,
    /// `Wasm.MAX_SERIAL_{IN,OUT}_PORTS`.
    pub serial_ports: i32,
}

impl Default for Sequencer {
    fn default() -> Self {
        Sequencer {
            limits: Limits::default(),
            event_message_max: abi::EVENT_MESSAGE_MAX,
            serial_ports: abi::MAX_SERIAL_PORTS,
        }
    }
}

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
    pub sequencer: Sequencer,
    pub source: Source,
}

/// The configuration to measure against: whatever was asked for, else the project's,
/// else a stock sequencer.
///
/// An explicit path that cannot be read is fatal — it is an instruction. A project
/// with no [`FILE`] is not: projects created before it existed still verify.
pub fn load(explicit: Option<&Path>, project: Option<&Project>) -> Result<Loaded> {
    let defaults = || Loaded {
        sequencer: Sequencer::default(),
        source: Source::Defaults,
    };

    // `path` is what to open; `named` is what to call it in the report.
    let (path, named) = match (explicit, project) {
        (Some(path), _) => (path.to_path_buf(), path.to_path_buf()),
        (None, Some(project)) => {
            let candidate = project.root().join(FILE);
            if !candidate.is_file() {
                return Ok(defaults());
            }
            (candidate, PathBuf::from(FILE))
        }
        (None, None) => return Ok(defaults()),
    };

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let sequencer = parse(&text).with_context(|| format!("in {}", named.display()))?;
    Ok(Loaded {
        sequencer,
        source: Source::File(named),
    })
}

pub fn parse(text: &str) -> Result<Sequencer> {
    let file: File = toml_edit::de::from_str(text)?;
    Ok(Sequencer {
        limits: Limits {
            page_size: file.constants.page_size,
            heap_pages: file.config.heap_pages,
            guest_memory: file.config.guest_memory,
            stack_size: file.config.stack_size,
            max_code_pages: file.config.max_code_pages,
            max_guest_modules: file.config.max_guest_modules,
            max_instructions: file.verify.max_instructions,
            stack_sample: file.verify.stack_sample,
        },
        event_message_max: file.constants.event_message_max,
        serial_ports: file.constants.serial_ports,
    })
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct File {
    config: Config,
    constants: Constants,
    verify: Verify,
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

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Verify {
    max_instructions: u64,
    stack_sample: usize,
}

impl Default for Verify {
    fn default() -> Self {
        let limits = Limits::default();
        Verify {
            max_instructions: limits.max_instructions,
            stack_sample: limits.stack_sample,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::template;

    /// The file `init` writes must be one this parses, and must say what the tool
    /// would have assumed anyway — otherwise a scaffolded project is measured against
    /// values it does not contain.
    #[test]
    fn the_generated_file_is_the_defaults() {
        let generated = parse(template::SEQUENCER.body).expect("the template should parse");
        assert_eq!(generated, Sequencer::default());
    }

    #[test]
    fn defaults_come_from_the_harness_and_the_abi() {
        let sequencer = Sequencer::default();
        assert_eq!(sequencer.limits, Limits::default());
        assert_eq!(sequencer.event_message_max, abi::EVENT_MESSAGE_MAX);
        assert_eq!(sequencer.serial_ports, abi::MAX_SERIAL_PORTS);
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(parse("").expect("empty is valid"), Sequencer::default());
    }

    /// A deployment that differs in one setting should have to write down only that
    /// one.
    #[test]
    fn a_partial_file_changes_only_what_it_names() {
        let sequencer = parse("[config]\nguest_memory = 4096\n").expect("valid");
        assert_eq!(sequencer.limits.guest_memory, 4096);
        assert_eq!(
            sequencer.limits.heap_pages,
            Limits::default().heap_pages,
            "an unnamed key must keep its default"
        );
        assert_eq!(sequencer.event_message_max, abi::EVENT_MESSAGE_MAX);
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

    #[test]
    fn no_project_and_no_flag_is_the_defaults() {
        let loaded = load(None, None).expect("defaults are always available");
        assert_eq!(loaded.sequencer, Sequencer::default());
        assert_eq!(loaded.source, Source::Defaults);
        assert_eq!(loaded.source.label(), "defaults");
    }
}

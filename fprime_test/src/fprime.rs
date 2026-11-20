//! Which F Prime the sequences fly on, and so which `spacewasm` release: its features
//! decide what the compiler and `wasm-opt` may emit.
//!
//! A project names it as [`ENV`] in `.cargo/config.toml`'s `[env]`, which Cargo hands to
//! the linker and to `cargo test`; `verify` and `backtrace` run outside Cargo and read
//! the file instead.

use anyhow::{Context, Result, bail};
use spacewasm_tools::Version;
use std::fmt;
use std::path::Path;

/// The variable, and the `[env]` key.
pub const ENV: &str = "FPRIME_VERSION";

/// The first F Prime of each `spacewasm` release, oldest first: `fprime_spacewasm/Cargo.toml`
/// in `Svc/WasmSequencer`. 4.4.0 is the first with a `WasmSequencer` at all.
const RELEASES: &[(Version, Version)] = &[
    (Version::new(4, 4, 0), Version::new(0, 7, 1)),
    (Version::new(4, 5, 0), Version::new(0, 8, 0)),
];

/// An F Prime release and the interpreter it flies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fprime {
    pub version: Version,
    pub spacewasm: Version,
}

impl Fprime {
    /// The newest F Prime this tool knows, for a project that does not say.
    pub fn latest() -> Fprime {
        let &(version, spacewasm) = RELEASES.last().expect("at least one release");
        Fprime { version, spacewasm }
    }

    /// `4.4.0`, `v4.4.1`, a pre-release such as `4.5.0a1`, or the `git describe` a
    /// development build puts in its dictionary (`v4.4.0-12-gabc123`), which has the
    /// features of the release it is ahead of.
    pub fn parse(text: &str) -> Result<Fprime> {
        let version = parse_version(text)
            .with_context(|| format!("`{text}` is not an F Prime version, e.g. `4.5.0`"))?;
        let Some(&(_, spacewasm)) = RELEASES.iter().rev().find(|(first, _)| *first <= version)
        else {
            bail!(
                "F Prime {version} has no WasmSequencer; it arrived in {}",
                RELEASES[0].0
            );
        };
        Ok(Fprime { version, spacewasm })
    }

    /// The release to hold builds and loads to, when it lacks something the `spacewasm`
    /// this tool decodes with implements; `None` when the two agree, as for `latest`. This
    /// tool's decoder would accept what that one refuses, so it has to be asked.
    pub fn older_interpreter(&self) -> Option<Version> {
        let release = self.spacewasm;
        spacewasm_tools::PROPOSALS
            .iter()
            .any(|proposal| proposal.supported() && !proposal.supported_in(release))
            .then_some(release)
    }

    /// [`ENV`] from the environment, else from the `.cargo/config.toml` Cargo would read
    /// for `root`: the nearest one that sets it. `None` when nothing does.
    pub fn configured(root: Option<&Path>) -> Result<Option<Fprime>> {
        if let Ok(text) = std::env::var(ENV) {
            return Fprime::parse(&text)
                .with_context(|| format!("in the {ENV} environment variable"))
                .map(Some);
        }
        let Some(root) = root else {
            return Ok(None);
        };
        for directory in root.ancestors() {
            let path = directory.join(".cargo").join("config.toml");
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(value) = from_cargo_config(&text) {
                return Fprime::parse(&value)
                    .with_context(|| format!("in {}", path.display()))
                    .map(Some);
            }
        }
        Ok(None)
    }
}

impl fmt::Display for Fprime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "F Prime {} (spacewasm {})", self.version, self.spacewasm)
    }
}

/// `MAJOR.MINOR.PATCH`, each part's leading digits; whatever follows the patch is ignored.
fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim();
    let mut parts = text.strip_prefix('v').unwrap_or(text).splitn(3, '.');
    let mut number = |whole: bool| {
        let part = parts.next()?;
        let digits = part.len() - part.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        (digits > 0 && (!whole || digits == part.len()))
            .then(|| part[..digits].parse().ok())
            .flatten()
    };
    Some(Version::new(number(true)?, number(true)?, number(false)?))
}

/// `[env] FPRIME_VERSION`, as a plain string or as `{ value = "..." }`.
fn from_cargo_config(text: &str) -> Option<String> {
    let document: toml_edit::DocumentMut = text.parse().ok()?;
    let entry = document.get("env")?.get(ENV)?;
    entry
        .as_str()
        .or_else(|| entry.get("value")?.as_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_release_maps_to_the_interpreter_it_flies() {
        let spacewasm = |text| Fprime::parse(text).expect("known").spacewasm;
        assert_eq!(spacewasm("4.4.0"), Version::new(0, 7, 1));
        assert_eq!(spacewasm("v4.4.1"), Version::new(0, 7, 1));
        // `devel` after the 4.4.0 tag still pins 0.7.1.
        assert_eq!(spacewasm("v4.4.0-12-gabc1234"), Version::new(0, 7, 1));
        assert_eq!(spacewasm("4.5.0"), Version::new(0, 8, 0));
        assert_eq!(spacewasm("4.5.0a1"), Version::new(0, 8, 0));
        assert_eq!(spacewasm("5.0.0"), Version::new(0, 8, 0));
        assert_eq!(Fprime::latest().spacewasm, Version::new(0, 8, 0));
    }

    #[test]
    fn only_an_older_interpreter_needs_holding_to() {
        let older = |text| Fprime::parse(text).unwrap().older_interpreter();
        assert_eq!(older("4.4.0"), Some(Version::new(0, 7, 1)));
        assert_eq!(older("4.5.0"), None);
        assert_eq!(Fprime::latest().older_interpreter(), None);
    }

    #[test]
    fn a_release_without_a_sequencer_or_a_version_is_refused() {
        let message = format!("{:#}", Fprime::parse("4.3.0").unwrap_err());
        assert!(message.contains("4.4.0"), "{message}");
        assert!(Fprime::parse("curated-fixture").is_err());
        assert!(Fprime::parse("4.5").is_err());
    }

    #[test]
    fn cargo_config_spells_it_either_way() {
        assert_eq!(
            from_cargo_config("[env]\nFPRIME_VERSION = \"4.4.0\"\n").as_deref(),
            Some("4.4.0")
        );
        assert_eq!(
            from_cargo_config("[env]\nFPRIME_VERSION = { value = \"4.5.0\" }\n").as_deref(),
            Some("4.5.0")
        );
        assert_eq!(from_cargo_config("[env]\nOTHER = \"1\"\n"), None);
    }
}

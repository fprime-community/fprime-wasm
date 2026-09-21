//! Shelling out to cargo, for the `build` and `test` subcommands.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// The profile a subcommand was asked for, as cargo and the artefact path spell it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub fn from_debug_flag(debug: bool) -> Self {
        match debug {
            true => Profile::Debug,
            false => Profile::Release,
        }
    }

    /// Directory name under `target/<triple>/`; matches [`fprime_test::project::Project::artifact_dir`].
    pub fn directory(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }

    /// Flag to pass, if any; `debug` has none (cargo's default).
    fn flag(self) -> Option<&'static str> {
        match self {
            Profile::Debug => None,
            Profile::Release => Some("--release"),
        }
    }
}

/// A cargo invocation, run in `root`.
pub struct Cargo {
    command: Command,
    /// Echoed before running, for the author to reuse.
    shown: Vec<String>,
}

impl Cargo {
    /// `cargo <subcommand>` in `root`
    pub fn new(root: &Path, subcommand: &str) -> Self {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let mut command = Command::new(cargo);
        command.current_dir(root).arg(subcommand);

        // `RUSTFLAGS` replaces (not merges with) the project's `.cargo/config.toml`
        // rustflags, which would silently drop `-zstack-size`/`--page-size`.
        command.env_remove("RUSTFLAGS");
        command.env_remove("CARGO_ENCODED_RUSTFLAGS");

        Cargo {
            command,
            shown: vec!["cargo".to_string(), subcommand.to_string()],
        }
    }

    pub fn arg(mut self, arg: impl AsRef<str>) -> Self {
        let arg = arg.as_ref();
        self.command.arg(arg);
        self.shown.push(arg.to_string());
        self
    }

    pub fn args<I: IntoIterator<Item = S>, S: AsRef<str>>(mut self, args: I) -> Self {
        for arg in args {
            self = self.arg(arg);
        }
        self
    }

    pub fn profile(self, profile: Profile) -> Self {
        match profile.flag() {
            Some(flag) => self.arg(flag),
            None => self,
        }
    }

    /// For the child only; not echoed as part of the command.
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.command.env(key, value);
        self
    }

    /// How the invocation reads, for the echo and for an error message.
    pub fn display(&self) -> String {
        self.shown.join(" ")
    }

    /// Runs it, inheriting stdio. `Ok(false)` means cargo ran and failed; `Err` means
    /// it could not be started.
    pub fn run(mut self) -> Result<bool> {
        println!("     Running `{}`", self.display());
        let status = self
            .command
            .status()
            .with_context(|| format!("could not run `{}`", self.display()))?;
        Ok(status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_flags() {
        assert_eq!(Profile::from_debug_flag(false), Profile::Release);
        assert_eq!(Profile::from_debug_flag(true), Profile::Debug);
        assert_eq!(Profile::Release.flag(), Some("--release"));
        assert_eq!(Profile::Debug.flag(), None);
    }

    #[test]
    fn profile_directories_match_cargos() {
        assert_eq!(Profile::Debug.directory(), "debug");
        assert_eq!(Profile::Release.directory(), "release");
    }

    #[test]
    fn display_reflects_full_command() {
        let cargo = Cargo::new(Path::new("."), "build")
            .profile(Profile::Release)
            .arg("--target")
            .arg("wasm32v1-none")
            .args(["--features", "wasm"]);
        assert_eq!(
            cargo.display(),
            "cargo build --release --target wasm32v1-none --features wasm"
        );
    }

    #[test]
    fn env_var_not_echoed() {
        let cargo = Cargo::new(Path::new("."), "test").env("FPRIME_TEST_PROFILE", "release");
        assert_eq!(cargo.display(), "cargo test");
    }
}

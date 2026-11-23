//! The `wasm-opt` every release link runs: binaryen's own build, embedded in this
//! binary by a `fprime-wasm-opt-<platform>` crate and written out to a user cache on
//! first use. Never one from the PATH, so a release optimises the same everywhere.
//!
//! Cargo fetches and checksums that crate like any other, so `cargo install` and the
//! prebuilt `cargo binstall` binary both carry it, and a link needs no network.

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `spacewasm_tools` runs this `wasm-opt` when set; one set by the user wins.
const ENV_WASM_OPT: &str = "SPACEWASM_WASM_OPT";
/// `none` turns optimisation off; nothing to run then.
const ENV_OPT: &str = "SPACEWASM_OPT";
/// Without it, `spacewasm_tools` leaves a module unoptimised when `wasm-opt` won't run.
const ENV_WASM_OPT_REQUIRED: &str = "SPACEWASM_WASM_OPT_REQUIRED";
/// Overrides where the files are written.
const ENV_CACHE: &str = "FPRIME_WASM_CACHE";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use fprime_wasm_opt_aarch64_linux as embedded;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use fprime_wasm_opt_arm64_macos as embedded;
#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
use fprime_wasm_opt_arm64_windows as embedded;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use fprime_wasm_opt_x86_64_linux as embedded;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use fprime_wasm_opt_x86_64_macos as embedded;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
use fprime_wasm_opt_x86_64_windows as embedded;

/// The binaryen release and its files, the first being `wasm-opt`.
type Embedded = Option<(&'static str, &'static [(&'static str, &'static [u8])])>;

/// `None` on a host binaryen does not build for, which must set `SPACEWASM_WASM_OPT`.
/// Every `cfg` here matches one in fprime_wasm/Cargo.toml.
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
))]
const EMBEDDED: Embedded = Some((embedded::BINARYEN, embedded::FILES));
#[cfg(not(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
)))]
const EMBEDDED: Embedded = None;

/// Points the link at the embedded `wasm-opt`, or the user's `SPACEWASM_WASM_OPT`, and
/// makes either one failing to run fail the link.
pub fn select_for_link(args: &[OsString]) -> Result<()> {
    let disabled = std::env::var(ENV_OPT).is_ok_and(|opt| opt.trim() == "none");
    if disabled || !optimized(args) {
        return Ok(());
    }
    let program = match std::env::var_os(ENV_WASM_OPT) {
        Some(program) => PathBuf::from(program),
        None => {
            let (binaryen, files) = EMBEDDED.with_context(|| {
                format!("no wasm-opt is bundled for this host; set {ENV_WASM_OPT} to one")
            })?;
            install(&cache_dir()?.join(binaryen), files)?
        }
    };
    // SAFETY: the linker is single-threaded, and nothing has started a thread yet.
    unsafe {
        std::env::set_var(ENV_WASM_OPT, program);
        std::env::set_var(ENV_WASM_OPT_REQUIRED, "1");
    }
    Ok(())
}

/// `rustc` passes `-O0` for an unoptimised link, which `spacewasm_tools` leaves alone.
fn optimized(args: &[OsString]) -> bool {
    args.iter()
        .rev()
        .find_map(|arg| arg.to_str()?.strip_prefix("-O").map(|level| level != "0"))
        == Some(true)
}

/// Writes `files` under `dir` unless already there, and returns the first.
fn install(dir: &Path, files: &[(&str, &[u8])]) -> Result<PathBuf> {
    let program = dir.join(files[0].0);
    // Only a complete set is ever renamed into place.
    if program.is_file() {
        return Ok(program);
    }

    let parent = dir.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("could not create {}", parent.display()))?;
    // Per process: cargo runs links in parallel, and each may get here.
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    let staging = parent.join(format!(".{name}-{}", std::process::id()));
    let _cleanup = RemoveOnDrop(staging.clone());
    for (path, bytes) in files {
        let target = staging.join(path);
        std::fs::create_dir_all(target.parent().unwrap_or(&staging))?;
        std::fs::write(&target, bytes)
            .with_context(|| format!("could not write {}", target.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
        }
    }

    if let Err(err) = std::fs::rename(&staging, dir) {
        // Another link got there first, with the same files.
        if !program.is_file() {
            return Err(err).with_context(|| format!("could not create {}", dir.display()));
        }
    }
    Ok(program)
}

/// The platform's per-user cache, as `dirs::cache_dir` would find it.
fn cache_dir() -> Result<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    let base = if let Some(dir) = var(ENV_CACHE) {
        return Ok(PathBuf::from(dir));
    } else if cfg!(windows) {
        var("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| Path::new(&home).join("Library/Caches"))
    } else {
        var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|home| Path::new(&home).join(".cache")))
    };
    let base = base.with_context(|| format!("no cache directory; set {ENV_CACHE}"))?;
    Ok(base.join("fprime-wasm").join("binaryen"))
}

struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_optimised_links_need_it() {
        let args = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(optimized(&args(&[
            "-flavor", "wasm", "-o", "a.wasm", "-O2"
        ])));
        assert!(!optimized(&args(&["-o", "a.wasm", "-O0"])));
        assert!(!optimized(&args(&["-o", "a.wasm"])));
    }

    #[test]
    fn installs_once_and_reuses() {
        let root =
            std::env::temp_dir().join(format!("fprime-wasm-opt-test-{}", std::process::id()));
        let _cleanup = RemoveOnDrop(root.clone());
        let dir = root.join("version_0");
        let files: &[(&str, &[u8])] =
            &[("bin/wasm-opt", b"opt"), ("lib/libbinaryen.dylib", b"lib")];

        let program = install(&dir, files).expect("installs");
        assert_eq!(program, dir.join("bin/wasm-opt"));
        assert_eq!(
            std::fs::read(dir.join("lib/libbinaryen.dylib")).unwrap(),
            b"lib"
        );

        // Present already: not rewritten.
        std::fs::write(&program, b"kept").unwrap();
        install(&dir, files).expect("reuses");
        assert_eq!(std::fs::read(&program).unwrap(), b"kept");
        // No staging directory left behind.
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn embeds_wasm_opt_on_every_released_host() {
        let (binaryen, files) = EMBEDDED.expect("a released host");
        assert!(binaryen.starts_with("version_"), "{binaryen}");
        assert!(files[0].0.starts_with("bin/wasm-opt"), "{}", files[0].0);
        assert!(files.iter().all(|(_, bytes)| !bytes.is_empty()));
    }
}

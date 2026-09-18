//! What a sequence or crate may be called.

use anyhow::{Result, bail};
use std::path::Path;

/// Whether `name` works as a Cargo target name, a file stem and a `use` path.
///
/// Cargo accepts more than this; a sequence name has to be unambiguous in all
/// three, so it is held to the intersection.
pub fn valid_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("a name cannot be empty");
    }
    if !name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
    {
        bail!("`{name}` must start with a letter or an underscore");
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
    {
        bail!("`{name}` contains `{bad}`; use letters, digits, `_` and `-` only");
    }
    Ok(())
}

/// Turn a directory name into a usable crate name, so `init` in `my sequences/`
/// need not be told what to call the crate.
pub fn crate_name_from_directory(directory: &Path) -> Option<String> {
    let raw = directory.file_name()?.to_str()?;
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = match cleaned.chars().next() {
        // A leading digit is not a valid crate name; prefixing is friendlier than
        // refusing.
        Some(first) if first.is_ascii_digit() => format!("_{cleaned}"),
        Some(_) => cleaned,
        None => return None,
    };
    valid_name(&cleaned).ok().map(|()| cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_reasonable_names() {
        for name in [
            "startup",
            "safing",
            "de_orbit",
            "phase-2",
            "_internal",
            "s1",
        ] {
            valid_name(name).unwrap_or_else(|err| panic!("{name} should be valid: {err}"));
        }
    }

    #[test]
    fn rejects_names_that_would_not_work_as_files_or_identifiers() {
        for name in ["", "2fast", "-leading", "has space", "has/slash", "dot.rs"] {
            assert!(
                valid_name(name).is_err(),
                "{name:?} should have been rejected"
            );
        }
    }

    #[test]
    fn derives_a_crate_name_from_a_directory() {
        assert_eq!(
            crate_name_from_directory(Path::new("/a/b/ref-sequences")).as_deref(),
            Some("ref-sequences")
        );
        assert_eq!(
            crate_name_from_directory(Path::new("/a/b/my sequences")).as_deref(),
            Some("my_sequences")
        );
        // A leading digit is not a valid crate name.
        assert_eq!(
            crate_name_from_directory(Path::new("/a/b/2024-seqs")).as_deref(),
            Some("_2024-seqs")
        );
        assert_eq!(crate_name_from_directory(Path::new("/")), None);
    }
}

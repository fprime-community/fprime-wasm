//! Sequence profiles: `#[fprime_main(<name>)]` holds a body to the rules in the crate's
//! `profile_<name>.toml`.
//!
//! - `config`: the TOML's shape, one rule per kind of syntax.
//! - `glob`: the name patterns a rule selects commands, telemetry and parameters by.
//! - `check`: walking a body against a profile.
//! - `waive`: `#[rule_waive("why")]`, accepting one node the profile rejects.

mod check;
mod config;
mod glob;
mod waive;

use crate::fprime_test::{file_link, manifest_dir};
use config::Profile;
use proc_macro2::{Ident, TokenStream};
use std::path::Path;

#[cfg(test)]
pub(crate) use check::check;
pub(crate) use waive::strip as strip_waivers;

/// The errors and warnings for `item` under the profile `attr` names, if it names one.
///
/// Emitted beside the item rather than in it, so the item keeps its attributes. Goto on the
/// name opens the profile, and naming the file in the expansion rebuilds the sequence when
/// the profile changes. The item must then go on with [`strip_waivers`] applied.
pub(crate) fn errors(attr: TokenStream, item: &TokenStream) -> syn::Result<TokenStream> {
    let Some(name) = syn::parse2::<Option<Ident>>(attr)? else {
        return match waive::find(item) {
            // Stripped all the same, so it would otherwise be silently ignored.
            Some(span) => Err(syn::Error::new(
                span,
                "`#[rule_waive]` waives a profile's rule, but this function names no profile",
            )),
            None => Ok(TokenStream::new()),
        };
    };

    let manifest = manifest_dir()
        .ok_or_else(|| syn::Error::new(name.span(), "`CARGO_MANIFEST_DIR` is not set"))?;
    let path = manifest.join(format!("profile_{name}.toml"));
    if !path.is_file() {
        return Err(syn::Error::new(
            name.span(),
            format!("no profile `{name}`: expected {}", path.display()),
        ));
    }

    let profile = load(&path).map_err(|err| syn::Error::new(name.span(), err))?;
    let mut out = check::check(&profile, item).into_tokens(&name);
    out.extend(file_link(name.span(), &path.to_string_lossy()));

    Ok(out)
}

pub(crate) fn load(path: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    parse(&text).map_err(|err| format!("{}: {err}", path.display()))
}

pub(crate) fn parse(text: &str) -> Result<Profile, toml::de::Error> {
    toml::from_str(text)
}

//! Generating a sequence project, and adding sequences to one.
//!
//! A sequence crate is fiddly to get right by hand: it targets `wasm32v1-none`,
//! every sequence is a separate `#![no_std]` `#![no_main]` binary with the test and
//! bench harnesses off, `build.rs` generates the dictionary once for the whole
//! crate, and the linker needs a stack size and a one-byte page size so guest
//! memory can be sized in bytes rather than 64 KiB pages.
//!
//! * [`template`] — the files, embedded from `templates/`, and substitution
//! * [`Plan`] — what `init` generates, as values rather than files
//! * [`write()`] and [`add_sequence`] — putting them on disk, never overwriting
//! * [`valid_name`] — what a sequence or crate may be called

mod manifest;
mod name;
mod plan;
pub mod template;
mod write;

#[cfg(test)]
mod fixture;

pub use manifest::DependencySpec;
pub use name::{crate_name_from_directory, valid_name};
pub use plan::{File, Plan, sequence};
pub use write::{Added, Written, add_sequence, write};

/// Stack the linker reserves at the bottom of guest linear memory, in bytes.
///
/// Every byte is memory the deployment has to provide, and sequences spill little
/// — the reference ones peak around 330 bytes — so the default is deliberately
/// tight. `fprime-wasm verify` reports the high-water mark.
pub const DEFAULT_STACK_SIZE: usize = 512;

/// Sequence `init` creates when none is named.
pub const DEFAULT_SEQUENCE: &str = "startup";

/// Where a copied dictionary goes, relative to the crate root.
pub const DICTIONARY_DIR: &str = "dictionary";

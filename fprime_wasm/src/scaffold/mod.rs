//! Generating a sequence project, and adding sequences to one.

mod manifest;
mod name;
mod plan;
pub mod template;
mod write;

#[cfg(test)]
mod fixture;

pub use manifest::DependencySpec;
pub use name::{crate_name_from_directory, valid_name};
pub use plan::{File, Plan, sequence, test};
pub use write::{Added, Written, add_sequence, write};

/// Stack the linker reserves at the bottom of guest linear memory, in bytes.
pub const DEFAULT_STACK_SIZE: usize = 512;

/// Sequence `init` creates when none is named.
pub const DEFAULT_SEQUENCE: &str = "startup";

/// Where a copied dictionary goes, relative to the crate root.
pub const DICTIONARY_DIR: &str = "dictionary";

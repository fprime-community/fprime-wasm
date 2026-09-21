#![no_std]
#![allow(nonstandard_style)]
#![allow(dead_code)]
#![allow(unused_imports)]

include!(concat!(env!("OUT_DIR"), "/dictionary.rs"));

// For #[fprime_test] (not .wasm builds)
#[cfg(not(target_family = "wasm"))]
include!(concat!(env!("OUT_DIR"), "/descriptors.rs"));

pub use Defs::*;
pub use fprime_core::*;

# F Prime Wasm

This repository includes compile-time and runtime-time
dependencies for interfacing Rust with F Prime running
inside a Wasm interpreter.

| Crate | |
|---|---|
| [`fprime-wasm`](fprime_wasm) | The `fprime-wasm` command: scaffold a sequence project, add sequences, size a module against the on-board interpreter |
| [`fprime_core`](fprime_core) | `no_std` runtime a sequence links against, and the guest side of the `fprime_v1` host ABI |
| [`fprime_build`](fprime_build) | `build.rs` code generator, turning an F Prime JSON dictionary into a typed Rust API |
| [`fprime_macros`](fprime_macros) | `#[fprime_main]` and the sequencing DSL |
| [`fprime_dictionary`](fprime_dictionary) | Deserialisation of F Prime JSON dictionaries |

## Installation

1. Install Rust: https://doc.rust-lang.org/cargo/getting-started/installation.html

2. Get the Wasm Rust target:

```shell
rustup target add wasm32v1-none
```

3. Install the tool:

```shell
cargo install fprime-wasm
```

## Starting a project

```shell
mkdir my-sequences && cd my-sequences
fprime-wasm init          # asks for the deployment's JSON dictionary
cargo build --release
fprime-wasm verify
```

See [`fprime_wasm/README.md`](fprime_wasm/README.md) for what `init`, `add` and
`verify` do.

## This repository

An example project that works with the `Ref` F Prime deployment lives in
`crates/example`. To build it:

```shell
cd crates/example
cargo build --release --features wasm
```

The Wasm target comes from `crates/example/.cargo/config.toml`, so `--target` is
not needed. This generates one binary per sequence under
`target/wasm32v1-none/release/`, e.g. `example.wasm` and `no_op.wasm`.

`--features wasm` is an artefact of this repository and nothing a sequence project
needs. A sequence is a `#![no_std] #![no_main]` module with no host entry point,
so it cannot link for the host — and here `crates/example` and `crates/bench` sit
in a workspace whose other members are host tools, so `cargo build --workspace`
would try. Every sequence bin in those two crates is therefore declared
`required-features = ["wasm"]`, which a host build of the workspace leaves alone:
it compiles the two dictionary libraries and skips the bins.

A project scaffolded by `fprime-wasm init` has no such feature and never will.
Its `.cargo/config.toml` sets `build.target`, so *every* build in it — including
`cargo build --workspace` — is already a Wasm build, and there is no host target
for a bin to fail to link for.

Two things to know about the gate. Forgetting the feature is quiet: the build
succeeds and writes no `.wasm`, so if `fprime-wasm verify` reports nothing to verify,
that is the reason. And `--all-features` selects the bins for the host and does
fail, because a Cargo feature cannot be made conditional on the target.

### Adding a sequence

`fprime-wasm add <name>` does this for you. By hand, each sequence is its own bin,
so each one builds to its own `.wasm`:

1. Add `crates/example/src/bin/<name>.rs`:

   ```rust
   #![no_std]
   #![no_main]

   use example::*;

   #[fprime_main]
   pub fn main() {
       CdhCore.cmdDisp.CMD_NO_OP();
   }
   ```

2. Declare it in `crates/example/Cargo.toml` so Cargo does not try to build a test
   harness for a `#![no_main]` bin:

   ```toml
   [[bin]]
   name = "<name>"
   required-features = ["wasm"]
   test = false
   bench = false
   ```

   `required-features` is the workspace gate described above, and `fprime-wasm add`
   does not write it — it has no reason to know about a feature that only exists in
   this repository. Adding it by hand is what the `cargo build --workspace` step in
   CI is there to remind you about.

The dictionary is generated once by `build.rs` and shared through the `example`
library target, so extra sequences do not re-expand it.

### The Wasm feature set

`spacewasm`, the on-board interpreter, implements WebAssembly 1.0 plus
`mutable-globals` and `custom-page-sizes`. Nothing else will load, and the failure
happens when the module is loaded rather than when it is built.

`.cargo/wasm-link` — the release linker wrapper that runs `wasm-opt` — therefore
pins the optimiser to that feature set. It is not enough to compile for
`wasm32v1-none`: `wasm-opt` will *introduce* post-MVP instructions while
optimising even when the compiler emitted none. `fprime-wasm verify` runs in CI to
catch a regression here.

## Releasing

Versions live in one place: `[workspace.package] version` in the root
`Cargo.toml`, which every publishable crate inherits, and which stays at the
`0.0.0` placeholder on `main`. Internal path dependencies carry a matching
`version = "=0.0.0"` so that a published crate depends on exactly its own release.

Pushing a `vX.Y.Z` tag runs `.github/workflows/release.yml`, which rewrites both
placeholders from the tag, runs the tests, builds and checks the sequences,
publishes the five crates to crates.io in dependency order, creates the GitHub
release, and attaches binaries for Linux, macOS and Windows.

Adding a publishable crate means adding it to the ordered `crates` list in that
workflow. A drift guard fails the release if the list and the workspace disagree,
rather than letting it discover the problem half-published.

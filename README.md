# F Prime Wasm

This repository includes compile-time and runtime-time
dependencies for interfacing Rust with F Prime running
inside a Wasm interpreter.

| Crate | |
|---|---|
| [`fprime-wasm`](fprime_wasm) | The `fprime-wasm` command: scaffold a sequence project, add sequences, build and test them, size a module against the on-board interpreter |
| [`fprime_test`](fprime_test) | The `#[fprime_test]` DSL: run a sequence on `spacewasm` and check what it does |
| [`fprime_core`](fprime_core) | `no_std` runtime a sequence links against, and the guest side of the `fprime_v1` host ABI |
| [`fprime_build`](fprime_build) | `build.rs` code generator, turning an F Prime JSON dictionary into a typed Rust API |
| [`fprime_macros`](fprime_macros) | `#[fprime_main]` and the sequencing DSL |
| [`fprime_dictionary`](fprime_dictionary) | Deserialisation of F Prime JSON dictionaries |

## Installation

1. Install Rust: https://doc.rust-lang.org/cargo/getting-started/installation.html

2. Get the Wasm Rust target:

```shell
rustup target add wasm32v1-none
cargo binstall wasm-opt # (optional) for optimizing --release builds
```

3. Install the tool. With
   [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) this
   downloads a prebuilt binary from the
   [releases](https://github.com/fprime-community/fprime-wasm/releases); plain
   `cargo install` always compiles from source, which takes a few minutes:

```shell
cargo binstall fprime-wasm   # prebuilt
cargo install fprime-wasm    # from source
```

Prebuilt binaries are published for Linux (`x86_64`, `aarch64`), macOS (Intel
and Apple silicon) and Windows (`x86_64`). `cargo binstall` falls back to
building from source on anything else.

## Starting a project

```shell
mkdir my-sequences && cd my-sequences
fprime-wasm init          # asks for the deployment's JSON dictionary
fprime-wasm test
fprime-wasm verify
```

See [`fprime_wasm/README.md`](fprime_wasm/README.md) for what `init`, `add`,
`build`, `test` and `verify` do.

[VS Code](https://code.visualstudio.com) is the recommended editor for
a generated sequences project.

## This repository

Besides the six published crates, `crates/` holds what exercises them:

| Crate | |
|---|---|
| [`crates/example`](crates/example) | A small project against the `Ref` F Prime deployment |
| [`crates/bench`](crates/bench) | One sequence per command/telemetry/parameter shape |
| [`crates/wasm_size`](crates/wasm_size) | Measures and compares the size of the sequences `bench` and `example` build |

`example` and `bench` build for `wasm32v1-none` behind a `wasm` feature, so a
host build of the workspace skips their sequence bins:

```shell
cargo test --workspace
cd crates/example
cargo build --release --features wasm
```

`spacewasm`, the on-board interpreter, only implements WebAssembly 1.0 plus
`mutable-globals` and `custom-page-sizes`. `.cargo/wasm-link` pins `wasm-opt` to
that same set, and `fprime-wasm verify` checks the result loads.

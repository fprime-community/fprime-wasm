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
cargo install wasm-opt # (optional) for optimizing --release builds
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

[VS Code](https://code.visualstudio.com) is the recommended editor for
a generated sequences project.

## This repository

Besides the five published crates, `crates/` holds what exercises them:

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
that same set, and `fprime-wasm verify` checks the result.

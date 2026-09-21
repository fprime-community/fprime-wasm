# fprime-wasm

Create and inspect [F Prime](https://github.com/nasa/fprime) Wasm sequence
projects.

```shell
cargo install fprime-wasm
cargo install wasm-opt
```

## `init`

To initialize a fprime-wasm project:

```shell
mkdir ref-sequences && cd ref-sequences
fprime-wasm init
```

Scaffolds a sequence crate with all the supporting boilerplate.

[VS Code](https://code.visualstudio.com) is the recommended editor for
a generated sequences project.

## `add`

To add a new sequence:

```shell
fprime-wasm add safing
```

Creates `src/bin/safing.rs` and `tests/safing.rs`.

## `build`

```shell
fprime-wasm build
```

Compiles the sequences to `target/wasm32v1-none/release/*.wasm`. The only place
the Wasm target and the `wasm` feature are named, so a sequence author types
neither.

Both are needed together, and this is why `[build] target` is *not* in the
scaffolded `.cargo/config.toml`: it would apply to `cargo test` too, and a
`wasm32v1-none` test binary cannot be built at all because libtest needs `std`.
Leaving the host as the default target is what lets a sequence project have
ordinary host tests. `--debug` builds the unoptimised modules.

## `test`

```shell
fprime-wasm test [FILTER]
```

Builds the sequences, then runs the tests in `tests/`. Each test runs a compiled
sequence on the `spacewasm` interpreter and checks the conversation it has with
the spacecraft — the commands it sends, the responses it gets, the telemetry it
reads. `--no-build` uses the modules already on disk; `--debug` tests the debug
build.

The second half is a plain `cargo test`, so `cargo test` and an editor's test
runner work directly too.

## `verify`

```shell
fprime-wasm verify
```

Builds the sequences, then loads each module into a
[`spacewasm`](https://github.com/nasa/spacewasm) interpreter and sizes it
against `sequencer.toml` limits. `--no-build` uses the modules already on disk.

```
Limits (sequencer.toml): memory 8192 B, heap 8 pages, code 256 pages, operand stack 1024 words, page 8192 B

  Module                   Bytes  Memory  Heap  Code  Fits
  -----------------------  -----  ------  ----  ----  ----
  cmd_no_args                122     516     2     1  ok
  example                    641     941     2     2  ok
  mixed_max                 1956    1004     2     6  ok
  tlm_scalar_x8              973     788     2     3  ok

All 4 modules fit.
```

Nothing is executed: `verify` decodes the module, links it against the
`fprime_v1` host interface, instantiates it and stops before the first
instruction — everything the on-board interpreter checks at load time. That is
what makes it a cheap CI gate: exit status is non-zero if a module will not
load or overruns a budget, and no sequence has to be driven anywhere to find
out.

Two settings can only be sized by running a sequence, so they are not here:
`stackSize` (the operand stack's depth) and the guest stack. `fprime-wasm test`
measures both, along with what the sequence actually does.

`--verbose` expands each module: every budget's utilisation, and the guest and
interpreter figures.

### JSON

```shell
fprime-wasm verify --json | jq '.modules | max_by(.guest.declared_bytes) | .path'
```

`--json` reports the same run as one document on stdout, diagnostics on
stderr: `{schema, limits, modules, errors, summary}`, one entry per module
under `modules` with the same figures the tables show. `limits` echoes
`sequencer.toml` whole, `stackSize` included — it is the configuration, not a
measurement.

## `sequencer.toml`

`init` writes one at the crate root, holding the limits `verify` sizes against
and `test` runs under. Edit it to match the deployment that will fly the
sequences:

```toml
# Per WasmSequencer component instance (Svc::WasmSequencer::Config)
[config]
heap_pages = 8            # heapPages
guest_memory = 8192       # guestMemorySize, bytes
stack_size = 1024         # stackSize, 32-bit words
max_code_pages = 256      # maxCodePages
max_guest_modules = 8     # maxGuestModules

# Build-time configuration (set across deployment/project)
[constants]
page_size = 8192          # WASM_SEQ_SPACEWASM_PAGE_SIZE
event_message_max = 128   # Wasm.GUEST_EVENT_MESSAGE_SIZE
serial_ports = 5          # Wasm.MAX_SERIAL_IN_PORTS / MAX_SERIAL_OUT_PORTS

# `fprime-wasm test` settings: only a run uses these, since `verify` does not execute
[test]
max_instructions = 10000000
stack_sample = 1
```

`--limits <path>` measures against a different file, for trying a
configuration out without editing the tracked one. A deployment with more than
one `WasmSequencer` instance wants a file per instance; a test names the one it
is about with `#[fprime_test(sequence = "safing", limits = "sequencer-payload.toml")]`.

## License

Apache-2.0

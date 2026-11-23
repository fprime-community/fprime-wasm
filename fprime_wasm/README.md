# fprime-wasm

Create, build, test and check [F Prime](https://github.com/nasa/fprime) Wasm
sequence projects.

```shell
cargo binstall fprime-wasm   # or `cargo install fprime-wasm` to build from source
```

## `init`

```shell
mkdir ref-sequences && cd ref-sequences
fprime-wasm init --fprime-version 4.4.0
```

Creates a sequence project and asks for the deployment's JSON dictionary.

`--fprime-version` is the F Prime release the sequences will fly on. Each release
runs a particular version of the on-board interpreter, and the project is built
for exactly that interpreter. Leave it out to target the newest release.

| F Prime | Interpreter (spacewasm) |
| ------- | ----------------------- |
| 4.4.x   | 0.7.1                   |
| 4.5.0+  | 0.8.0                   |

The version is stored as `FPRIME_VERSION` in `.cargo/config.toml`. To move an
existing project to a different F Prime release, the simplest route is to run
`init --fprime-version` in an empty directory and copy its
`.cargo/config.toml` over.

[VS Code](https://code.visualstudio.com) is the recommended editor.

## `add`

```shell
fprime-wasm add safing
```

Creates the sequence `src/bin/safing.rs` and its tests, `tests/safing.rs`.

## `build`

```shell
fprime-wasm build
```

Compiles every sequence to `target/wasm32v1-none/release/<sequence>.wasm`.
`--debug` builds without optimisation.

Each `.wasm` comes with a `<sequence>.sym`. **Uplink the `.wasm` and keep the
`.sym` on the ground**: it is what `fprime-wasm backtrace` uses to turn a position
in the uplinked sequence back into a source line. A `.sym` only matches the
`.wasm` built with it, so archive the two together.

If the build uses something the target interpreter does not support, it fails
and names the instruction and the fix.

## `test`

```shell
fprime-wasm test [FILTER]
```

Builds the sequences, then runs the tests in `tests/`. Each test runs a compiled
sequence on the same interpreter used on board, and checks what it does: the
commands it sends, the responses it gets, the telemetry it reads.

`--no-build` tests the sequences already built; `--debug` tests the debug build.
`cargo test`, and an editor's test runner, work too.

## `verify`

```shell
fprime-wasm verify
```

Builds the sequences, then loads each one as the on-board `WasmSequencer` would
and checks it fits the deployment's limits in `sequencer.toml`. It is the
ground equivalent of the `LOAD` command.

```
Limits (sequencer.toml): F Prime 4.5.0 (spacewasm 0.8.0), memory 8192 B, heap 8 pages, code 256 pages, operand stack 1024 words, page 8192 B

  Module                   Bytes  Memory  Heap  Code  Fits
  -----------------------  -----  ------  ----  ----  ----
  cmd_no_args                122     516     2     1  ok
  example                    641     941     2     2  ok
  mixed_max                 1956    1004     2     6  ok
  tlm_scalar_x8              973     788     2     3  ok

All 4 modules fit.
```

Nothing is run, so it is quick and suits CI: the exit status is non-zero if a
sequence will not load or is over a limit. The operand stack depth and the guest
stack are only known by running a sequence, which `fprime-wasm test` does.

- `--no-build` checks the sequences already built.
- `--verbose` shows every limit for each sequence.
- `--json` prints the same report as JSON, for scripts:

  ```shell
  fprime-wasm verify --json | jq '.modules | max_by(.guest.declared_bytes) | .path'
  ```

## `backtrace`

```shell
fprime-wasm backtrace example --pc 0x20,0x10
```

Turns a position in a sequence that flew into the source lines it came from,
including inlined callers:

```
#0 0xac:
  fprime_core::tlm::telemetry
      at fprime_core/src/tlm.rs:14:9
  <example::Tlm::CdhCoreEvents>::EventsDropped
      at target/wasm32v1-none/release/build/example-…/out/dictionary.rs:561:68
  main
      at crates/example/src/bin/example.rs:16:47
#1 0x98:
  main
      at crates/example/src/bin/example.rs:11:12
```

- `--pc` takes the program counters the on-board interpreter reports,
  innermost first. Needs F Prime 4.5.0 or later on board.
- `--offset` takes byte offsets into the `.wasm` instead.

Name the sequence to use the current build, or give the path to an archived
`.sym`. `--module` checks the `.wasm` that actually flew against it, so a
mismatched pair is refused instead of pointing at the wrong lines.

## `sequencer.toml`

The limits `verify` checks against and `test` runs under. `init` writes the
`WasmSequencer` defaults; change them to match the deployment:

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

# `fprime-wasm test` only
[test]
max_instructions = 10000000   # stop a sequence that loops forever
stack_sample = 1
```

A deployment with more than one `WasmSequencer` instance can keep a file per
instance. `verify --limits <file>` checks against one, and a test picks one with
`#[fprime_test(sequence = "safing", limits = "sequencer-payload.toml")]`.

## License

Apache-2.0

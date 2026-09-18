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

Creates `src/bin/safing.rs`.

## `verify`

```shell
cargo build --release
fprime-wasm verify
```

Loads each module into a [`spacewasm`](https://github.com/nasa/spacewasm)
interpreter and validates it against `sequencer.toml` limits:

```
Limits (sequencer.toml): memory 8192 B, heap 8 pages, code 256 pages, operand stack 1024 words, page 8192 B

  Module                   Bytes  Memory  Stack  Heap  Code  Operand  Status
  -----------------------  -----  ------  -----  ----  ----  -------  ------
  cmd_no_args                122     516      -     2     1        4  ok
  example                    641     941      -     2     2       15  ok
  mixed_max                 1956    1004    328     2     6       30  ok
  tlm_scalar_x8              973     788    148     2     3       32  ok

All 4 modules fit.
```

Exit status is non-zero if a module overruns a budget or fails to run, so it
works as a CI gate. `--verbose` expands each module: every budget's
utilisation, the guest and interpreter figures, and the commands, channels and
parameters it touched.

### Driving a sequence down a branch

Telemetry and parameters read as zero by default. To choose another:

```shell
fprime-wasm verify --tlm 16781312=0003 --prm 4=01 --args deadbeef --trace
```

`--trace` prints every host call the sequence made. With a dictionary in the
project (or `--dictionary`), commands, channels and parameters are named
rather than numbered.

### JSON

```shell
fprime-wasm verify --json | jq '.modules | max_by(.guest.memory_bytes) | .path'
```

`--json` reports the same run as one document on stdout, diagnostics on
stderr: `{schema, limits, modules, errors, summary}`, one entry per module
under `modules` with the same figures the tables show.

## `sequencer.toml`

`init` writes one at the crate root, holding the limits `verify` measures
against. Edit it to match the deployment that will fly the sequences:

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

# fprime-wasm verify settings
[verify]
max_instructions = 10000000
stack_sample = 1
```

`--limits <path>` measures against a different file, for trying a
configuration out without editing the tracked one.

## License

Apache-2.0

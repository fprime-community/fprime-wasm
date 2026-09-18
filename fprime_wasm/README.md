# fprime-wasm

Create and inspect [F Prime](https://github.com/nasa/fprime) Wasm sequence
projects.

```shell
cargo install fprime-wasm
```

## `init`

```shell
mkdir ref-sequences && cd ref-sequences
fprime-wasm init
```

Fills the directory with a sequence crate: the `wasm32v1-none` target, the linker
arguments that make guest memory sizeable in bytes, a `build.rs` that turns the
deployment's dictionary into a typed Rust API, and one starter sequence. If no
dictionary is given with `--dictionary`, and the project does not already have
one, it asks for the path.

## `add`

```shell
fprime-wasm add safing
```

Writes `src/bin/safing.rs` and declares the `[[bin]]` it needs. Each sequence is
its own binary so each links to its own `.wasm`, and each must set
`test = false` and `bench = false` — Cargo otherwise tries to build a test
harness for a `#![no_main]` binary and the build fails. Both halves are
idempotent, and neither overwrites a file you have edited.

## `verify`

```shell
cargo build --release
fprime-wasm verify
```

Loads each module into a [`spacewasm`](https://github.com/nasa/spacewasm)
interpreter — the same one `Svc::WasmSequencer` embeds — registers the same
`fprime_v1` host interface, runs it, and reports what it needed:

```
Limits: memory 8192 B, heap 8 pages, code 256 pages, operand stack 1024 words, page 8192 B

  Module                   Bytes  Memory  Stack  Heap  Code  Operand  Status
  -----------------------  -----  ------  -----  ----  ----  -------  ------
  cmd_no_args                122     516      -     2     1        4  ok
  example                    641     941      -     2     2       15  ok
  mixed_max                 1956    1004    328     2     6       30  ok
  tlm_scalar_x8              973     788    148     2     3       32  ok

All 4 modules fit.
```

One row per module. The limits are the same for every module in a run, so they are
stated once on the first line rather than repeated as `941/8192` on every row.
`Status` is `ok`, `OVER`, or how the sequence ended if it did not finish; a `-`
under `Stack` means the module declares no stack pointer at all, which is not the
same as declaring one and using none of it.

Every column names something you configure, so the table is what you size the
component with. The defaults are the on-board defaults, so plain `verify` answers
"does this fit a stock sequencer?"; pass `--heap-pages`, `--guest-memory`,
`--page-size` and so on to measure against your deployment's. The exit status is
non-zero if a module overruns a budget or does not run, which makes it usable as a
CI gate.

`--verbose` expands each module: every budget with its utilisation, the guest and
interpreter figures, and the commands, channels and parameters it touched.

Two of those numbers are worth explaining:

- **`PAGE_SIZE`** is the largest *single* allocation the interpreter made. A page
  is the largest contiguous block the on-board allocator can serve, so an
  allocation bigger than one page fails however many pages there are — a separate
  problem from running out of them.
- **Guest stack** is measured by filling the linker's stack region with a poison
  byte before the run and seeing how far in the sequence wrote. That region is
  `[0, __stack_pointer)`: `wasm32v1-none` links stack-first, so it is pure scratch
  and nothing that expects to be zero lives there.

`verify` also catches what a build cannot. `spacewasm` implements WebAssembly 1.0
plus `mutable-globals` and `custom-page-sizes`; anything else is rejected when the
module loads, not when it compiles. `wasm-opt` will introduce post-MVP
instructions while optimising even when the compiler emitted none, so a module can
build clean and fail on board. When that happens `verify` names the instruction, the
proposal and the flag that keeps it out.

### Driving a sequence down a branch

Telemetry and parameters read as zero by default, so a sequence that branches on
them takes the zero path. To choose another:

```shell
fprime-wasm verify --tlm 16781312=0003 --prm 4=01 --args deadbeef --trace
```

`--trace` prints every host call the sequence made — commands with their opcodes,
events with their severities and messages, sleeps, serial traffic. With a
dictionary in the project (or `--dictionary`), commands, channels and parameters
are named rather than numbered.

### JSON

`--json` reports the same run as one JSON document on stdout instead of tables.
Diagnostics go to stderr, so it pipes cleanly:

```shell
fprime-wasm verify --json | jq '.modules | max_by(.guest.memory_bytes) | .path'
```

The document is `{schema, limits, modules, errors, summary}`. `schema` is an
integer that changes when the shape does. Each module carries the same figures the
tables show — `budgets`, `page_size`, `guest`, `interpreter`, `commands`,
`telemetry_read`, `parameters_read`, `notes` — plus `passed` and, when it did not,
`failures`.

Three things worth knowing about the schema:

- **`errors` is separate from `modules`.** An input that could not be decoded has no
  measurements, so it is listed there rather than as a module with zeroes. It still
  counts in `summary.failed`, so the summary always agrees with the exit status.
- **`guest.stack` is `null`** when the module declares no stack pointer, rather than
  a zeroed object. Anything sizing `-zstack-size` has to tell those apart.
- **`calls` is only present with `--trace`.** A long sequence's trace dwarfs the
  rest of the report, so it is opt-in; when absent the key is omitted rather than
  null.

## License

Apache-2.0

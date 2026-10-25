# fprime_macros

Procedural macros for [fprime-wasm] sequences.

- `#[fprime_main]` marks a sequence's entry point. It exports the function to the
  Wasm module as `main` — the name `Svc::WasmSequencer` looks for — and installs
  the `#[panic_handler]` that reports a guest panic through the `fprime_v1` host
  ABI.
- `#[fprime]` enables the sequencing DSL on a function without exporting it, so
  bare enum variants (`ACTIVITY_HI`, `BLUE`) and struct literals (`SeqArgs { .. }`)
  resolve against the dictionary instead of needing a fully qualified path. As much
  of the dictionary's own qualifier as reads well may still be written out
  (`ProcType::PROC_TYPE_NONE`, `Svc::SeqArgs { .. }`).

These are re-exported by `fprime_core`, so a sequence crate does not normally
depend on this crate directly.

```rust
#![no_std]
#![no_main]

use my_deployment::*;

#[fprime_main]
pub fn main() {
    CdhCore.cmdDisp.CMD_NO_OP();
    CdhCore.events.SET_EVENT_FILTER(ACTIVITY_HI, DISABLED);
}
```

[fprime-wasm]: https://github.com/fprime-community/fprime-wasm

## License

Apache-2.0

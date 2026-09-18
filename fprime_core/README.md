# fprime_core

The `no_std` runtime an [fprime-wasm] sequence links against.

It provides the guest half of the `fprime_v1` host ABI that
[`Svc::WasmSequencer`](https://github.com/nasa/fprime) implements: dispatching
commands, reading telemetry channels and parameters, emitting events, sleeping,
and the serial ports — plus the serialisation, panic reporting and command
failure-mode handling around them.

You do not call this ABI directly. `fprime_build` generates a typed API for your
deployment's dictionary on top of it, and `fprime_core` re-exports the macros that
make that API usable:

```rust
#![no_std]
#![no_main]

use my_deployment::*;

#[fprime_main]
pub fn main() {
    set_fail_mode(FailMode::Checked);
    CdhCore.cmdDisp.CMD_NO_OP();
}
```

This crate only builds for a Wasm target. Start a project with
`cargo install fprime-wasm && fprime-wasm init`, which wires up the target, the
dictionary and the release profile for you.

[fprime-wasm]: https://github.com/fprime-community/fprime-wasm

## License

Apache-2.0

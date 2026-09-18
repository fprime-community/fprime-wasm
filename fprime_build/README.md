# fprime_build

Turns an [F Prime](https://github.com/nasa/fprime) JSON dictionary into the Rust
API an [fprime-wasm] sequence writes against.

Call it from a sequence crate's `build.rs`:

```rust
pub fn main() {
    fprime_build::generate("dictionary/RefTopologyDictionary.json");
}
```

It writes `$OUT_DIR/dictionary.rs`, which the crate's `lib.rs` includes:

```rust
#![no_std]
include!(concat!(env!("OUT_DIR"), "/dictionary.rs"));
pub use Defs::*;
pub use fprime_core::*;
```

From there every command, telemetry channel and parameter in the deployment is a
typed call — `CdhCore.cmdDisp.CMD_NO_OP()` — and the dictionary's enums, arrays
and structs are Rust types. The generated module is emitted once per crate and
shared by every sequence binary in it, so adding a sequence does not re-expand it.

`fprime-wasm init` sets all of this up for you.

[fprime-wasm]: https://github.com/fprime-community/fprime-wasm

## License

Apache-2.0

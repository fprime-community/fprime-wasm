# fprime_dictionary

Deserialisation of [F Prime](https://github.com/nasa/fprime) JSON dictionaries.

This is the shared data model underneath the rest of [fprime-wasm]: the commands,
telemetry channels, parameters, constants and type definitions of a deployment,
read straight from the dictionary its build emits. `fprime_build` turns that model
into Rust, and the `fprime-wasm` tool uses it to validate a project's dictionary.

Most users do not depend on this crate directly. Depend on `fprime_build` from a
sequence crate's `build.rs`, or install the `fprime-wasm` tool.

```rust
let dictionary = fprime_dictionary::try_parse(std::path::Path::new("RefTopologyDictionary.json"))?;
for command in &dictionary.commands {
    println!("{} = {:#x}", command.name, command.opcode);
}
```

[fprime-wasm]: https://github.com/fprime-community/fprime-wasm

## License

Apache-2.0

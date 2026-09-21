# fprime_test

Test [F Prime](https://github.com/nasa/fprime) Wasm sequences: run one on the
same interpreter that flies, and check what it does.

A sequence project scaffolded by
[`fprime-wasm`](https://crates.io/crates/fprime-wasm) already depends on this.

```toml
[dev-dependencies]
fprime_test = "0.1"
```

## What a test says

A test describes the conversation a sequence has with the spacecraft — the
commands it sends, the responses it gets back, the telemetry it reads:

```rust,ignore
use sequences::*;

#[fprime_test(sequence = "safing")]
fn retries_power_off_once(t: Test) {
    t.given(Ref.power.BatteryVoltage, 21.5);

    t.expect(Ref.power.PWR_OFF()).responds(EXECUTION_ERROR);
    t.expect(Ref.power.PWR_OFF()).responds(OK);
    t.expect_exit(0);
}
```

Commands, telemetry channels and parameters are named the way the deployment's
dictionary names them, and the way a sequence names them — so a test reads like
the sequence it is about, and a misspelling is a compile error rather than a
surprise in flight.

The sequence name is a link: ask your editor to go to the definition of
`"safing"` and it opens `src/bin/safing.rs`, the sequence the test is about.

The steps you list must happen **in the order you list them**, but you do not
have to list everything: a sequence may do other things in between. Commands you
say nothing about succeed.

You can say what the spacecraft reports (`given`, `given_each`, `given_param`,
`given_serial`), what it answers (`responds`), and what changes because the
sequence got somewhere (`sets`, `queues_serial`). Steps cover everything a
sequence can do: commands, telemetry and parameter reads, events, sleeps, serial
traffic in both directions, and the clock.

## A sequence that never ends

Some sequences loop, waiting for work:

```rust,ignore
#[fprime_test(sequence = "worker")]
fn handles_a_request(t: Test) {
    t.given_serial_value(0, 100u32);

    t.expect(serial_recv(0).blocking().finding_a_message());
    t.expect(CdhCore.cmdDisp.CMD_NO_OP());
    t.expect(serial_send(0));

    // No exit: it went back to waiting for the next message.
    t.expect_still_running();
}
```

There is no exit to expect, and none is needed. Once the sequence has read
everything you queued, its next receive finds an empty port and the run ends
there — at the end of a complete iteration, which is where you want to look.

A sequence that *polls* instead of blocking never reaches such a point. For one
of those, add `t.stop_after_last_step()` and the run ends once every step you
listed has matched. Note what that means for `never`: the run stops early, so a
`never` rule only covers the sequence up to that point.

## Which sequencer it runs on

By default, the `sequencer.toml` at the crate root — the deployment the sequences are for. A
deployment that configures more than one `WasmSequencer` instance needs a file per instance, and
a test names the one it is about:

```rust,ignore
#[fprime_test(sequence = "safing", limits = "sequencer-payload.toml")]
fn safing_fits_the_payload_sequencer(t: Test) {
```

The path is relative to the crate root, next to the default one. It has to be there: a name
that is not a file is a compile error on the string itself, not a surprise when the test runs.
Goto-definition on it opens the file, the same way it does on the sequence name.

## Running

```shell
fprime-wasm test
```

Builds the sequences, then runs the tests. `cargo test` works too — the tests are
ordinary Rust integration tests — and so does an editor's test runner.

## What it actually runs

The compiled `.wasm`, on [`spacewasm`](https://github.com/nasa/spacewasm), the
interpreter `Svc::WasmSequencer` embeds, with the same memory and instruction
limits `sequencer.toml` configures. So a passing test is a statement about the
module that would be uplinked, not about its Rust source.

## License

Apache-2.0

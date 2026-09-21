//! Which `sequencer.toml` a test runs against.
//!
//! This crate has none, so a test without `limits = "..."` runs against the stock defaults.
//! `sequencer-short-events.toml` stands in for a second `WasmSequencer` instance in the same
//! deployment, and `worker` behaves differently on it.

use example::*;
use fprime_test::*;

/// `worker` emits "handled a request" — 17 bytes, which the stock 128-byte cap keeps whole.
#[fprime_test(sequence = "worker")]
fn the_default_cap_keeps_the_whole_message(t: Test) {
    t.initial_serial(0, 1u32);

    t.expect_serial_recv(serial_recv(0).blocking().finding_a_message());
    t.expect_event_exactly("handled a request");
    t.expect_still_running();
}

/// The same sequence, on the instance that caps events at 8 bytes: the sequencer truncates the
/// message on the way out and the guest is never told. Only the attribute makes this happen.
#[fprime_test(sequence = "worker", limits = "sequencer-short-events.toml")]
fn a_shorter_cap_truncates_the_message(t: Test) {
    t.initial_serial(0, 1u32);

    t.expect_serial_recv(serial_recv(0).blocking().finding_a_message());
    t.expect_event_exactly("handled ")
        .because("this instance is configured for 8-byte event messages");
    t.expect_still_running();
}

/// `t.limits(..)` is the more specific of the two, so it wins over the attribute — here it
/// puts the stock cap back and the whole message survives.
#[fprime_test(sequence = "worker", limits = "sequencer-short-events.toml")]
fn limits_in_the_body_win_over_the_attribute(t: Test) {
    t.limits(Limits::default());
    t.initial_serial(0, 1u32);

    t.expect_serial_recv(serial_recv(0).blocking().finding_a_message());
    t.expect_event_exactly("handled a request");
    t.expect_still_running();
}

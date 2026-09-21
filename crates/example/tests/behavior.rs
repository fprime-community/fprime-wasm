//! `#[fprime_test]` cases run against the real `no_op.wasm` and `example.wasm`.
//!
//! `fprime-wasm test` builds the modules first; a plain `cargo test` needs
//! `fprime-wasm build` to have run.

use example::*;
use fprime_test::*;

/// Two commands, in order, and a clean end.
#[fprime_test(sequence = "no_op")]
fn no_op_sends_commands_in_order(t: Test) {
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP());
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP_STRING("hello from no_op.wasm"));
    t.expect_ok();
}

/// A bare command matches the opcode with any arguments.
#[fprime_test(sequence = "no_op")]
fn bare_command_matches_any_args(t: Test) {
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP);
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP_STRING);
    t.expect_ok();
}

/// Matches a subsequence, not the full transcript.
#[fprime_test(sequence = "example")]
fn subsequence_selects_color_after_load(t: Test) {
    t.expect_command(Ref.wasmSeq.LOAD("helloworld"));
    t.expect_command(Ref.dpDemo.SelectColor);
    t.expect_ok();
}

/// `never_command` is checked over the whole run.
#[fprime_test(sequence = "no_op")]
fn no_op_touches_only_dispatcher(t: Test) {
    t.never_command(Ref.wasmSeq.LOAD);
    t.never_command(Ref.dpDemo.SelectColor);
    t.expect_ok();
}

/// `Checked` mode: a refusal ends the sequence with `PanicCode::CmdFailed`.
#[fprime_test(sequence = "no_op")]
fn refused_command_stops_checked_sequence(t: Test) {
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP())
        .responds(Desc::Response::EXECUTION_ERROR)
        .because("a checked sequence must not carry on past a rejected command");
    t.never_command(CdhCore.cmdDisp.CMD_NO_OP_STRING);
    // 2 is PanicCode::CmdFailed.
    t.expect_panic(2);
}

/// Sends the drop report only when `EventsDropped` is above 2.
#[fprime_test(sequence = "example")]
fn reports_dropped_events_above_two(t: Test) {
    t.initial_telemetry(CdhCore.events.EventsDropped, 5);
    t.expect_telemetry_read(CdhCore.events.EventsDropped);
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP_STRING("DROPPED 2"));
    t.expect_ok();
}

/// The other branch: no report when nothing was dropped.
#[fprime_test(sequence = "example")]
fn silent_when_no_events_dropped(t: Test) {
    t.initial_telemetry(CdhCore.events.EventsDropped, 0);
    t.never_command(CdhCore.cmdDisp.CMD_NO_OP_STRING("DROPPED 2"));
    t.expect_ok();
}

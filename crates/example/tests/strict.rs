//! `#[fprime_test]` cases for the `strict` sequence.

use example::*;
use fprime_test::*;

/// Above the limit it reports the drops and selects red.
#[fprime_test(sequence = "strict")]
fn reports_drops_above_the_limit(t: Test) {
    t.initial_telemetry(CdhCore.events.EventsDropped, 3);
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP_STRING("TOO MANY DROPPED"));
    t.expect_command(Ref.dpDemo.SelectColor(RED));
    t.expect_ok();
}

/// At the limit it stays quiet and selects green.
#[fprime_test(sequence = "strict")]
fn selects_green_at_the_limit(t: Test) {
    t.initial_telemetry(CdhCore.events.EventsDropped, 2);
    t.never_command(CdhCore.cmdDisp.CMD_NO_OP_STRING);
    t.expect_command(Ref.dpDemo.SelectColor(GREEN));
    t.expect_ok();
}

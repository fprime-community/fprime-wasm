//! Tests for the `worker` and `poller` long-running sequences.

use example::*;
use fprime_test::*;

/// Exercises the whole host ABI surface once.
#[fprime_test(sequence = "worker")]
fn handles_one_request_then_waits(t: Test) {
    t.initial_serial(0, 100u32);
    t.initial_telemetry(CdhCore.events.EventsDropped, 7);
    t.initial_parameter(Ref.wasmSeq.INSTRUCTION_FUEL, 20);

    t.expect_serial_recv(serial_recv(0).blocking().finding_a_message());
    t.expect_time_read();
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP());
    t.expect_telemetry_read(CdhCore.events.EventsDropped);
    t.expect_parameter_read(Ref.wasmSeq.INSTRUCTION_FUEL);
    t.expect_event_containing("handled a request");
    t.expect_sleep(sleep().of_secs(1.0).relative());
    // request + dropped + fuel + elapsed. Elapsed is 1: the `rsleep` falls between the two
    // clock reads, so 100 + 7 + 20 + 1 = 128.
    t.expect_serial_send(serial_send(0).of(128u32));

    // No exit: it went back to waiting for the next message.
    t.expect_still_running();
}

/// Proves the loop, not just one pass.
#[fprime_test(sequence = "worker")]
fn handles_requests_in_turn(t: Test) {
    t.initial_serial(0, 1u32);
    t.initial_serial(0, 2u32);

    // Nothing supplied for the channel or the parameter, so both read zero; each reply is its
    // request plus the one second of elapsed time.
    t.expect_serial_send(serial_send(0).of(2u32));
    t.expect_serial_recv(serial_recv(0).finding_a_message());
    t.expect_serial_send(serial_send(0).of(3u32));
    // The third receive finds nothing, and that is where it stops.
    t.expect_serial_recv(serial_recv(0).finding_nothing());

    t.expect_still_running();
}

/// A reply can queue the next request.
#[fprime_test(sequence = "worker")]
fn reply_queues_next_request(t: Test) {
    t.initial_serial(0, 10u32);

    t.expect_serial_send(serial_send(0).of(11u32))
        .queues_serial(0, 20u32)
        .because("the ground answers the first reply with another request");
    t.expect_serial_send(serial_send(0).of(21u32));

    t.expect_still_running();
}

/// `Permissive` mode: refusal is reported, not fatal.
#[fprime_test(sequence = "worker")]
fn reports_refused_command(t: Test) {
    t.initial_serial(0, 1u32);

    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP())
        .responds(Desc::Response::EXECUTION_ERROR);
    t.expect_event_containing("refused");
    // It still replies, and still goes back to waiting.
    t.expect_serial_send(serial_send(0));

    t.expect_still_running();
}

/// The clock must reflect the sequence's own sleeps, not stay frozen.
#[fprime_test(sequence = "worker")]
fn clock_advances_with_sleeps(t: Test) {
    t.initial_serial(0, 0u32);
    t.initial_serial(0, 0u32);

    t.expect_sleep(sleep().of_secs(1.0));
    // 0 + 0 + 0 + 1 second of elapsed time.
    t.expect_serial_send(serial_send(0).of(1u32));
    // And the second iteration measures its own second, not a running total.
    t.expect_serial_send(serial_send(0).of(1u32));

    t.expect_still_running();
}

/// The `never_serial_send` here is load-bearing — without the stop armed, `worker` would
/// still reply.
#[fprime_test(sequence = "worker")]
fn stop_after_last_step_ends_run(t: Test) {
    t.initial_serial(0, 1u32);

    t.expect_serial_recv(serial_recv(0).blocking().finding_a_message());
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP());

    // `worker` always replies. So this holds only because the run was cut before it could.
    t.never_serial_send(serial_send(0));

    t.stop_after_last_step();
    t.expect_still_running();
}

/// Polling never parks, so the test has to call the stop itself.
#[fprime_test(sequence = "poller")]
fn polling_sequence_stopped_by_test(t: Test) {
    // It finds nothing, sleeps, and the request arrives while it is asleep.
    t.expect_serial_recv(serial_recv(1).polling().finding_nothing())
        .queues_serial(1, 5u32)
        .because("the ground sends a request only once the poller is up");
    t.expect_sleep(sleep().of_us(100_000));
    t.expect_serial_recv(serial_recv(1).polling().finding_a_message());
    t.expect_command(CdhCore.cmdDisp.CMD_NO_OP());
    t.expect_serial_send(serial_send(1).of(5u32));

    t.stop_after_last_step();
    t.expect_still_running();
}

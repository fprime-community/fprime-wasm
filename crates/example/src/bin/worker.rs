//! Waits for work, handles it, replies, and loops.

#![no_std]
#![no_main]

use example::*;

#[fprime_main]
pub fn main() {
    // Permissive: a refused command is something this sequence reports rather than dies of.
    set_fail_mode(FailMode::Permissive);

    let inbox = serial_in!(u32, 0);
    let outbox = serial_out!(u32, 0);

    loop {
        // Parks here when there is nothing to do, which is where a test's run ends.
        let request = inbox.recv_block();

        let started = now();
        let response = CdhCore.cmdDisp.CMD_NO_OP();

        let (dropped, _) = CdhCore.events.EventsDropped();
        let fuel = Ref.wasmSeq.INSTRUCTION_FUEL();

        if response == CmdResponse::Ok {
            message(EventSeverity::ActivityHigh, "handled a request");
        } else {
            message(EventSeverity::WarningHi, "the dispatcher refused a no-op");
        }

        rsleep(1_000_000);

        // The reply carries something from every read, so a wrong value is visible on the wire.
        let elapsed = now().seconds - started.seconds;
        outbox.send(request + dropped as u32 + fuel as u32 + elapsed);
    }
}

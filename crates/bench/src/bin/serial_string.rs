//! A string-typed port: the variable-length path, where the wire carries a length
//! prefix and the buffer is sized for the longest string the port allows.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let inbox = serial_in!(String<40>, 0);
    let outbox = serial_out!(String<40>, 0);

    let request = inbox.recv_block();
    if !request.is_empty() {
        outbox.send(request);
    }
}

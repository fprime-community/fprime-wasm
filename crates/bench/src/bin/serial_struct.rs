//! A structure-typed port: deserializing and reserializing a request
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let inbox = serial_in!(Ref::ScalarStruct, 0);
    let outbox = serial_out!(Ref::ScalarStruct, 0);

    let mut request = inbox.recv_block();
    request.u32 += 1;
    outbox.send(request);
}

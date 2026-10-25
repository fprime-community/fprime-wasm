//! A structure with a member that is not a constant.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    let color = Tlm::Ref.typeDemo.ChoiceCh().0;
    Ref.typeDemo.CHOICE_PAIR(ChoicePair {
        firstChoice: color,
        secondChoice: BLUE,
    });
}

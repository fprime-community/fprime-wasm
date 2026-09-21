//! A structure with scalars either side of it in the argument list.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    Ref.typeDemo.CHOICE_PAIR_WITH_FRIENDS(
        1,
        ChoicePair {
            firstChoice: RED,
            secondChoice: BLUE,
        },
        2,
    );
}

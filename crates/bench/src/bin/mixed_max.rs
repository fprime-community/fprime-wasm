//! Everything at once: const-encoded scalars, strings, arrays and structures, a
//! telemetry read, a parameter read, and both fallback shapes. The worst case a
//! real sequence is likely to reach.
#![no_std]
#![no_main]

use bench::*;

#[fprime_main]
pub fn main() {
    CdhCore.cmdDisp.CMD_NO_OP();
    CdhCore.cmdDisp.CMD_NO_OP_STRING("mixed maximum");
    CdhCore.events.SET_EVENT_FILTER(ACTIVITY_HI, DISABLED);
    CdhCore.health.HLTH_PING_ENABLE("pinger", ENABLED);

    Ref.typeDemo.SEND_INTS(1, -2, 3, -4, 5, -6, 7, -8);
    Ref.typeDemo.SEND_FLOATS(1.5, -2.5);
    Ref.typeDemo.SEND_BOOL(true);
    Ref.typeDemo.SEND_WIDE_CHOICE(WIDE_HIGH);
    Ref.typeDemo.CHOICES([ONE, TWO]);
    Ref.typeDemo.SEND_2D([[ONE, TWO], [TWO, ONE]]);
    Ref.typeDemo.GLUTTON_OF_CHOICE(ChoiceSlurry {
        tooManyChoices: [[BLUE, RED], [TWO, TWO]],
        choiceAsMemberArray: [2, 3],
        choicePair: ChoicePair {
            firstChoice: RED,
            secondChoice: BLUE,
        },
        separateChoice: ONE,
    });

    let (dropped, _) = CdhCore.events.EventsDropped();
    if dropped > 2 && Ref.recvBuffComp.parameter1() > 0 {
        CdhCore.cmdDisp.CMD_NO_OP_STRING("dropped");
    }

    Ref.dpDemo.Dp(IMMEDIATE, dropped as u32, PROC_TYPE_NONE);
    Ref.typeDemo.SEND_NAMES([
        StrTruncate::truncate("first"),
        StrTruncate::truncate("second"),
    ]);

    Ref.wasmSeq.LOAD("finished");
    Ref.dpDemo.SelectColor(GREEN);
}

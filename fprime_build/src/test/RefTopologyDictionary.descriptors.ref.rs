pub mod Desc {
    #[allow(unused_imports)]
    use fprime_core::*;
    pub mod CdhCore {
        #[allow(unused_imports)]
        use fprime_core::*;
        pub mod cmdDisp {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// No-op command
            pub const CMD_NO_OP: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000000 as u32,
                path: "CdhCore.cmdDisp.CMD_NO_OP",
            };
            /// No-op command with a string argument
            pub const CMD_NO_OP_STRING: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000001 as u32,
                path: "CdhCore.cmdDisp.CMD_NO_OP_STRING",
            };
            /// A command with a signed, a float and an unsigned argument
            pub const CMD_TEST_CMD_1: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000002 as u32,
                path: "CdhCore.cmdDisp.CMD_TEST_CMD_1",
            };
            /// Commands dispatched
            pub const CommandsDispatched: fprime_core::desc::Chan<u32> = fprime_core::desc::Chan::new(
                0x1000000 as i64,
                "CdhCore.cmdDisp.CommandsDispatched",
                "U32",
            );
        }
        pub mod events {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Set the event filter
            pub const SET_EVENT_FILTER: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1001000 as u32,
                path: "CdhCore.events.SET_EVENT_FILTER",
            };
            /// Events dropped, typed through an alias chain
            pub const EventsDropped: fprime_core::desc::Chan<crate::Defs::FwSizeType> = fprime_core::desc::Chan::new(
                0x1001000 as i64,
                "CdhCore.events.EventsDropped",
                "FwSizeType",
            );
        }
        pub mod health {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Enable or disable pinging of one entry
            pub const HLTH_PING_ENABLE: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1002001 as u32,
                path: "CdhCore.health.HLTH_PING_ENABLE",
            };
        }
    }
    pub mod Ref {
        #[allow(unused_imports)]
        use fprime_core::*;
        pub mod SG1 {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Request a data product from the signal generator
            pub const Dp: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10011003 as u32,
                path: "Ref.SG1.Dp",
            };
        }
        pub mod dpDemo {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Select a colour
            pub const SelectColor: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0xA10 as u32,
                path: "Ref.dpDemo.SelectColor",
            };
            /// Request a data product
            pub const Dp: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0xA11 as u32,
                path: "Ref.dpDemo.Dp",
            };
        }
        pub mod recvBuffComp {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// A u32 parameter
            pub const parameter1: fprime_core::desc::Prm<u32> = fprime_core::desc::Prm::new(
                0x10022000 as i64,
                "Ref.recvBuffComp.parameter1",
                "U32",
            );
        }
        pub mod typeDemo {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// A single enumeration
            pub const CHOICE: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005000 as u32,
                path: "Ref.typeDemo.CHOICE",
            };
            /// An array of enumerations
            pub const CHOICES: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005003 as u32,
                path: "Ref.typeDemo.CHOICES",
            };
            /// A structure of enumerations
            pub const CHOICE_PAIR: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000500B as u32,
                path: "Ref.typeDemo.CHOICE_PAIR",
            };
            /// A structure with scalars either side of it
            pub const CHOICE_PAIR_WITH_FRIENDS: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000500C as u32,
                path: "Ref.typeDemo.CHOICE_PAIR_WITH_FRIENDS",
            };
            /// A nested structure, the deepest shape in the dictionary
            pub const GLUTTON_OF_CHOICE: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000500F as u32,
                path: "Ref.typeDemo.GLUTTON_OF_CHOICE",
            };
            /// Every scalar width, inside a structure
            pub const SEND_SCALARS: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005015 as u32,
                path: "Ref.typeDemo.SEND_SCALARS",
            };
            /// Every integer width as a separate argument
            pub const SEND_INTS: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005016 as u32,
                path: "Ref.typeDemo.SEND_INTS",
            };
            /// Both float widths
            pub const SEND_FLOATS: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005017 as u32,
                path: "Ref.typeDemo.SEND_FLOATS",
            };
            /// A boolean argument
            pub const SEND_BOOL: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005018 as u32,
                path: "Ref.typeDemo.SEND_BOOL",
            };
            /// An argument whose type is an alias
            pub const SEND_ALIAS: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005019 as u32,
                path: "Ref.typeDemo.SEND_ALIAS",
            };
            /// An enumeration that needs an i64 on the wire
            pub const SEND_WIDE_CHOICE: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000501A as u32,
                path: "Ref.typeDemo.SEND_WIDE_CHOICE",
            };
            /// A string short enough to make truncation easy to hit
            pub const SEND_TINY_STRING: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000501B as u32,
                path: "Ref.typeDemo.SEND_TINY_STRING",
            };
            /// An array of arrays
            pub const SEND_2D: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000501C as u32,
                path: "Ref.typeDemo.SEND_2D",
            };
            /// An array of scalars
            pub const SEND_U32_ARRAY: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000501D as u32,
                path: "Ref.typeDemo.SEND_U32_ARRAY",
            };
            /// An array of booleans
            pub const SEND_BOOL_ARRAY: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000501E as u32,
                path: "Ref.typeDemo.SEND_BOOL_ARRAY",
            };
            /// An array of floats
            pub const SEND_FLOAT_ARRAY: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x1000501F as u32,
                path: "Ref.typeDemo.SEND_FLOAT_ARRAY",
            };
            /// An array of enumerations of a different width
            pub const SEND_COLOR_ARRAY: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005020 as u32,
                path: "Ref.typeDemo.SEND_COLOR_ARRAY",
            };
            /// An array of strings, which cannot be encoded at compile time
            pub const SEND_NAMES: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005021 as u32,
                path: "Ref.typeDemo.SEND_NAMES",
            };
            /// A structure with a string member, which cannot be encoded at compile time
            pub const SEND_COLOR_INFO: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10005022 as u32,
                path: "Ref.typeDemo.SEND_COLOR_INFO",
            };
            /// An enumeration channel
            pub const ChoiceCh: fprime_core::desc::Chan<crate::Defs::Ref::Choice> = fprime_core::desc::Chan::new(
                0x10005000 as i64,
                "Ref.typeDemo.ChoiceCh",
                "Ref.Choice",
            );
            /// An array channel
            pub const ChoicesCh: fprime_core::desc::Chan<
                crate::Defs::Ref::ManyChoices,
            > = fprime_core::desc::Chan::new(
                0x10005001 as i64,
                "Ref.typeDemo.ChoicesCh",
                "Ref.ManyChoices",
            );
            /// A string channel
            pub const NameCh: fprime_core::desc::Chan<fprime_core::String<40>> = fprime_core::desc::Chan::new(
                0x10005002 as i64,
                "Ref.typeDemo.NameCh",
                "string size 40",
            );
            /// A structure channel
            pub const ScalarStructCh: fprime_core::desc::Chan<
                crate::Defs::Ref::ScalarStruct,
            > = fprime_core::desc::Chan::new(
                0x10005009 as i64,
                "Ref.typeDemo.ScalarStructCh",
                "Ref.ScalarStruct",
            );
            /// A u8 channel
            pub const ScalarU8Ch: fprime_core::desc::Chan<u8> = fprime_core::desc::Chan::new(
                0x1000500A as i64,
                "Ref.typeDemo.ScalarU8Ch",
                "U8",
            );
            /// A u16 channel
            pub const ScalarU16Ch: fprime_core::desc::Chan<u16> = fprime_core::desc::Chan::new(
                0x1000500B as i64,
                "Ref.typeDemo.ScalarU16Ch",
                "U16",
            );
            /// A u32 channel
            pub const ScalarU32Ch: fprime_core::desc::Chan<u32> = fprime_core::desc::Chan::new(
                0x1000500C as i64,
                "Ref.typeDemo.ScalarU32Ch",
                "U32",
            );
            /// A u64 channel
            pub const ScalarU64Ch: fprime_core::desc::Chan<u64> = fprime_core::desc::Chan::new(
                0x1000500D as i64,
                "Ref.typeDemo.ScalarU64Ch",
                "U64",
            );
            /// An i8 channel
            pub const ScalarI8Ch: fprime_core::desc::Chan<i8> = fprime_core::desc::Chan::new(
                0x1000500E as i64,
                "Ref.typeDemo.ScalarI8Ch",
                "I8",
            );
            /// An i16 channel
            pub const ScalarI16Ch: fprime_core::desc::Chan<i16> = fprime_core::desc::Chan::new(
                0x1000500F as i64,
                "Ref.typeDemo.ScalarI16Ch",
                "I16",
            );
            /// An i32 channel
            pub const ScalarI32Ch: fprime_core::desc::Chan<i32> = fprime_core::desc::Chan::new(
                0x10005010 as i64,
                "Ref.typeDemo.ScalarI32Ch",
                "I32",
            );
            /// An i64 channel
            pub const ScalarI64Ch: fprime_core::desc::Chan<i64> = fprime_core::desc::Chan::new(
                0x10005011 as i64,
                "Ref.typeDemo.ScalarI64Ch",
                "I64",
            );
            /// An f32 channel
            pub const ScalarF32Ch: fprime_core::desc::Chan<f32> = fprime_core::desc::Chan::new(
                0x10005012 as i64,
                "Ref.typeDemo.ScalarF32Ch",
                "F32",
            );
            /// An f64 channel
            pub const ScalarF64Ch: fprime_core::desc::Chan<f64> = fprime_core::desc::Chan::new(
                0x10005013 as i64,
                "Ref.typeDemo.ScalarF64Ch",
                "F64",
            );
            /// An enumeration parameter
            pub const CHOICE_PRM: fprime_core::desc::Prm<crate::Defs::Ref::Choice> = fprime_core::desc::Prm::new(
                0x10005000 as i64,
                "Ref.typeDemo.CHOICE_PRM",
                "Ref.Choice",
            );
        }
        pub mod wasmSeq {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Load a sequence
            pub const LOAD: fprime_core::desc::CmdDesc = fprime_core::desc::CmdDesc {
                opcode: 0x10007002 as u32,
                path: "Ref.wasmSeq.LOAD",
            };
            /// A u64 parameter
            pub const INSTRUCTION_FUEL: fprime_core::desc::Prm<u64> = fprime_core::desc::Prm::new(
                0x10007001 as i64,
                "Ref.wasmSeq.INSTRUCTION_FUEL",
                "U64",
            );
        }
    }
    pub mod Response {
        #[allow(unused_imports)]
        use fprime_core::*;
        pub const OK: fprime_core::desc::Response = fprime_core::desc::Response::new(
            0,
            "OK",
        );
        pub const INVALID_OPCODE: fprime_core::desc::Response = fprime_core::desc::Response::new(
            1,
            "INVALID_OPCODE",
        );
        pub const VALIDATION_ERROR: fprime_core::desc::Response = fprime_core::desc::Response::new(
            2,
            "VALIDATION_ERROR",
        );
        pub const FORMAT_ERROR: fprime_core::desc::Response = fprime_core::desc::Response::new(
            3,
            "FORMAT_ERROR",
        );
        pub const EXECUTION_ERROR: fprime_core::desc::Response = fprime_core::desc::Response::new(
            4,
            "EXECUTION_ERROR",
        );
        pub const BUSY: fprime_core::desc::Response = fprime_core::desc::Response::new(
            5,
            "BUSY",
        );
    }
}

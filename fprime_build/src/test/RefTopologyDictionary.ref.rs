pub mod Defs {
    #[allow(unused_imports)]
    use fprime_core::*;
    /// The id type.
    pub type FwIdType = u32;
    /// The type of a command opcode
    pub type FwOpcodeType = crate::Defs::FwIdType;
    /// The unsigned type of larger sizes internal to the software.
    pub type FwSizeType = crate::Defs::PlatformSizeType;
    /// The type used to serialize a time context value
    pub type FwTimeContextStoreType = u8;
    /// The unsigned type of larger sizes internal to the software.
    pub type PlatformSizeType = u64;
    /// Time base for a time stamp
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
    #[repr(u16)]
    pub enum TimeBase {
        TB_NONE = 0,
        TB_PROC_TIME = 1,
        TB_WORKSTATION_TIME = 2,
        TB_SC_TIME = 3,
    }
    pub mod Fw {
        #[allow(unused_imports)]
        use fprime_core::*;
        /// The status of a command
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
        #[repr(u8)]
        pub enum CmdResponse {
            OK = 0,
            INVALID_OPCODE = 1,
            VALIDATION_ERROR = 2,
            FORMAT_ERROR = 3,
            EXECUTION_ERROR = 4,
            BUSY = 5,
        }
        /// Enabled or not
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
        #[repr(u8)]
        pub enum Enabled {
            DISABLED = 0,
            ENABLED = 1,
        }
        /// A time stamp
        #[derive(Clone, Debug, PartialEq, Serializable)]
        pub struct TimeValue {
            pub timeBase: crate::Defs::TimeBase,
            pub timeContext: crate::Defs::FwTimeContextStoreType,
            pub seconds: u32,
            pub useconds: u32,
        }
        pub mod DpCfg {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Data product processing type
            #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
            #[repr(u8)]
            pub enum ProcType {
                PROC_TYPE_NONE = 0,
                PROC_TYPE_ZLIB_DEFLATE = 1,
                PROC_TYPE_ONE = 2,
                PROC_TYPE_TWO = 4,
            }
        }
    }
    pub mod Ref {
        #[allow(unused_imports)]
        use fprime_core::*;
        /// An alias of a scalar
        pub type AliasedU32 = u32;
        /// An enumeration with an i32 representation
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
        #[repr(i32)]
        pub enum Choice {
            ONE = 0,
            TWO = 1,
            RED = 2,
            BLUE = 3,
        }
        /// A structure of enumerations
        #[derive(Clone, Debug, PartialEq, Serializable)]
        pub struct ChoicePair {
            pub firstChoice: crate::Defs::Ref::Choice,
            pub secondChoice: crate::Defs::Ref::Choice,
        }
        /// A structure with a nested array, a nested structure and a member array
        #[derive(Clone, Debug, PartialEq, Serializable)]
        pub struct ChoiceSlurry {
            pub tooManyChoices: crate::Defs::Ref::TooManyChoices,
            pub separateChoice: crate::Defs::Ref::Choice,
            pub choicePair: crate::Defs::Ref::ChoicePair,
            pub choiceAsMemberArray: [u8; 2],
        }
        /// An array of enumerations
        pub type ManyChoices = [crate::Defs::Ref::Choice; 2];
        /// Every scalar width in one structure
        #[derive(Clone, Debug, PartialEq, Serializable)]
        pub struct ScalarStruct {
            pub i8: i8,
            pub i16: i16,
            pub i32: i32,
            pub i64: i64,
            pub u8: u8,
            pub u16: u16,
            pub u32: u32,
            pub u64: u64,
            pub f32: f32,
            pub f64: f64,
        }
        /// An array of arrays
        pub type TooManyChoices = [crate::Defs::Ref::ManyChoices; 2];
        /// An enumeration wide enough to need an i64 on the wire
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
        #[repr(i64)]
        pub enum WideChoice {
            WIDE_LOW = -4294967296,
            WIDE_ZERO = 0,
            WIDE_HIGH = 4294967296,
        }
        /// An integer constant
        pub const dimension: u32 = 3;
        pub mod DpDemo {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// An array of booleans
            pub type BooleanArray = [bool; 2];
            /// A colour
            #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
            #[repr(i32)]
            pub enum ColorEnum {
                RED = 0,
                GREEN = 1,
                BLUE = 2,
            }
            /// A structure with a string member
            #[derive(Clone, Debug, PartialEq, Serializable)]
            pub struct ColorInfoStruct {
                pub color: crate::Defs::Ref::DpDemo::ColorEnum,
                pub name: String<40>,
            }
            /// How a data product is requested
            #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
            #[repr(i32)]
            pub enum DpReqType {
                IMMEDIATE = 0,
                ASYNC = 1,
            }
            /// An array of enumerations
            pub type EnumArray = [crate::Defs::Ref::DpDemo::ColorEnum; 3];
            /// An array of floats
            pub type F32Array = [f32; 3];
            /// An array of strings
            pub type StringArray = [String<80>; 2];
            /// An array of scalars
            pub type U32Array = [u32; 5];
            /// Another integer constant
            pub const stringSize: u32 = 80;
        }
        pub mod SignalGen {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Declared a second time, so a bare constant only resolves via its receiver
            #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
            #[repr(i32)]
            pub enum DpReqType {
                IMMEDIATE = 0,
                ASYNC = 1,
            }
        }
    }
    pub mod Svc {
        #[allow(unused_imports)]
        use fprime_core::*;
        pub mod EventManager {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Enabled or not, with the opposite ordering to Fw.Enabled
            #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
            #[repr(u8)]
            pub enum Enabled {
                ENABLED = 0,
                DISABLED = 1,
            }
            /// Severity of an event filter
            #[derive(Clone, Copy, Debug, Eq, PartialEq, Serializable)]
            #[repr(u8)]
            pub enum FilterSeverity {
                WARNING_HI = 0,
                WARNING_LO = 1,
                COMMAND = 2,
                ACTIVITY_HI = 3,
                ACTIVITY_LO = 4,
                DIAGNOSTIC = 5,
            }
        }
        pub mod Fpy {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// A string constant
            pub const DEFAULT_SEQ_BASE_DIR: &'static str = "/seq";
        }
    }
}
mod Impl {
    #[allow(unused_imports)]
    use fprime_core::*;
    #[allow(unused_imports)]
    use super::Defs::FwOpcodeType;
    const __SCRATCH_SIZE: usize = 252usize;
    static mut __SCRATCH: [u8; __SCRATCH_SIZE] = [0x0; __SCRATCH_SIZE];
    const __TIME_SIZE: usize = crate::Defs::Fw::TimeValue::SIZE;
    static mut __TIME: [u8; __TIME_SIZE] = [0; __TIME_SIZE];
    pub struct CdhCoreCmdDisp {}
    impl CdhCoreCmdDisp {
        pub const DEFAULT: Self = Self {};
        /// No-op command
        #[fprime_command(opcode = 0x1000000)]
        pub fn CMD_NO_OP(&self) -> fprime_core::CmdResponse {}
        /// No-op command with a string argument
        ///
        ///  * `arg1`
        #[fprime_command(opcode = 0x1000001)]
        pub fn CMD_NO_OP_STRING(&self, arg1: String<40>) -> fprime_core::CmdResponse {}
        /// A command with a signed, a float and an unsigned argument
        ///
        ///  * `arg1`
        ///  * `arg2`
        ///  * `arg3`
        #[fprime_command(opcode = 0x1000002)]
        pub fn CMD_TEST_CMD_1(
            &self,
            arg1: i32,
            arg2: f32,
            arg3: u8,
        ) -> fprime_core::CmdResponse {}
        /// Commands dispatched
        #[fprime_telemetry(id = 0x1000000)]
        pub fn CommandsDispatched(&self) -> (u32, super::Defs::Fw::TimeValue) {}
    }
    pub struct CdhCoreEvents {}
    impl CdhCoreEvents {
        pub const DEFAULT: Self = Self {};
        /// Set the event filter
        ///
        ///  * `filterLevel`
        ///  * `filterEnabled`
        #[fprime_command(opcode = 0x1001000)]
        pub fn SET_EVENT_FILTER(
            &self,
            filterLevel: crate::Defs::Svc::EventManager::FilterSeverity,
            filterEnabled: crate::Defs::Svc::EventManager::Enabled,
        ) -> fprime_core::CmdResponse {}
        /// Events dropped, typed through an alias chain
        #[fprime_telemetry(id = 0x1001000)]
        pub fn EventsDropped(
            &self,
        ) -> (crate::Defs::FwSizeType, super::Defs::Fw::TimeValue) {}
    }
    pub struct CdhCoreHealth {}
    impl CdhCoreHealth {
        pub const DEFAULT: Self = Self {};
        /// Enable or disable pinging of one entry
        ///
        ///  * `entry`
        ///  * `enable`
        #[fprime_command(opcode = 0x1002001)]
        pub fn HLTH_PING_ENABLE(
            &self,
            entry: String<40>,
            enable: crate::Defs::Fw::Enabled,
        ) -> fprime_core::CmdResponse {}
    }
    pub struct CdhCore {
        pub cmdDisp: CdhCoreCmdDisp,
        pub events: CdhCoreEvents,
        pub health: CdhCoreHealth,
    }
    impl CdhCore {
        pub const DEFAULT: Self = Self {
            cmdDisp: CdhCoreCmdDisp::DEFAULT,
            events: CdhCoreEvents::DEFAULT,
            health: CdhCoreHealth::DEFAULT,
        };
    }
    pub struct RefSg1 {}
    impl RefSg1 {
        pub const DEFAULT: Self = Self {};
        /// Request a data product from the signal generator
        ///
        ///  * `reqType`
        ///  * `records`
        ///  * `priority`
        #[fprime_command(opcode = 0x10011003)]
        pub fn Dp(
            &self,
            reqType: crate::Defs::Ref::SignalGen::DpReqType,
            records: u32,
            priority: u32,
        ) -> fprime_core::CmdResponse {}
    }
    pub struct RefDpDemo {}
    impl RefDpDemo {
        pub const DEFAULT: Self = Self {};
        /// Select a colour
        ///
        ///  * `color`
        #[fprime_command(opcode = 0xA10)]
        pub fn SelectColor(
            &self,
            color: crate::Defs::Ref::DpDemo::ColorEnum,
        ) -> fprime_core::CmdResponse {}
        /// Request a data product
        ///
        ///  * `reqType`
        ///  * `priority`
        ///  * `proc`
        #[fprime_command(opcode = 0xA11)]
        pub fn Dp(
            &self,
            reqType: crate::Defs::Ref::DpDemo::DpReqType,
            priority: u32,
            proc: crate::Defs::Fw::DpCfg::ProcType,
        ) -> fprime_core::CmdResponse {}
    }
    pub struct RefRecvBuffComp {}
    impl RefRecvBuffComp {
        pub const DEFAULT: Self = Self {};
        /// A u32 parameter
        #[fprime_parameter(id = 0x10022000)]
        pub fn parameter1(&self) -> u32 {}
    }
    pub struct RefTypeDemo {}
    impl RefTypeDemo {
        pub const DEFAULT: Self = Self {};
        /// A single enumeration
        ///
        ///  * `choice`
        #[fprime_command(opcode = 0x10005000)]
        pub fn CHOICE(
            &self,
            choice: crate::Defs::Ref::Choice,
        ) -> fprime_core::CmdResponse {}
        /// An array of enumerations
        ///
        ///  * `choices`
        #[fprime_command(opcode = 0x10005003)]
        pub fn CHOICES(
            &self,
            choices: crate::Defs::Ref::ManyChoices,
        ) -> fprime_core::CmdResponse {}
        /// A structure of enumerations
        ///
        ///  * `choices`
        #[fprime_command(opcode = 0x1000500B)]
        pub fn CHOICE_PAIR(
            &self,
            choices: crate::Defs::Ref::ChoicePair,
        ) -> fprime_core::CmdResponse {}
        /// A structure with scalars either side of it
        ///
        ///  * `repeat`
        ///  * `choices`
        ///  * `repeat_max`
        #[fprime_command(opcode = 0x1000500C)]
        pub fn CHOICE_PAIR_WITH_FRIENDS(
            &self,
            repeat: u8,
            choices: crate::Defs::Ref::ChoicePair,
            repeat_max: u8,
        ) -> fprime_core::CmdResponse {}
        /// A nested structure, the deepest shape in the dictionary
        ///
        ///  * `choices`
        #[fprime_command(opcode = 0x1000500F)]
        pub fn GLUTTON_OF_CHOICE(
            &self,
            choices: crate::Defs::Ref::ChoiceSlurry,
        ) -> fprime_core::CmdResponse {}
        /// Every scalar width, inside a structure
        ///
        ///  * `scalar_input`
        #[fprime_command(opcode = 0x10005015)]
        pub fn SEND_SCALARS(
            &self,
            scalar_input: crate::Defs::Ref::ScalarStruct,
        ) -> fprime_core::CmdResponse {}
        /// Every integer width as a separate argument
        ///
        ///  * `a`
        ///  * `b`
        ///  * `c`
        ///  * `d`
        ///  * `e`
        ///  * `f`
        ///  * `g`
        ///  * `h`
        #[fprime_command(opcode = 0x10005016)]
        pub fn SEND_INTS(
            &self,
            a: u8,
            b: i8,
            c: u16,
            d: i16,
            e: u32,
            f: i32,
            g: u64,
            h: i64,
        ) -> fprime_core::CmdResponse {}
        /// Both float widths
        ///
        ///  * `single`
        ///  * `double`
        #[fprime_command(opcode = 0x10005017)]
        pub fn SEND_FLOATS(&self, single: f32, double: f64) -> fprime_core::CmdResponse {}
        /// A boolean argument
        ///
        ///  * `flag`
        #[fprime_command(opcode = 0x10005018)]
        pub fn SEND_BOOL(&self, flag: bool) -> fprime_core::CmdResponse {}
        /// An argument whose type is an alias
        ///
        ///  * `aliased`
        #[fprime_command(opcode = 0x10005019)]
        pub fn SEND_ALIAS(
            &self,
            aliased: crate::Defs::Ref::AliasedU32,
        ) -> fprime_core::CmdResponse {}
        /// An enumeration that needs an i64 on the wire
        ///
        ///  * `choice`
        #[fprime_command(opcode = 0x1000501A)]
        pub fn SEND_WIDE_CHOICE(
            &self,
            choice: crate::Defs::Ref::WideChoice,
        ) -> fprime_core::CmdResponse {}
        /// A string short enough to make truncation easy to hit
        ///
        ///  * `tiny`
        #[fprime_command(opcode = 0x1000501B)]
        pub fn SEND_TINY_STRING(&self, tiny: String<8>) -> fprime_core::CmdResponse {}
        /// An array of arrays
        ///
        ///  * `choices`
        #[fprime_command(opcode = 0x1000501C)]
        pub fn SEND_2D(
            &self,
            choices: crate::Defs::Ref::TooManyChoices,
        ) -> fprime_core::CmdResponse {}
        /// An array of scalars
        ///
        ///  * `values`
        #[fprime_command(opcode = 0x1000501D)]
        pub fn SEND_U32_ARRAY(
            &self,
            values: crate::Defs::Ref::DpDemo::U32Array,
        ) -> fprime_core::CmdResponse {}
        /// An array of booleans
        ///
        ///  * `flags`
        #[fprime_command(opcode = 0x1000501E)]
        pub fn SEND_BOOL_ARRAY(
            &self,
            flags: crate::Defs::Ref::DpDemo::BooleanArray,
        ) -> fprime_core::CmdResponse {}
        /// An array of floats
        ///
        ///  * `values`
        #[fprime_command(opcode = 0x1000501F)]
        pub fn SEND_FLOAT_ARRAY(
            &self,
            values: crate::Defs::Ref::DpDemo::F32Array,
        ) -> fprime_core::CmdResponse {}
        /// An array of enumerations of a different width
        ///
        ///  * `colors`
        #[fprime_command(opcode = 0x10005020)]
        pub fn SEND_COLOR_ARRAY(
            &self,
            colors: crate::Defs::Ref::DpDemo::EnumArray,
        ) -> fprime_core::CmdResponse {}
        /// An array of strings, which cannot be encoded at compile time
        ///
        ///  * `names`
        #[fprime_command(opcode = 0x10005021)]
        pub fn SEND_NAMES(
            &self,
            r#names: crate::Defs::Ref::DpDemo::StringArray,
        ) -> fprime_core::CmdResponse {}
        /// A structure with a string member, which cannot be encoded at compile time
        ///
        ///  * `info`
        #[fprime_command(opcode = 0x10005022)]
        pub fn SEND_COLOR_INFO(
            &self,
            info: crate::Defs::Ref::DpDemo::ColorInfoStruct,
        ) -> fprime_core::CmdResponse {}
        /// An enumeration channel
        #[fprime_telemetry(id = 0x10005000)]
        pub fn ChoiceCh(
            &self,
        ) -> (crate::Defs::Ref::Choice, super::Defs::Fw::TimeValue) {}
        /// An array channel
        #[fprime_telemetry(id = 0x10005001)]
        pub fn ChoicesCh(
            &self,
        ) -> (crate::Defs::Ref::ManyChoices, super::Defs::Fw::TimeValue) {}
        /// A string channel
        #[fprime_telemetry(id = 0x10005002)]
        pub fn NameCh(&self) -> (String<40>, super::Defs::Fw::TimeValue) {}
        /// A structure channel
        #[fprime_telemetry(id = 0x10005009)]
        pub fn ScalarStructCh(
            &self,
        ) -> (crate::Defs::Ref::ScalarStruct, super::Defs::Fw::TimeValue) {}
        /// A u8 channel
        #[fprime_telemetry(id = 0x1000500A)]
        pub fn ScalarU8Ch(&self) -> (u8, super::Defs::Fw::TimeValue) {}
        /// A u16 channel
        #[fprime_telemetry(id = 0x1000500B)]
        pub fn ScalarU16Ch(&self) -> (u16, super::Defs::Fw::TimeValue) {}
        /// A u32 channel
        #[fprime_telemetry(id = 0x1000500C)]
        pub fn ScalarU32Ch(&self) -> (u32, super::Defs::Fw::TimeValue) {}
        /// A u64 channel
        #[fprime_telemetry(id = 0x1000500D)]
        pub fn ScalarU64Ch(&self) -> (u64, super::Defs::Fw::TimeValue) {}
        /// An i8 channel
        #[fprime_telemetry(id = 0x1000500E)]
        pub fn ScalarI8Ch(&self) -> (i8, super::Defs::Fw::TimeValue) {}
        /// An i16 channel
        #[fprime_telemetry(id = 0x1000500F)]
        pub fn ScalarI16Ch(&self) -> (i16, super::Defs::Fw::TimeValue) {}
        /// An i32 channel
        #[fprime_telemetry(id = 0x10005010)]
        pub fn ScalarI32Ch(&self) -> (i32, super::Defs::Fw::TimeValue) {}
        /// An i64 channel
        #[fprime_telemetry(id = 0x10005011)]
        pub fn ScalarI64Ch(&self) -> (i64, super::Defs::Fw::TimeValue) {}
        /// An f32 channel
        #[fprime_telemetry(id = 0x10005012)]
        pub fn ScalarF32Ch(&self) -> (f32, super::Defs::Fw::TimeValue) {}
        /// An f64 channel
        #[fprime_telemetry(id = 0x10005013)]
        pub fn ScalarF64Ch(&self) -> (f64, super::Defs::Fw::TimeValue) {}
        /// An enumeration parameter
        #[fprime_parameter(id = 0x10005000)]
        pub fn CHOICE_PRM(&self) -> crate::Defs::Ref::Choice {}
    }
    pub struct RefWasmSeq {}
    impl RefWasmSeq {
        pub const DEFAULT: Self = Self {};
        /// Load a sequence
        ///
        ///  * `fileName`
        #[fprime_command(opcode = 0x10007002)]
        pub fn LOAD(&self, fileName: String<240>) -> fprime_core::CmdResponse {}
        /// A u64 parameter
        #[fprime_parameter(id = 0x10007001)]
        pub fn INSTRUCTION_FUEL(&self) -> u64 {}
    }
    pub struct Ref {
        pub SG1: RefSg1,
        pub dpDemo: RefDpDemo,
        pub recvBuffComp: RefRecvBuffComp,
        pub typeDemo: RefTypeDemo,
        pub wasmSeq: RefWasmSeq,
    }
    impl Ref {
        pub const DEFAULT: Self = Self {
            SG1: RefSg1::DEFAULT,
            dpDemo: RefDpDemo::DEFAULT,
            recvBuffComp: RefRecvBuffComp::DEFAULT,
            typeDemo: RefTypeDemo::DEFAULT,
            wasmSeq: RefWasmSeq::DEFAULT,
        };
    }
}
pub const CdhCore: Impl::CdhCore = Impl::CdhCore::DEFAULT;
pub const Ref: Impl::Ref = Impl::Ref::DEFAULT;
pub fn now() -> crate::Defs::Fw::TimeValue {
    use fprime_core::Serializable;
    let mut buf: [u8; crate::Defs::Fw::TimeValue::SIZE] = [0; crate::Defs::Fw::TimeValue::SIZE];
    unsafe {
        fprime_core::time::time_read(&mut buf);
    }
    crate::Defs::Fw::TimeValue::deserialize(&buf)
}
impl PartialOrd for crate::Defs::Fw::TimeValue {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.timeBase != other.timeBase {
            fprime_core::panic(fprime_core::PanicCode::TimeBaseIncomparable);
        } else if self.timeContext != other.timeContext {
            fprime_core::panic(fprime_core::PanicCode::TimeContextIncomparable);
        }
        match self.seconds.partial_cmp(&other.seconds) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.useconds.partial_cmp(&other.useconds)
    }
}
pub mod Konst {
    #[allow(unused_imports)]
    use fprime_core::*;
    pub mod CdhCore {
        #[allow(unused_imports)]
        use fprime_core::*;
        pub mod cmdDisp {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CMD_NO_OP__size() -> usize {
                4
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CMD_NO_OP__encode<const N: usize>() -> [u8; N] {
                let mut __buf = [0u8; N];
                let __offset = put_u32(&mut __buf, 0, 16777216);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CMD_NO_OP_STRING__size(arg1: &str) -> usize {
                4 + str_len(arg1, 40)
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CMD_NO_OP_STRING__encode<const N: usize>(
                arg1: &str,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 16777217);
                __offset = put_str(&mut __buf, __offset, arg1, 40);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CMD_TEST_CMD_1__size(arg1: i32, arg2: f32, arg3: u8) -> usize {
                13
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CMD_TEST_CMD_1__encode<const N: usize>(
                arg1: i32,
                arg2: f32,
                arg3: u8,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 16777218);
                __offset = put_i32(&mut __buf, __offset, arg1);
                __offset = put_f32(&mut __buf, __offset, arg2);
                __offset = put_u8(&mut __buf, __offset, arg3);
                assert!(__offset == N);
                __buf
            }
        }
        pub mod events {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SET_EVENT_FILTER__size(
                filterLevel: crate::Defs::Svc::EventManager::FilterSeverity,
                filterEnabled: crate::Defs::Svc::EventManager::Enabled,
            ) -> usize {
                6
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SET_EVENT_FILTER__encode<const N: usize>(
                filterLevel: crate::Defs::Svc::EventManager::FilterSeverity,
                filterEnabled: crate::Defs::Svc::EventManager::Enabled,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 16781312);
                __offset = put_u8(&mut __buf, __offset, filterLevel as u8);
                __offset = put_u8(&mut __buf, __offset, filterEnabled as u8);
                assert!(__offset == N);
                __buf
            }
        }
        pub mod health {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn HLTH_PING_ENABLE__size(
                entry: &str,
                enable: crate::Defs::Fw::Enabled,
            ) -> usize {
                5 + str_len(entry, 40)
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn HLTH_PING_ENABLE__encode<const N: usize>(
                entry: &str,
                enable: crate::Defs::Fw::Enabled,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 16785409);
                __offset = put_str(&mut __buf, __offset, entry, 40);
                __offset = put_u8(&mut __buf, __offset, enable as u8);
                assert!(__offset == N);
                __buf
            }
        }
    }
    pub mod Ref {
        #[allow(unused_imports)]
        use fprime_core::*;
        pub mod SG1 {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn Dp__size(
                reqType: crate::Defs::Ref::SignalGen::DpReqType,
                records: u32,
                priority: u32,
            ) -> usize {
                16
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn Dp__encode<const N: usize>(
                reqType: crate::Defs::Ref::SignalGen::DpReqType,
                records: u32,
                priority: u32,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268505091);
                __offset = put_i32(&mut __buf, __offset, reqType as i32);
                __offset = put_u32(&mut __buf, __offset, records);
                __offset = put_u32(&mut __buf, __offset, priority);
                assert!(__offset == N);
                __buf
            }
        }
        pub mod dpDemo {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SelectColor__size(
                color: crate::Defs::Ref::DpDemo::ColorEnum,
            ) -> usize {
                8
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SelectColor__encode<const N: usize>(
                color: crate::Defs::Ref::DpDemo::ColorEnum,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 2576);
                __offset = put_i32(&mut __buf, __offset, color as i32);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn Dp__size(
                reqType: crate::Defs::Ref::DpDemo::DpReqType,
                priority: u32,
                proc: crate::Defs::Fw::DpCfg::ProcType,
            ) -> usize {
                13
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn Dp__encode<const N: usize>(
                reqType: crate::Defs::Ref::DpDemo::DpReqType,
                priority: u32,
                proc: crate::Defs::Fw::DpCfg::ProcType,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 2577);
                __offset = put_i32(&mut __buf, __offset, reqType as i32);
                __offset = put_u32(&mut __buf, __offset, priority);
                __offset = put_u8(&mut __buf, __offset, proc as u8);
                assert!(__offset == N);
                __buf
            }
        }
        pub mod typeDemo {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CHOICE__size(choice: crate::Defs::Ref::Choice) -> usize {
                8
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CHOICE__encode<const N: usize>(
                choice: crate::Defs::Ref::Choice,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455936);
                __offset = put_i32(&mut __buf, __offset, choice as i32);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CHOICES__size(choices: crate::Defs::Ref::ManyChoices) -> usize {
                12
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CHOICES__encode<const N: usize>(
                choices: crate::Defs::Ref::ManyChoices,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455939);
                let mut __i0 = 0;
                while __i0 < 2 {
                    __offset = put_i32(&mut __buf, __offset, choices[__i0] as i32);
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CHOICE_PAIR__size(
                choices: crate::Defs::Ref::ChoicePair,
            ) -> usize {
                12
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CHOICE_PAIR__encode<const N: usize>(
                choices: crate::Defs::Ref::ChoicePair,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455947);
                __offset = put_i32(&mut __buf, __offset, choices.firstChoice as i32);
                __offset = put_i32(&mut __buf, __offset, choices.secondChoice as i32);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn CHOICE_PAIR_WITH_FRIENDS__size(
                repeat: u8,
                choices: crate::Defs::Ref::ChoicePair,
                repeat_max: u8,
            ) -> usize {
                14
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn CHOICE_PAIR_WITH_FRIENDS__encode<const N: usize>(
                repeat: u8,
                choices: crate::Defs::Ref::ChoicePair,
                repeat_max: u8,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455948);
                __offset = put_u8(&mut __buf, __offset, repeat);
                __offset = put_i32(&mut __buf, __offset, choices.firstChoice as i32);
                __offset = put_i32(&mut __buf, __offset, choices.secondChoice as i32);
                __offset = put_u8(&mut __buf, __offset, repeat_max);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn GLUTTON_OF_CHOICE__size(
                choices: crate::Defs::Ref::ChoiceSlurry,
            ) -> usize {
                34
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn GLUTTON_OF_CHOICE__encode<const N: usize>(
                choices: crate::Defs::Ref::ChoiceSlurry,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455951);
                let mut __i0 = 0;
                while __i0 < 2 {
                    let mut __i1 = 0;
                    while __i1 < 2 {
                        __offset = put_i32(
                            &mut __buf,
                            __offset,
                            choices.tooManyChoices[__i0][__i1] as i32,
                        );
                        __i1 += 1;
                    }
                    __i0 += 1;
                }
                __offset = put_i32(&mut __buf, __offset, choices.separateChoice as i32);
                __offset = put_i32(
                    &mut __buf,
                    __offset,
                    choices.choicePair.firstChoice as i32,
                );
                __offset = put_i32(
                    &mut __buf,
                    __offset,
                    choices.choicePair.secondChoice as i32,
                );
                let mut __i0 = 0;
                while __i0 < 2 {
                    __offset = put_u8(
                        &mut __buf,
                        __offset,
                        choices.choiceAsMemberArray[__i0],
                    );
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_SCALARS__size(
                scalar_input: crate::Defs::Ref::ScalarStruct,
            ) -> usize {
                46
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_SCALARS__encode<const N: usize>(
                scalar_input: crate::Defs::Ref::ScalarStruct,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455957);
                __offset = put_i8(&mut __buf, __offset, scalar_input.i8);
                __offset = put_i16(&mut __buf, __offset, scalar_input.i16);
                __offset = put_i32(&mut __buf, __offset, scalar_input.i32);
                __offset = put_i64(&mut __buf, __offset, scalar_input.i64);
                __offset = put_u8(&mut __buf, __offset, scalar_input.u8);
                __offset = put_u16(&mut __buf, __offset, scalar_input.u16);
                __offset = put_u32(&mut __buf, __offset, scalar_input.u32);
                __offset = put_u64(&mut __buf, __offset, scalar_input.u64);
                __offset = put_f32(&mut __buf, __offset, scalar_input.f32);
                __offset = put_f64(&mut __buf, __offset, scalar_input.f64);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_INTS__size(
                a: u8,
                b: i8,
                c: u16,
                d: i16,
                e: u32,
                f: i32,
                g: u64,
                h: i64,
            ) -> usize {
                34
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_INTS__encode<const N: usize>(
                a: u8,
                b: i8,
                c: u16,
                d: i16,
                e: u32,
                f: i32,
                g: u64,
                h: i64,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455958);
                __offset = put_u8(&mut __buf, __offset, a);
                __offset = put_i8(&mut __buf, __offset, b);
                __offset = put_u16(&mut __buf, __offset, c);
                __offset = put_i16(&mut __buf, __offset, d);
                __offset = put_u32(&mut __buf, __offset, e);
                __offset = put_i32(&mut __buf, __offset, f);
                __offset = put_u64(&mut __buf, __offset, g);
                __offset = put_i64(&mut __buf, __offset, h);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_FLOATS__size(single: f32, double: f64) -> usize {
                16
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_FLOATS__encode<const N: usize>(
                single: f32,
                double: f64,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455959);
                __offset = put_f32(&mut __buf, __offset, single);
                __offset = put_f64(&mut __buf, __offset, double);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_BOOL__size(flag: bool) -> usize {
                5
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_BOOL__encode<const N: usize>(flag: bool) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455960);
                __offset = put_u8(&mut __buf, __offset, flag as u8);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_ALIAS__size(
                aliased: crate::Defs::Ref::AliasedU32,
            ) -> usize {
                8
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_ALIAS__encode<const N: usize>(
                aliased: crate::Defs::Ref::AliasedU32,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455961);
                __offset = put_u32(&mut __buf, __offset, aliased);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_WIDE_CHOICE__size(
                choice: crate::Defs::Ref::WideChoice,
            ) -> usize {
                12
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_WIDE_CHOICE__encode<const N: usize>(
                choice: crate::Defs::Ref::WideChoice,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455962);
                __offset = put_i64(&mut __buf, __offset, choice as i64);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_TINY_STRING__size(tiny: &str) -> usize {
                4 + str_len(tiny, 8)
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_TINY_STRING__encode<const N: usize>(
                tiny: &str,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455963);
                __offset = put_str(&mut __buf, __offset, tiny, 8);
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_2D__size(
                choices: crate::Defs::Ref::TooManyChoices,
            ) -> usize {
                20
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_2D__encode<const N: usize>(
                choices: crate::Defs::Ref::TooManyChoices,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455964);
                let mut __i0 = 0;
                while __i0 < 2 {
                    let mut __i1 = 0;
                    while __i1 < 2 {
                        __offset = put_i32(
                            &mut __buf,
                            __offset,
                            choices[__i0][__i1] as i32,
                        );
                        __i1 += 1;
                    }
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_U32_ARRAY__size(
                values: crate::Defs::Ref::DpDemo::U32Array,
            ) -> usize {
                24
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_U32_ARRAY__encode<const N: usize>(
                values: crate::Defs::Ref::DpDemo::U32Array,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455965);
                let mut __i0 = 0;
                while __i0 < 5 {
                    __offset = put_u32(&mut __buf, __offset, values[__i0]);
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_BOOL_ARRAY__size(
                flags: crate::Defs::Ref::DpDemo::BooleanArray,
            ) -> usize {
                6
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_BOOL_ARRAY__encode<const N: usize>(
                flags: crate::Defs::Ref::DpDemo::BooleanArray,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455966);
                let mut __i0 = 0;
                while __i0 < 2 {
                    __offset = put_u8(&mut __buf, __offset, flags[__i0] as u8);
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_FLOAT_ARRAY__size(
                values: crate::Defs::Ref::DpDemo::F32Array,
            ) -> usize {
                16
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_FLOAT_ARRAY__encode<const N: usize>(
                values: crate::Defs::Ref::DpDemo::F32Array,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455967);
                let mut __i0 = 0;
                while __i0 < 3 {
                    __offset = put_f32(&mut __buf, __offset, values[__i0]);
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn SEND_COLOR_ARRAY__size(
                colors: crate::Defs::Ref::DpDemo::EnumArray,
            ) -> usize {
                16
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn SEND_COLOR_ARRAY__encode<const N: usize>(
                colors: crate::Defs::Ref::DpDemo::EnumArray,
            ) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268455968);
                let mut __i0 = 0;
                while __i0 < 3 {
                    __offset = put_i32(&mut __buf, __offset, colors[__i0] as i32);
                    __i0 += 1;
                }
                assert!(__offset == N);
                __buf
            }
        }
        pub mod wasmSeq {
            #[allow(unused_imports)]
            use fprime_core::*;
            /// Wire size of this command with these arguments.
            #[allow(unused_variables)]
            pub const fn LOAD__size(fileName: &str) -> usize {
                4 + str_len(fileName, 240)
            }
            /// The encoded `Fw::ComBuffer` for this command with these arguments.
            ///
            /// `N` must be what the matching `__size` returned; the assertion is
            /// proven during const evaluation, so a mismatch is a compile error and
            /// costs nothing at runtime.
            pub const fn LOAD__encode<const N: usize>(fileName: &str) -> [u8; N] {
                let mut __buf = [0u8; N];
                let mut __offset = put_u32(&mut __buf, 0, 268464130);
                __offset = put_str(&mut __buf, __offset, fileName, 240);
                assert!(__offset == N);
                __buf
            }
        }
    }
}

//! Dictionary point descriptors for generated `Desc` consts and test scaffolding.
//!
//! Unused consts cost nothing on board — they aren't codegen'd.

use crate::{Serializable, String};
use core::marker::PhantomData;

/// A telemetry channel; `I` is the point's wire type, the one [`Serializable`] writes.
/// A `string size N` channel is a `Chan<String<N>>`, which a test still supplies with a
/// plain `&str` — see [`IntoWire`].
pub struct Chan<I: Serializable> {
    /// `FwChanIdType`.
    pub id: i64,
    /// Dotted dictionary name, e.g. `"Ref.power.BatteryVoltage"`.
    pub path: &'static str,
    /// Dictionary type spelling, e.g. `"F32"`. For failure messages only.
    pub ty: &'static str,
    /// The wire type is what a value is supplied as, not something a descriptor stores.
    wire: PhantomData<fn(I)>,
}

/// A parameter descriptor — kept distinct from [`Chan`] since a channel and a parameter
/// are separate host calls with separate id spaces and validity encodings.
pub struct Prm<I: Serializable> {
    /// `FwPrmIdType`.
    pub id: i64,
    pub path: &'static str,
    pub ty: &'static str,
    wire: PhantomData<fn(I)>,
}

impl<I: Serializable> Chan<I> {
    pub const fn new(id: i64, path: &'static str, ty: &'static str) -> Self {
        Chan {
            id,
            path,
            ty,
            wire: PhantomData,
        }
    }
}

impl<I: Serializable> Prm<I> {
    pub const fn new(id: i64, path: &'static str, ty: &'static str) -> Self {
        Prm {
            id,
            path,
            ty,
            wire: PhantomData,
        }
    }
}

// Written by hand, not derived: `derive(Copy)` requires `I: Copy`, which a generated
// struct type (Clone but not Copy) would fail.
impl<I: Serializable> Clone for Chan<I> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<I: Serializable> Copy for Chan<I> {}

impl<I: Serializable> Clone for Prm<I> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<I: Serializable> Copy for Prm<I> {}

/// What a test may write where a point of wire type `I` is expected: the wire type itself,
/// or a `&str` for a `string size N` point.
///
/// This is why a descriptor needs no encoder of its own — every dictionary type is
/// [`Serializable`], and the one type a test would rather write by hand than construct,
/// a string, converts on the way in.
pub trait IntoWire<I> {
    fn into_wire(self) -> I;
}

impl<I: Serializable> IntoWire<I> for I {
    fn into_wire(self) -> I {
        self
    }
}

impl<const N: usize> IntoWire<String<N>> for &str {
    /// Truncated to what the point holds, as assigning to an `Fw::String` is on board.
    ///
    /// Pushed a character at a time rather than through `StrTruncate`: nothing here is
    /// flight code, so this side can afford to stop on a character boundary and to do it
    /// without `unsafe`.
    fn into_wire(self) -> String<N> {
        let mut out = String::new();
        for c in self.chars() {
            if out.push(c).is_err() {
                break;
            }
        }
        out
    }
}

/// A command matched on opcode alone, with no argument values — what a bare
/// `Ref.power.PWR_OFF` in a test means.
#[derive(Clone, Copy)]
pub struct CmdDesc {
    pub opcode: u32,
    /// Dotted dictionary name, e.g. `"Ref.power.PWR_OFF"`.
    pub path: &'static str,
}

/// A command with an exact, const-encoded `Fw::ComBuffer`, from the same generated
/// `Konst` encoder a sequence uses to send it.
#[derive(Clone, Copy)]
pub struct Cmd {
    pub path: &'static str,
    /// A big-endian `FwOpcodeType`, then the arguments.
    pub buffer: &'static [u8],
}

/// Width of a serialised `FwOpcodeType`, which every ComBuffer starts with.
const OPCODE_BYTES: usize = 4;

impl CmdDesc {
    /// Attaches an encoded buffer. `const fn` so a test step is one constant expression.
    pub const fn with(self, buffer: &'static [u8]) -> Cmd {
        Cmd {
            path: self.path,
            buffer,
        }
    }
}

impl Cmd {
    /// The big-endian opcode the buffer leads with (`from_be_bytes` isn't usable on a
    /// slice in a `const fn`).
    pub const fn opcode(&self) -> u32 {
        let b = self.buffer;
        if b.len() < OPCODE_BYTES {
            // Every generated encoder writes the opcode first; reaching here means a
            // corrupted buffer.
            panic!("a command buffer is shorter than its opcode");
        }
        ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32)
    }

    /// The buffer's argument bytes (opcode stripped) — what a recorded `Call::Command`
    /// payload holds.
    pub const fn args(&self) -> &'static [u8] {
        let (_, args) = self.buffer.split_at(OPCODE_BYTES);
        args
    }
}

/// A named `Fw::CmdResponse`, so a failure message can say `EXECUTION_ERROR (4)` instead
/// of `4`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Response {
    pub code: i32,
    pub name: &'static str,
}

impl Response {
    pub const fn new(code: i32, name: &'static str) -> Self {
        Response { code, name }
    }
}

/// A named `Fw::LogSeverity`, for the same reason as [`Response`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Severity {
    pub code: i32,
    pub name: &'static str,
}

impl Severity {
    pub const fn new(code: i32, name: &'static str) -> Self {
        Severity { code, name }
    }
}

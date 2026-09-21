//! What one run cost, and how it ended.

use super::{Recording, Usage};

/// How the run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The sequence called `fprime_v1.exit`.
    Exited(i32),
    /// The sequence called `fprime_v1.panic`, which `fprime_core` does from its panic
    /// handler and from a failed command under `FailMode::Verified`.
    Panicked(i32),
    /// `main` returned without calling `exit`.
    Returned,
    /// The interpreter trapped: an out-of-bounds access, an unreachable, a failed
    /// host call.
    Trapped(String),
    /// A host call asked to suspend and no message was available to resume it — a
    /// blocking `serial_recv` on an empty queue.
    Suspended,
    /// Hit [`Limits::max_instructions`](super::Limits::max_instructions) and was still running.
    OutOfInstructions,
}

impl Outcome {
    /// Whether this is a nominal end to a sequence. Returning from `main` without
    /// calling `exit` is fine: `#[fprime_main]` does not require it.
    pub fn is_nominal(&self) -> bool {
        matches!(self, Outcome::Exited(0) | Outcome::Returned)
    }
}

/// Guest linear-memory stack use, measured by poisoning the region and seeing how far
/// in the sequence wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestStack {
    /// Bytes the linker reserved (`-zstack-size`).
    pub reserved: usize,
    /// Bytes the sequence actually touched, at its deepest.
    pub used: usize,
}

impl GuestStack {
    pub fn headroom(&self) -> usize {
        self.reserved.saturating_sub(self.used)
    }
}

/// Everything one run measured.
#[derive(Debug)]
pub struct Report {
    pub outcome: Outcome,
    pub usage: Usage,
    /// Code pages the module compiled to, against [`Limits::max_code_pages`](super::Limits::max_code_pages).
    pub code_pages: usize,
    /// 16-bit instruction words used across those pages, and the words they hold.
    pub code_words: usize,
    pub code_capacity: usize,
    pub instructions: u64,
    /// Deepest the operand stack got, in 32-bit words, against [`Limits::stack_size`](super::Limits::stack_size).
    pub peak_operand_stack: usize,
    /// Peak linear memory, growth included, against [`Limits::guest_memory`](super::Limits::guest_memory).
    pub guest_memory: u64,
    /// What the module declared before running. Lower than `guest_memory` if it grew.
    pub declared_memory: u64,
    /// `memory.grow` requests the pool refused. The guest saw -1 and continued, so it
    /// may have taken a path it would not take with a larger pool.
    pub refused_grows: u64,
    /// `None` when the module declares no `__stack_pointer`, which means it never
    /// spills to linear memory and so uses no guest stack at all.
    pub guest_stack: Option<GuestStack>,
    pub recording: Recording,
}

impl Report {
    /// Pages of `page_size` needed to hold the interpreter heap at its peak.
    pub fn required_heap_pages(&self) -> u32 {
        self.usage.peak_pages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clean exit and a plain return are both fine; anything else is not.
    #[test]
    fn classifies_nominal_outcomes() {
        assert!(Outcome::Exited(0).is_nominal());
        assert!(Outcome::Returned.is_nominal());
        assert!(!Outcome::Exited(1).is_nominal());
        assert!(!Outcome::Panicked(0).is_nominal());
        assert!(!Outcome::Trapped("Unreachable".into()).is_nominal());
        assert!(!Outcome::Suspended.is_nominal());
        assert!(!Outcome::OutOfInstructions.is_nominal());
    }

    #[test]
    fn guest_stack_headroom_never_underflows() {
        assert_eq!(
            GuestStack {
                reserved: 512,
                used: 328
            }
            .headroom(),
            184
        );
        // Should not be reachable, but must not panic if it is.
        assert_eq!(
            GuestStack {
                reserved: 512,
                used: 600
            }
            .headroom(),
            0
        );
    }
}

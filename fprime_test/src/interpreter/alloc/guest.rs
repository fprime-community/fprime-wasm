//! Guest linear memory, bounded like the on-board bump pool.

use spacewasm::{AllocError, WasmMemoryAllocator};
use std::alloc::Layout;
use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

/// The pool guest memory is served from.
///
/// `WasmSequencer` serves it from a bump pool of `Config::guestMemorySize`. A
/// `memory.grow` is granted only if the memory is the last allocation in that pool
/// and the grown size still fits it (`guestRealloc`); otherwise the guest sees
/// `memory.grow` return -1 and carries on. `verify` loads one module at a time, so
/// it is always the last allocation and the size bound is the whole test.
///
/// # One deliberate divergence
///
/// On board, `guestAlloc` refuses the *initial* allocation too, so a module
/// declaring more memory than the pool fails to load. Here it is granted anyway,
/// because "needs 941 bytes, configured 8192" is the answer `verify` exists to
/// give, and `GuestMemoryAllocationFailure` is not. Growth is still bounded, so a
/// module cannot grow past the pool and be reported as fitting.
#[derive(Debug)]
pub struct GuestPool {
    capacity: usize,
    /// Bytes currently handed out.
    granted: Cell<usize>,
    /// Largest the guest's memory ever got, which is what the budget must cover.
    peak: Cell<usize>,
    /// Grow requests refused for want of pool space.
    refused: Cell<u64>,
}

impl GuestPool {
    pub fn new(capacity: usize) -> Self {
        GuestPool {
            capacity,
            granted: Cell::new(0),
            peak: Cell::new(0),
            refused: Cell::new(0),
        }
    }

    /// Peak bytes the guest's linear memory reached, growth included.
    pub fn peak(&self) -> usize {
        self.peak.get()
    }

    /// How many `memory.grow` requests the pool refused.
    pub fn refused(&self) -> u64 {
        self.refused.get()
    }

    /// Grant the initial allocation whatever its size; see the divergence above.
    fn grant(&self, size: usize) {
        self.granted.set(size);
        self.peak.set(self.peak.get().max(size));
    }

    /// Whether a grow to `size` fits. Bounded by the pool, or by what was already
    /// granted if that is larger — an over-declared module has already been let
    /// through, and refusing every subsequent grow would say nothing useful.
    fn may_grow_to(&self, size: usize) -> bool {
        if size > self.capacity.max(self.granted.get()) {
            self.refused.set(self.refused.get() + 1);
            return false;
        }
        self.grant(size);
        true
    }
}

/// The allocator handed to `spacewasm` for guest linear memory.
///
/// Holds a shared handle to the pool so the caller can read the peak back after the
/// run; `spacewasm` takes ownership of the allocator itself.
pub struct Guest {
    pool: Rc<GuestPool>,
}

impl Guest {
    pub fn new(pool: Rc<GuestPool>) -> Self {
        Guest { pool }
    }
}

impl WasmMemoryAllocator for Guest {
    fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        self.pool.grant(layout.size());
        NonNull::new(unsafe { std::alloc::alloc(layout) }).ok_or(AllocError::AllocationFailed)
    }

    fn reallocate(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        layout: Layout,
    ) -> Result<NonNull<u8>, AllocError> {
        // Refusing here is what makes the guest see `memory.grow` return -1,
        // exactly as the on-board pool does; it is not a trap.
        if !self.pool.may_grow_to(layout.size()) {
            return Err(AllocError::OutOfMemory);
        }
        NonNull::new(unsafe { std::alloc::realloc(ptr.as_ptr(), old_layout, layout.size()) })
            .ok_or(AllocError::AllocationFailed)
    }

    fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { std::alloc::dealloc(ptr.as_ptr(), layout) }
    }
}

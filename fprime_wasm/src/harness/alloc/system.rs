//! The host allocator, as `spacewasm` wants to see it.

use spacewasm::{AllocError, Allocator, WasmMemoryAllocator};
use std::alloc::Layout;
use std::ptr::NonNull;

/// Plain system allocator, used both as the backing store for whole pages and as
/// the guest linear-memory allocator (which on board is a separate bump allocator
/// over `Config::guestMemorySize`, not the page pool).
#[derive(Clone, Copy)]
pub struct System;

unsafe impl Allocator for System {
    unsafe fn alloc(&self, layout: Layout) -> Result<*mut u8, AllocError> {
        // The trait's contract is that `layout` is non-zero-sized, which is what
        // makes `std::alloc::alloc` sound here.
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            Err(AllocError::AllocationFailed)
        } else {
            Ok(ptr)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { std::alloc::dealloc(ptr, layout) }
    }
}

impl WasmMemoryAllocator for System {
    fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        NonNull::new(unsafe { std::alloc::alloc(layout) }).ok_or(AllocError::AllocationFailed)
    }

    fn reallocate(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        layout: Layout,
    ) -> Result<NonNull<u8>, AllocError> {
        NonNull::new(unsafe { std::alloc::realloc(ptr.as_ptr(), old_layout, layout.size()) })
            .ok_or(AllocError::AllocationFailed)
    }

    fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { std::alloc::dealloc(ptr.as_ptr(), layout) }
    }
}

//! The interpreter's heap, and what one run asked of it.
//!
//! On board, `Svc::WasmSequencer` hands `spacewasm` a
//! `PageAllocator<_, SPACEWASM_MAX_PAGES>` over a pool of fixed-size pages carved
//! out of an `Fw::MemAllocator` at `configure` time. Every allocation the
//! interpreter makes — the operand stack, the code page table, each code page, the
//! module store — comes from there, and two things can go wrong:
//!
//! * a single allocation larger than one page can never be served, whatever the
//!   pool size, because a page is the largest contiguous block
//!   (`AllocError::PageTooSmall`);
//! * the pages in flight at peak can exceed `Config::heapPages`
//!   (`AllocError::OutOfMemory`).
//!
//! So [`Tracker`] wraps the same allocator and records the largest single
//! allocation and the live high-water mark, which is what turns a `.wasm` file into
//! the two numbers a deployment has to configure.

use super::System;
use spacewasm::{AllocError, Allocator, PageAllocator};
use std::alloc::Layout;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::BTreeSet;

/// Page-table capacity, standing in for `SPACEWASM_MAX_PAGES`. Bounds how many
/// pages the allocator may hold at once; `verify` reports the number actually used,
/// so this only has to be above any plausible configuration.
pub const MAX_PAGES: usize = 4096;

/// Page size used if something allocates before [`Tracker::configure`] runs. The
/// measured path always configures first; this only keeps an early allocation from
/// having nowhere to go, which in an allocator cannot be reported as an error the
/// caller could act on.
const DEFAULT_PAGE_SIZE: usize = 8192;

/// What the interpreter asked of its heap over one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Largest single allocation. A page smaller than this can never serve it, so
    /// this is the floor on `SPACEWASM_PAGE_SIZE`.
    pub largest: usize,
    /// Peak sum of live allocation sizes, excluding page padding.
    pub peak_live: usize,
    /// Bytes still held when the run was measured. Not a leak: the decoded module,
    /// its code pages and the interpreter are all still alive at that point, so this
    /// is what a loaded, idle sequence occupies.
    pub resident: usize,
    /// Total allocations made.
    pub allocations: u64,
    /// Pages held at the end of the run, and the padding lost inside them. These
    /// come from the `PageAllocator` itself, so they account for the bump
    /// allocator's real behaviour rather than an estimate from `peak_live`.
    pub pages: u32,
    pub padding: u32,
    pub page_bytes: u32,
    /// Peak pages held at any point in the run. `Config::heapPages` has to cover
    /// this, not the count left at the end.
    pub peak_pages: u32,
    /// Allocations larger than one page. On board each of these is a `PageTooSmall`
    /// failure that stops the sequence; here they are served from the system
    /// allocator so the run can finish and report everything at once.
    pub oversize: u64,
}

impl Usage {
    /// Smallest page size that would have served every allocation, rounded up to a
    /// power of two: `SPACEWASM_PAGE_SIZE` is a whole-pool divisor and the allocator
    /// aligns pages to 8, so a power of two is what a deployment actually sets.
    pub fn required_page_size(&self) -> usize {
        self.largest.next_power_of_two().max(8)
    }
}

/// A `PageAllocator` that records what passed through it.
///
/// The page allocator is behind an `Option` because the page size is a runtime
/// choice (`--page-size`) while the global allocator has to be a `const`
/// initialiser. [`Tracker::configure`] installs it before the interpreter is built,
/// and every allocation after that goes through it.
pub struct Tracker {
    pages: UnsafeCell<Option<PageAllocator<System, MAX_PAGES>>>,
    largest: Cell<usize>,
    live: Cell<usize>,
    peak_live: Cell<usize>,
    allocations: Cell<u64>,
    peak_pages: Cell<u32>,
    oversize_count: Cell<u64>,
    /// Addresses served from the system allocator because no page could hold them,
    /// so `dealloc` can route them back to the same place. Addresses rather than
    /// pointers to keep the set `Ord` without a raw-pointer compare.
    oversize: RefCell<BTreeSet<usize>>,
}

impl Tracker {
    pub const fn new() -> Self {
        Tracker {
            pages: UnsafeCell::new(None),
            largest: Cell::new(0),
            live: Cell::new(0),
            peak_live: Cell::new(0),
            allocations: Cell::new(0),
            peak_pages: Cell::new(0),
            oversize_count: Cell::new(0),
            oversize: RefCell::new(BTreeSet::new()),
        }
    }

    /// Install a page allocator of `page_size` bytes per page and clear the
    /// counters, so each module in a multi-module run is measured on its own.
    ///
    /// # Panics
    ///
    /// If anything is still allocated. Replacing the page allocator while the
    /// interpreter holds pointers into its pages would free them out from under it,
    /// so this refuses rather than corrupting the run.
    pub fn configure(&self, page_size: usize) {
        assert_eq!(
            self.live.get(),
            0,
            "cannot reconfigure the allocator while {} bytes are still live",
            self.live.get()
        );
        assert!(page_size > 0, "page size must be positive");

        self.largest.set(0);
        self.peak_live.set(0);
        self.allocations.set(0);
        self.peak_pages.set(0);
        self.oversize_count.set(0);
        self.oversize.borrow_mut().clear();

        // SAFETY: single-threaded, and the assertion above establishes that no
        // outstanding allocation points into the allocator being replaced.
        unsafe { *self.pages.get() = Some(PageAllocator::new(System, page_size)) };
    }

    fn page_allocator(&self) -> &PageAllocator<System, MAX_PAGES> {
        // SAFETY: single-threaded. Installing a default rather than panicking
        // because this runs inside `alloc`, where a panic cannot unwind.
        unsafe {
            let slot = &mut *self.pages.get();
            slot.get_or_insert_with(|| PageAllocator::new(System, DEFAULT_PAGE_SIZE))
        }
    }

    pub fn usage(&self) -> Usage {
        let stats = self.page_allocator().stats();
        Usage {
            largest: self.largest.get(),
            peak_live: self.peak_live.get(),
            resident: self.live.get(),
            allocations: self.allocations.get(),
            pages: stats.pages,
            padding: stats.pad_bytes,
            page_bytes: stats.total_bytes,
            peak_pages: self.peak_pages.get(),
            oversize: self.oversize_count.get(),
        }
    }
}

unsafe impl Allocator for Tracker {
    unsafe fn alloc(&self, layout: Layout) -> Result<*mut u8, AllocError> {
        self.largest.set(self.largest.get().max(layout.size()));
        self.live.set(self.live.get() + layout.size());
        self.peak_live
            .set(self.peak_live.get().max(self.live.get()));
        self.allocations.set(self.allocations.get() + 1);

        let pages = self.page_allocator();
        let result = unsafe { pages.alloc(layout) };
        let ptr = match result {
            Ok(ptr) => ptr,
            // On board these are terminal. Serving them here instead lets one run
            // report every budget that was blown rather than only the first.
            Err(AllocError::PageTooSmall | AllocError::OutOfMemory) => {
                let ptr = unsafe { std::alloc::alloc(layout) };
                if ptr.is_null() {
                    self.live.set(self.live.get() - layout.size());
                    return Err(AllocError::AllocationFailed);
                }
                self.oversize_count.set(self.oversize_count.get() + 1);
                self.oversize.borrow_mut().insert(ptr as usize);
                ptr
            }
            Err(err) => {
                self.live.set(self.live.get() - layout.size());
                return Err(err);
            }
        };

        // Sampled after the allocation that could have added a page. Pages are
        // released as they empty, so the count at the end understates the peak.
        self.peak_pages
            .set(self.peak_pages.get().max(pages.stats().pages));
        Ok(ptr)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.live.set(self.live.get() - layout.size());
        if self.oversize.borrow_mut().remove(&(ptr as usize)) {
            unsafe { std::alloc::dealloc(ptr, layout) };
            return;
        }
        unsafe { self.page_allocator().dealloc(ptr, layout) }
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Tracker::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_the_required_page_size_up_to_a_power_of_two() {
        let usage = |largest| Usage {
            largest,
            peak_live: 0,
            resident: 0,
            allocations: 0,
            pages: 0,
            padding: 0,
            page_bytes: 0,
            peak_pages: 0,
            oversize: 0,
        };
        assert_eq!(usage(4096).required_page_size(), 4096);
        assert_eq!(usage(4097).required_page_size(), 8192);
        assert_eq!(usage(1).required_page_size(), 8);
        // A module that allocates nothing still needs a positive page size.
        assert_eq!(usage(0).required_page_size(), 8);
    }
}

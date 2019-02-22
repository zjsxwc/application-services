/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, Ordering, ATOMIC_BOOL_INIT};

use clear_memory::secure_zero_memory;

static SHOULD_ZERO_ALLOCS: AtomicBool = ATOMIC_BOOL_INIT;

pub extern "C" fn zeroing_allocator_enable() {
    SHOULD_ZERO_ALLOCS.store(true, Ordering::SeqCst);
}

/// An allocator that zeroes all memory on release.
///
/// We do this by using the same trick as OpenSSL in OPENSSL_cleanse,
/// e.g. we read a function pointer that resolves to `memset` through
/// a volatile reference. This is effective in defeting compiler
/// optimizations, and relatively simple.
///
/// If `std::intrinsics::volatile_set_memory` ever stablizes, we should
/// do that instead.
///
/// ## Usage
///
/// In the megazord crate, do
///
/// ```rust,no_run
///
/// // Note: for zeroing_allocator_enable
/// pub extern crate zeroing_allocator;
/// #[global_allocator]
/// static A: zeroing_allocator::ZeroingAlloc = zeroing_allocator::ZeroingAlloc;
///
/// ```
pub struct ZeroingAlloc;

unsafe impl GlobalAlloc for ZeroingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // `Acquire` load is enough because it ensures that the enabled
        // or disabled changes are visible to us. This is called on every
        // free, so the perf difference matters for us.
        if SHOULD_ZERO_ALLOCS.load(Ordering::Acquire) {
            secure_zero_memory(ptr, layout.size());
        }
        System.dealloc(ptr, layout);
    }
}

// Implements the mechanics of zeroing memory.
mod clear_memory {

    type MemsetFunc =
        unsafe extern "C" fn(*mut libc::c_void, libc::c_int, libc::size_t) -> *mut libc::c_void;

    #[repr(transparent)]
    struct MemsetHolder(std::cell::UnsafeCell<MemsetFunc>);

    // Nobody assigns to this so it's fine.
    unsafe impl Sync for MemsetHolder {}

    static MEMSET_HOLDER: MemsetHolder = MemsetHolder(std::cell::UnsafeCell::new(libc::memset));

    impl MemsetHolder {
        fn get_memset(&self) -> MemsetFunc {
            unsafe {
                // Note: This is actually a pointer to a function pointer.
                let memset_ptr: *mut MemsetFunc = self.0.get();
                std::ptr::read_volatile(memset_ptr)
            }
        }
    }

    // The inline(never) shouldn't matter here, but the more optimization
    // barriers we have, the better.
    #[inline(never)]
    pub unsafe fn secure_zero_memory(mem: *mut u8, len: usize) {
        if len == 0 {
            return;
        }
        assert!(!mem.is_null());
        let memset_fn = MEMSET_HOLDER.get_memset();
        memset_fn(mem as *mut libc::c_void, 0, len);
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

pub extern crate fxaclient_ffi;
pub extern crate logins_ffi;
pub extern crate rc_log_ffi;

use std::alloc::{GlobalAlloc, Layout, System};

/// An allocator that zeroes all memory on release. This provides some minor
/// security benefit against some fairly esoteric threat models, but has a
/// nontrivial performance cost, and the only consumer who cares Lockbox, so we
/// do this here instead of somewhere shared.
struct ZeroingAlloc;

unsafe impl GlobalAlloc for ZeroingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        zero_memory(ptr, layout.size());
        System.dealloc(ptr, layout);
    }
}

/// Not perfect, but probably as good as we can do here. Hopefully good enough
/// in practice. Ideally we'd call something like OPENSSL_cleanse, which
/// guarantees it does the right thing, but that's tricky to wrangle for us, and
/// presumably the combination of `#[inline(never)]` and `write_volatile` mean
/// that this is fine.
#[inline(never)]
unsafe fn zero_memory(ptr: *mut u8, size: usize) {
    for i in 0..size {
        ptr.offset(i as isize).write_volatile(0)
    }
}

#[global_allocator]
static ZERO_ALLOC: ZeroingAlloc = ZeroingAlloc;

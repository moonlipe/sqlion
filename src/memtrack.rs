//! Test-only allocator that counts the bytes currently allocated, to check that data is really freed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct Counting;

thread_local! {
    /// per thread, so that tests running in parallel do not disturb each other
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

fn add(n: isize) {
    let _ = LIVE.try_with(|c| c.set(c.get() + n));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        add(l.size() as isize);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        add(-(l.size() as isize));
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        add(new as isize - l.size() as isize);
        System.realloc(p, l, new)
    }
}

/// Bytes allocated right now by the calling thread.
pub fn live_bytes() -> usize {
    LIVE.with(|c| c.get().max(0) as usize)
}

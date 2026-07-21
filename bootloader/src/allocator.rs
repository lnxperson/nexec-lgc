use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_START: usize = 0x400000;
const HEAP_SIZE: usize = 1024 * 1024;

struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    const fn new() -> Self {
        BumpAllocator {
            heap_start: HEAP_START,
            heap_end: HEAP_START + HEAP_SIZE,
            next: AtomicUsize::new(HEAP_START),
        }
    }
}

unsafe impl Send for BumpAllocator {}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

pub fn init() {}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        loop {
            let current = self.next.load(Ordering::Relaxed);
            let align = layout.align();
            let size = layout.size();
            let aligned = (current + align - 1) & !(align - 1);
            let end = aligned + size;
            if end > self.heap_end {
                return null_mut();
            }
            if self.next.compare_exchange_weak(current, end, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator never frees
    }
}

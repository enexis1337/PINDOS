use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

const HEAP_SIZE: usize = 1024 * 1024;

#[repr(align(16))]
struct Heap(UnsafeCell<[u8; HEAP_SIZE]>);
unsafe impl Sync for Heap {}

static HEAP: Heap = Heap(UnsafeCell::new([0; HEAP_SIZE]));

struct BumpAllocator {
    next: core::sync::atomic::AtomicUsize,
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap_start = HEAP.0.get() as usize;
        let align = layout.align();
        let size = layout.size();

        let current = self.next.load(core::sync::atomic::Ordering::Relaxed);
        let base = if current == 0 { heap_start } else { current };
        let aligned = (base + align - 1) & !(align - 1);
        let new_next = aligned + size;

        if new_next > heap_start + HEAP_SIZE {
            return null_mut();
        }

        self.next.store(new_next, core::sync::atomic::Ordering::Relaxed);
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
    }
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator { next: core::sync::atomic::AtomicUsize::new(0) };
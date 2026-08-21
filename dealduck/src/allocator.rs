use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

const HEAP_SIZE: usize = 1024 * 1024;

#[repr(align(16))]
pub struct Heap(UnsafeCell<[u8; HEAP_SIZE]>);
unsafe impl Sync for Heap {}

// Make HEAP public so main.rs can access its address
pub static HEAP: Heap = Heap(UnsafeCell::new([0; HEAP_SIZE]));

struct BumpAllocator {
    next: core::sync::atomic::AtomicUsize,
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap_start = HEAP.0.get() as usize;
        
        // DEBUG: ALWAYS print heap address on first alloc
        if self.next.load(core::sync::atomic::Ordering::Relaxed) == 0 {
            // Print "alloc start:"
            let msg = b"[dealduck] alloc start: ";
            core::arch::asm!(
                "syscall",
                in("rax") 1u64,
                in("rdi") 1u64,
                in("rsi") msg.as_ptr() as u64,
                in("rdx") msg.len() as u64,
                lateout("rcx") _,
                lateout("r11") _,
            );
            // Print heap_start as hex (16 chars + newline)
            let mut buf = [0u8; 18];
            let mut val = heap_start;
            for i in (0..16).rev() {
                let nibble = (val & 0xF) as u8;
                buf[i] = if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 };
                val >>= 4;
            }
            buf[16] = b'\n';
            buf[17] = 0;
            core::arch::asm!(
                "syscall",
                in("rax") 1u64,
                in("rdi") 1u64,
                in("rsi") buf.as_ptr() as u64,
                in("rdx") 17u64,
                lateout("rcx") _,
                lateout("r11") _,
            );
        }
        
        // DEBUG: check if heap_start is valid (non-zero, in userspace)
        if heap_start == 0 || heap_start < 0x1000_0000 || heap_start > 0xFFFF_FFFF {
            let msg = b"[dealduck] alloc: HEAP addr INVALID (0 or out of range)!\n";
            core::arch::asm!(
                "syscall",
                in("rax") 1u64,
                in("rdi") 1u64,
                in("rsi") msg.as_ptr() as u64,
                in("rdx") msg.len() as u64,
                lateout("rcx") _,
                lateout("r11") _,
            );
            return null_mut();
        }
        
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
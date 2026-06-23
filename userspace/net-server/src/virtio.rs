use core::sync::atomic::{fence, Ordering};
use alloc::alloc::{alloc_zeroed, Layout};

const QUEUE_SIZE: usize = 256;

#[repr(C)]
struct VirtqDesc {
    addr:  u64,
    len:   u32,
    flags: u16,
    next:  u16,
}

#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx:   u16,
    ring:  [u16; QUEUE_SIZE],
}

#[repr(C)]
struct VirtqUsedElem { id: u32, len: u32 }

#[repr(C)]
struct VirtqUsed {
    flags: u16,
    idx:   u16,
    ring:  [VirtqUsedElem; QUEUE_SIZE],
}

pub struct Virtqueue {
    desc:      *mut VirtqDesc,
    avail:     *mut VirtqAvail,
    used:      *mut VirtqUsed,
    free_head: u16,
    last_used: u16,
    io_base:   u16,
    queue_idx: u16,
}

unsafe fn dbg_outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nostack));
}
unsafe fn dbg_str(s: &str) {
    for &b in s.as_bytes() {
        dbg_outb(0x3f8, b);
    }
}

impl Virtqueue {
    pub unsafe fn init(io_base: u16, queue_idx: u16) -> Self {
        dbg_str("[virtio] init start\n");

        let desc_bytes  = core::mem::size_of::<VirtqDesc>() * QUEUE_SIZE;
        let avail_bytes = core::mem::size_of::<VirtqAvail>();
        let used_bytes  = core::mem::size_of::<VirtqUsed>();
        let total = desc_bytes + avail_bytes + used_bytes;
        dbg_str("[virtio] alloc_zeroed\n");

        let layout = Layout::from_size_align(total, 4096).expect("layout");
        let ptr = alloc_zeroed(layout);
        if ptr.is_null() { panic!("virtqueue alloc failed"); }

        let desc  = ptr as *mut VirtqDesc;
        let avail = ptr.add(desc_bytes) as *mut VirtqAvail;
        let used  = ptr.add(desc_bytes + avail_bytes) as *mut VirtqUsed;

        for i in 0..QUEUE_SIZE - 1 {
            (*desc.add(i)).next  = (i + 1) as u16;
            (*desc.add(i)).flags = 1;
        }

        dbg_str("[virtio] QUEUE_SEL\n");
        outw(io_base + 14, queue_idx);
        dbg_str("[virtio] QUEUE_PFN\n");
        outl(io_base + 8,  (ptr as u32) / 4096);

        dbg_str("[virtio] init done\n");

        Self { desc, avail, used, free_head: 0, last_used: 0, io_base, queue_idx }
    }

    pub unsafe fn send(&mut self, data: &[u8]) {
        let idx = self.free_head as usize;
        self.free_head = (*self.desc.add(idx)).next;

        (*self.desc.add(idx)).addr  = data.as_ptr() as u64;
        (*self.desc.add(idx)).len   = data.len() as u32;
        (*self.desc.add(idx)).flags = 0;

        let avail_idx = (*self.avail).idx as usize % QUEUE_SIZE;
        (*self.avail).ring[avail_idx] = idx as u16;
        fence(Ordering::Release);
        (*self.avail).idx = (*self.avail).idx.wrapping_add(1);
        fence(Ordering::Release);

        outw(self.io_base + 16, self.queue_idx);
    }

    pub unsafe fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        if (*self.used).idx == self.last_used { return None; }

        let elem = &(*self.used).ring[self.last_used as usize % QUEUE_SIZE];
        let desc = &*self.desc.add(elem.id as usize);
        let len  = (elem.len as usize).min(buf.len());

        core::ptr::copy_nonoverlapping(desc.addr as *const u8, buf.as_mut_ptr(), len);
        self.last_used = self.last_used.wrapping_add(1);

        (*self.desc.add(elem.id as usize)).next = self.free_head;
        self.free_head = elem.id as u16;

        Some(len)
    }
}

unsafe fn outw(port: u16, val: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nostack));
}
unsafe fn outl(port: u16, val: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack));
}

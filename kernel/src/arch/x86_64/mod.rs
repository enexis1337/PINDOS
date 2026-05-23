// x86_64 architecture-specific code
pub mod bootstrap;
pub mod gdt;
pub mod idt;
pub mod paging;

use crate::memory::frame::PhysicalFrame;
use x86_64::structures::paging::{PageTable, PhysFrame};
use x86_64::VirtAddr;

/// Early initialization for x86_64
pub fn early_init(multiboot_info: usize, _boot_type: u32) {
    // Validate Multiboot2 header if present
    unsafe {
        if _boot_type == 0x36D76289 {
            // Multiboot2 magic number found
            let mbi = multiboot_info as *const Multiboot2Header;
            parse_multiboot2_info(&*mbi);
        }
    }
    
    // Initialize GDT
    gdt::init();
    
    // Initialize IDT
    idt::init();
    
    // Initialize paging
    paging::init();
}

/// Multiboot2 header structure
#[repr(C, packed)]
struct Multiboot2Header {
    total_size: u32,
    reserved: u32,
}

unsafe fn parse_multiboot2_info(header: &Multiboot2Header) {
    let mut offset = 8;
    let end = header.total_size as usize;
    
    while offset < end {
        let tag = (header as *const _ as usize + offset) as *const MultibootTag;
        let tag_type = (*tag).tag_type;
        let tag_size = (*tag).size;
        
        match tag_type {
            1 => { // Basic memory info
                let mem_info = &*tag as *const MultibootTagMemory as *const MemInfoTag;
                let lower = (*mem_info).mem_lower;
                let upper = (*mem_info).mem_upper;
                // Store memory info for allocator
            }
            3 => { // Boot device
                let bootdev = &*tag as *const MultibootTagBootdev as *const BootDevTag;
                // Parse boot device info
            }
            6 => { // Memory map
                let mmap = &*tag as *const MultibootTagMmap as *const MmapTag;
                parse_memory_map(&(*mmap));
            }
            8 => { // Framebuffer info
                let fb = &*tag as *const MultibootTagFramebuffer as *const FramebufferTag;
                // Store framebuffer info
            }
            _ => {}
        }
        
        // Align to 8-byte boundary
        offset = (offset + tag_size as usize + 7) & !7;
    }
}

#[repr(C, packed)]
struct MultibootTag {
    tag_type: u32,
    size: u32,
}

#[repr(C, packed)]
struct MemInfoTag {
    tag_type: u32,
    size: u32,
    mem_lower: u32,
    mem_upper: u32,
}

#[repr(C, packed)]
struct BootDevTag {
    tag_type: u32,
    size: u32,
    biosdev: u32,
    part: u32,
    subpart: u32,
}

#[repr(C, packed)]
struct MmapTag {
    tag_type: u32,
    size: u32,
    entry_size: u32,
    entry_version: u32,
}

#[repr(C, packed)]
struct MmapEntry {
    addr: u64,
    len: u64,
    mmap_type: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct FramebufferTag {
    tag_type: u32,
    size: u32,
    framebuffer_addr: u64,
    framebuffer_pitch: u64,
    framebuffer_width: u64,
    framebuffer_height: u64,
    framebuffer_bpp: u8,
    framebuffer_type: u8,
    reserved: u8,
}

fn parse_memory_map(mmap: &MmapTag) {
    let entries = (mmap.size as usize - 16) / mmap.entry_size as usize;
    let mmap_entries = (mmap as *const _ as usize + 16) as *const MmapEntry;
    
    for i in 0..entries {
        unsafe {
            let entry = &*mmap_entries.add(i);
            if entry.mmap_type == 1 {
                // Usable memory region
                let start = entry.addr as usize;
                let end = (entry.addr + entry.len) as usize;
                // Add to memory allocator
            }
        }
    }
}
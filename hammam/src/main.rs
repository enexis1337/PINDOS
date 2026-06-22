#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::all)]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

pub mod boot;

pub mod boot_info;
pub mod drivers;
pub mod interrupts;
pub mod mm;
pub mod acpi;
pub mod arch;
pub mod sched;
pub mod smp;
pub mod loader;
pub mod cap;
pub mod process;
pub mod userspace_blob;
pub mod vfs;
pub mod io;
pub mod block;
pub mod security;

use boot_info::{MemoryKind, MemoryRegion};
use core::mem::size_of;
use mm::physical::PHYSICAL_ALLOCATOR;

const MAX_MEMORY_REGIONS: usize = 64;

#[repr(C)]
struct Mb2Tag {
    tag_type: u32,
    tag_size: u32,
}

#[repr(C)]
struct Mb2MmapTag {
    tag_type: u32,
    tag_size: u32,
    entry_size: u32,
    entry_version: u32,
}

#[repr(C)]
struct Mb2MmapEntry {
    base_addr: u64,
    length: u64,
    entry_type: u32,
    reserved: u32,
}

fn parse_multiboot2_memory_map(mbi_ptr: u32, regions: &mut [MemoryRegion]) -> usize {
    let boot_info = mbi_ptr as *const Mb2BootInfo;
    let total_size = unsafe { (*boot_info).total_size };
    let end = (mbi_ptr + total_size) as *const u8;
    let mut tag_ptr = (mbi_ptr + 8) as *const u8;
    let mut count = 0;

    while (tag_ptr as u32 + 8) <= (end as u32) {
        let tag = unsafe { &*(tag_ptr as *const Mb2Tag) };
        if tag.tag_type == 0 {
            break;
        }
        if tag.tag_type == 6 {
            let mmap = unsafe { &*(tag_ptr as *const Mb2MmapTag) };
            let entry_size = mmap.entry_size as usize;
            let entries_byte_count = (mmap.tag_size as usize).saturating_sub(size_of::<Mb2MmapTag>());
            let entries_start = unsafe { tag_ptr.add(size_of::<Mb2MmapTag>()) };
            let mut offset: usize = 0;

            while offset + entry_size <= entries_byte_count && count < regions.len() {
                let entry = unsafe { &*((entries_start as usize + offset) as *const Mb2MmapEntry) };
                let kind = match entry.entry_type {
                    1 => MemoryKind::Usable,
                    3 => MemoryKind::AcpiReclaimable,
                    4 => MemoryKind::AcpiNvs,
                    _ => MemoryKind::Reserved,
                };
                regions[count] = MemoryRegion {
                    start: entry.base_addr,
                    end: entry.base_addr + entry.length,
                    kind,
                };
                count += 1;
                offset += entry_size;
            }
        }
        let aligned_size = ((tag.tag_size + 7) & !7) as usize;
        tag_ptr = unsafe { tag_ptr.add(aligned_size) };
    }

    count
}

#[repr(C)]
struct Mb2BootInfo {
    total_size: u32,
    reserved: u32,
}

/// Точка входа ядра из assembly (_hammam_entry).
#[no_mangle]
pub extern "C" fn _start_multiboot2(magic: u32, mbi_ptr: u32) -> ! {
    unsafe { drivers::serial::SERIAL.get().init(); }

    kprintln!("Hammam / PINDOS booting...");
    kprintln!("magic = {:#x}", magic);
    kprintln!("mbi   = {:#x}", mbi_ptr);

    arch::x86_64::gdt::init();
    kprintln!("[OK] GDT initialized");

    security::enable_smep_smap();
    security::enable_nx();
    security::init_canary();
    kprintln!("[OK] Security features enabled (SMEP/SMAP/NX/canary)");

    let mut regions = [MemoryRegion {
        start: 0,
        end: 0,
        kind: MemoryKind::Reserved,
    }; MAX_MEMORY_REGIONS];

    let region_count = parse_multiboot2_memory_map(mbi_ptr, &mut regions);
    kprintln!("[OK] Multiboot2 memory map: {} regions", region_count);

    if region_count > 0 {
        unsafe { PHYSICAL_ALLOCATOR.lock().init(&regions[..region_count]); }
        kprintln!("[OK] Buddy allocator initialized");
    }

    unsafe { mm::HEAP_ALLOCATOR.init(); }
    kprintln!("[OK] Heap allocator initialized");

    let v: Vec<u32> = vec![1, 2, 3];
    kprintln!("heap test: {:?}", v);

    kprintln!("Boot sequence complete. Halting.");
    loop {
        unsafe { core::arch::asm!("hlt", options(nostack)); }
    }
}

/// Обработчик паник ядра Hammam.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    kprintln!("\n!!! KERNEL PANIC !!!");
    if let Some(location) = info.location() {
        kprintln!("Location: {}:{}:{}", location.file(), location.line(), location.column());
    }
    kprintln!("Message: {}", info);
    kprintln!("====================================================");

    loop {
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

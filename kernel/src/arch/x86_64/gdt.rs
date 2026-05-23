// x86_64 Global Descriptor Table implementation
use core::arch::asm;

const GDT_ENTRIES: usize = 6;

#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

#[repr(C, packed)]
struct GdtDescriptor {
    limit: u16,
    base: u64,
}

static mut GDT: [GdtEntry; GDT_ENTRIES] = [
    // Null descriptor (required by CPU)
    GdtEntry {
        limit_low: 0,
        base_low: 0,
        base_middle: 0,
        access: 0,
        granularity: 0,
        base_high: 0,
    },
    // Kernel code segment (selector 0x08)
    GdtEntry {
        limit_low: 0xFFFF,
        base_low: 0,
        base_middle: 0,
        access: 0x9A, // Present, DPL=0, Code/Data, Execute/Read
        granularity: 0xA0, // 64-bit code segment
        base_high: 0,
    },
    // Kernel data segment (selector 0x10)
    GdtEntry {
        limit_low: 0xFFFF,
        base_low: 0,
        base_middle: 0,
        access: 0x92, // Present, DPL=0, Data/Writable
        granularity: 0xA0,
        base_high: 0,
    },
    // User code segment (selector 0x18)
    GdtEntry {
        limit_low: 0xFFFF,
        base_low: 0,
        base_middle: 0,
        access: 0xFA, // Present, DPL=3, Code/Data, Execute/Read
        granularity: 0xA0,
        base_high: 0,
    },
    // User data segment (selector 0x20)
    GdtEntry {
        limit_low: 0xFFFF,
        base_low: 0,
        base_middle: 0,
        access: 0xF2, // Present, DPL=3, Data/Writable
        granularity: 0xA0,
        base_high: 0,
    },
    // TSS descriptor (selector 0x28)
    GdtEntry {
        limit_low: 0,
        base_low: 0,
        base_middle: 0,
        access: 0x89, // Present, DPL=0, TSS (64-bit)
        granularity: 0,
        base_high: 0,
    },
];

static mut GDTR: GdtDescriptor = GdtDescriptor {
    limit: (core::mem::size_of::<GdtEntry>() * GDT_ENTRIES - 1) as u16,
    base: 0,
};

/// Initialize the Global Descriptor Table
pub fn init() {
    unsafe {
        // Get the address of the GDT
        let gdt_addr = GDT.as_ptr() as u64;
        GDTR.base = gdt_addr;
        GDTR.limit = (core::mem::size_of::<GdtEntry>() * GDT_ENTRIES - 1) as u16;
        
        // Load GDT using lgdt instruction
        asm!(
            "lgdt [{}]",
            "mov ax, 0x10",     // Load kernel data segment selector
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            "jmp 0x08:1f",      // Far jump to reload CS
            "1:",
            in(reg) &GDTR,
        );
    }
}

/// Reload segment registers (used after context switch)
#[inline]
pub fn reload_segments() {
    unsafe {
        asm!(
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
        );
    }
}
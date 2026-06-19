// Minimal IDT setup for early boot
#![allow(dead_code)]

#[repr(C, align(16))]
#[derive(Copy, Clone)]
struct IDTEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

#[repr(C, align(16))]
struct IDTR {
    limit: u16,
    base: u64,
}

static mut IDT: [IDTEntry; 256] = [IDTEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    flags: 0,
    offset_mid: 0,
    offset_high: 0,
    reserved: 0,
}; 256];

static mut IDTR: IDTR = IDTR { limit: 0, base: 0 };

extern "C" fn dummy_exception_handler() {
    unsafe {
        core::arch::asm!(
            "cli",
            "2:",
            "hlt",
            "jmp 2b",
            options(noreturn)
        );
    }
}

pub unsafe fn init_idt() {
    let handler_addr = dummy_exception_handler as *const () as u64;

    unsafe {
        for i in 0..256 {
            IDT[i] = IDTEntry {
                offset_low: (handler_addr & 0xFFFF) as u16,
                selector: 0x08,
                ist: 0,
                flags: 0x8E,
                offset_mid: ((handler_addr >> 16) & 0xFFFF) as u16,
                offset_high: ((handler_addr >> 32) & 0xFFFFFFFF) as u32,
                reserved: 0,
            };
        }

        IDTR.limit = (core::mem::size_of::<IDTEntry>() * 256 - 1) as u16;
        IDTR.base = IDT.as_ptr() as u64;

        core::arch::asm!(
            "lidt [{}]",
            in(reg) &raw const IDTR as *const IDTR,
            options(nostack, preserves_flags)
        );
    }
}

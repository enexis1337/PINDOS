// Architecture-specific bootstrap code
pub mod x86_64;
pub mod aarch64;

use core::arch::asm;

/// Halt the CPU
pub fn halt() {
    unsafe {
        asm!("hlt");
    }
}

/// Get current privilege level (CPL)
#[inline]
pub fn get_cpl() -> u8 {
    unsafe {
        let rflags: u64;
        asm!("pushfq; pop {}", out(reg) rflags);
        ((rflags >> 12) & 0x3) as u8
    }
}

/// Read current stack pointer
#[inline]
pub fn get_sp() -> usize {
    let sp: usize;
    unsafe {
        asm!("mov {}, rsp", out(reg) sp);
    }
    sp
}

/// Read current instruction pointer
#[inline]
pub fn get_ip() -> usize {
    let ip: usize;
    unsafe {
        asm!("lea {}, [rip]", out(reg) ip);
    }
    ip
}
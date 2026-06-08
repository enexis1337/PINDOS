#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::all)]

extern crate alloc;

pub mod boot;  // Boot header and entry point

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

/// Точка входа ядра из assembly (_hammam_entry).
/// Вызывается после установки стека и переключения в 64-битный режим.
#[no_mangle]
pub extern "C" fn _start_multiboot2(magic: u32, mbi_ptr: u32) -> ! {
    unsafe {
        core::arch::asm!("mov dx, 0x3F8", "mov al, 'E'", "out dx, al", options(nostack));
    }
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

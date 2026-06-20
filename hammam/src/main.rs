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
    // ── Шаг 0: инициализируем serial (COM1) ─────────────────────────────────
    // SAFETY: единственный вызов при старте, до любых других потоков.
    unsafe { drivers::serial::SERIAL.get().init(); }

    kprintln!("====================================================");
    kprintln!("  PINDOS Hammam Kernel — boot sequence");
    kprintln!("====================================================");
    kprintln!("[boot] magic = {:#010x}", magic);

    const MULTIBOOT2_MAGIC: u32 = 0x36d76289;
    if magic != MULTIBOOT2_MAGIC {
        panic!("invalid Multiboot2 magic: {:#010x}", magic);
    }
    kprintln!("[boot] Multiboot2 magic OK");

    // ── Шаг 1: GDT ──────────────────────────────────────────────────────────
    kprintln!("[step 1] init GDT...");
    // SAFETY: вызывается один раз при старте до включения прерываний.
    unsafe { arch::x86_64::gdt::init(); }
    kprintln!("[step 1] GDT OK");

    kprintln!("[boot] step 1 complete — halting");
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

// hammam/src/arch/x86_64/entry.rs

use core::arch::naked_asm;

#[repr(C, align(16))]
pub struct KernelStack([u8; 64 * 1024]); // 64 KiB стек ядра

#[used]
#[no_mangle]
#[link_section = ".data"]  // .data а не .bss — чтобы точно было в PT_LOAD
pub static KERNEL_STACK: KernelStack = KernelStack([0; 64 * 1024]);

/// Точка входа из boot.rs после переключения в 64-битный режим.
/// boot.rs передаёт: RAX=magic, RBX=mbi_ptr.
/// Устанавливает стек ядра и вызывает Rust entry point.
///
/// # Safety
/// Вызывается только из boot.rs после настройки 64-битного режима.
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn _hammam_entry() -> ! {
    naked_asm!(
        // RAX=magic, RBX=mbi_ptr — переданы из boot.rs

        // Отладка: мы вошли в _hammam_entry
        "mov dx, 0x3F8",
        "mov al, '6'",
        "out dx, al",

        // Установить свежий выровненный стек
        "lea rsp, [rip + KERNEL_STACK]",
        "add rsp, {stack_size}",
        "and rsp, -16",

        // Отладка: стек установлен
        "mov dx, 0x3F8",
        "mov al, '7'",
        "out dx, al",

        "mov rdi, rax",
        "mov rsi, rbx",

        // Отладка: перед call
        "mov dx, 0x3F8",
        "mov al, '8'",
        "out dx, al",

        "call _start_multiboot2",

        // Если вернулась (не должна) — halt
        "99:",
        "cli",
        "hlt",
        "jmp 99b",

        stack_size = const 64 * 1024usize,
    );
}

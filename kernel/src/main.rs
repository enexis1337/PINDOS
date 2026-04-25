#![no_std]
#![no_main]
#![feature(naked_functions)]

mod vga;
mod fs;
mod shell;
mod auth;
pub mod utils;
pub mod dos;
pub mod linux;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    vga::clear_screen();
    vga::print_colored("PINDOS init...\n", 0x08);

    dos::v86::init();
    fs::init();
    crate::linux::paging::init();

    // Первый запуск — настройка пользователей
    if auth::is_first_run() {
        auth::first_run_setup();
    }

    // Цикл логина
    loop {
        if auth::login() {
            shell::run_as(auth::current_name(), auth::current_is_su());
        }
        // login вернул false (3 неудачи) — повторяем
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga::print("\n[KERNEL PANIC] ");
    if let Some(msg) = info.message().as_str() {
        vga::print(msg);
    }
    loop {}
}

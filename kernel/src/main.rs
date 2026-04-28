#![no_std]
#![no_main]
// naked_functions стабильна с 1.88 — убираем feature флаг
// #![feature(naked_functions)]

mod vga;
mod fs;
mod fs_fat;
mod uglyshell;
use uglyshell as shell;
mod auth;
pub mod drivers;
pub mod utils;
pub mod dos;
pub mod linux;
pub mod mell;
pub mod dealduckd;
pub mod version;
pub mod bridgie;

use core::panic::PanicInfo;

fn print_u32(n: u32) {
    if n == 0 { crate::vga::put_char(b'0'); return; }
    let mut buf = [0u8; 10];
    let mut i = 0;
    let mut n = n;
    while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { crate::vga::put_char(buf[j]); }
}
// ИСПРАВЛЕНО: убран неиспользуемый use crate::linux::syscall

#[no_mangle]
pub extern "C" fn kernel_main(mb2_info: u32) -> ! {
    // Пишем прямо в VGA буфер ДО любой инициализации
    unsafe {
        let vga = 0xB8000 as *mut u8;
        *vga.add(0) = b'K'; *vga.add(1) = 0x0F;
        *vga.add(2) = b'1'; *vga.add(3) = 0x0F;
    }

    // ПЕРВЫМ ДЕЛОМ: загружаем IDT и перемаппируем PIC.
    dos::v86::init();

    unsafe {
        let vga = 0xB8000 as *mut u8;
        *vga.add(4) = b'K'; *vga.add(5) = 0x0F;
        *vga.add(6) = b'2'; *vga.add(7) = 0x0F;
    }

    vga::serial_init();
    vga::clear_screen();
    vga::print_colored("PINDOS init...\n", 0x08);

    // VESA framebuffer — инициализируем но НЕ переключаемся сразу
    // Сначала выводим диагностику в VGA текст
    drivers::vesa::probe_only(); // только определяем параметры, не включаем LFB

    // Диагностика framebuffer
    {
        let fb = drivers::vesa::get();
        vga::print_colored("fb: ", 0x08);
        if fb.ready {
            vga::print("0x");
            let addr = fb.addr;
            for i in (0..8).rev() {
                let nibble = ((addr >> (i * 4)) & 0xF) as u8;
                vga::put_char(if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 });
            }
            vga::print(" ");
            print_u32(fb.width); vga::print("x");
            print_u32(fb.height as u32); vga::print("x");
            print_u32(fb.bpp as u32);
            vga::print(" pitch="); print_u32(fb.pitch);
            vga::print_colored(" OK\n", 0x0A);
        } else {
            vga::print_colored("fb: NOT FOUND\n", 0x0C);
        }
    }

    // PS/2 — может зависнуть на системах без контроллера, оборачиваем
    vga::print_colored("ps2...\n", 0x08);
    drivers::ps2::init();

    // ATA — сканируем диски
    vga::print_colored("ata...\n", 0x08);
    drivers::ata::init();

    // USB — только если есть PCI
    vga::print_colored("usb...\n", 0x08);
    drivers::usb::scan_pci(); // только сканирование, без инициализации UHCI
    drivers::ehci::init_from_pci();

    // Монтируем FAT разделы
    for i in 0..drivers::ata::disk_count() {
        if let Some(disk) = drivers::ata::get_disk(i) {
            let letter = b'C' + i as u8;
            if !fs_fat::file::mount_volume(disk, 0, letter) {
                let mut mbr = [0u8; 512];
                if drivers::ata::read_sectors(disk, 0, 1, &mut mbr).is_ok() {
                    for p in 0..4 {
                        let off = 0x1BE + p * 16;
                        let ptype = mbr[off + 4];
                        if matches!(ptype, 0x0B | 0x0C | 0x06 | 0x04 | 0x0E) {
                            let lba = u32::from_le_bytes([
                                mbr[off+8], mbr[off+9], mbr[off+10], mbr[off+11]
                            ]);
                            fs_fat::file::mount_volume(disk, lba as u64, letter);
                            break;
                        }
                    }
                }
            }
        }
    }

    fs::init();

    // Инициализируем планировщик задач
    dealduckd::init();
    // Инициализируем пакетный менеджер
    bridgie::registry::init();

    vga::print_colored("PINDOS ready.\n", 0x0A);

    // Загрузочная мелодия
    drivers::speaker::play_melody(drivers::speaker::MELODY_BOOT);

    if auth::is_first_run() {
        auth::drun();
    }

    loop {
        if auth::login() {
            shell::run_as(auth::current_name(), auth::current_is_su());
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Выводим панику и ЗАВИСАЕМ — не ребутимся
    vga::print_colored("\n\n[KERNEL PANIC]\n", 0x0C);
    if let Some(msg) = info.message().as_str() {
        vga::print(msg);
    }
    if let Some(loc) = info.location() {
        vga::print("\nat ");
        vga::print(loc.file());
        vga::print(":");
        // Выводим номер строки
        let line = loc.line();
        let mut buf = [0u8; 10];
        let mut i = 0;
        let mut n = line;
        if n == 0 { buf[0] = b'0'; i = 1; }
        while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        for j in (0..i).rev() { vga::put_char(buf[j]); }
    }
    vga::print_colored("\nSystem halted. Reset manually.\n", 0x0C);
    // Бесконечный цикл — НЕ возвращаемся, НЕ ребутимся
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

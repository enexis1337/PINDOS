// fastfetch — системная информация с ASCII артом

use crate::vga;
use crate::fs;
use crate::auth;

const LOGO: &[&str] = &[
r":.                                   .:.                                          ",
r"  .@*=@*.                              +@*=@:                                     ",
r"  @+  -%%-                         -%%=  -@:                                      ",
r"  **     *@+     ..:-===-:.     .#@*     *%.                                      ",
r"  -%      .=@*%%*=:::...::-=#%#*@:      .%*                                       ",
r"   .@     :*@%:                 :#@*:    :@:                                       ",
r"   .%=  =@@+                       :@@+  =%                                        ",
r"     +@@@+.                           :@@@@=                                       ",
r"      -%:                               *#.                                        ",
r"      =@.                                 +%.                                      ",
r"     .%.                                   **                                      ",
r"     #=                                    :%:                                     ",
r"    :@:                                     #%@@=                                  ",
r"      -%%%@*+=:.                          .-*#%#=##                                ",
r"          :@:  :=*#%%%#*:              -#%*=.     **                               ",
r"           %=                                    .%-                               ",
r"       ###*#@*+==----=++:             .==---:::::*%--===:                          ",
r"       **      ..:::             .--::..   :%:                                     ",
r"          %%%@@%*=:..                 ..:=+%@@*.                                   ",
r"    =*%%*=:=%:                            .*%..-*%%:                               ",
r"              .##:                        .=%-                                     ",
r"                .*@=                    :#%:                                       ",
r"                   :*%*=:          .-*%#=.                                         ",
r"                       -*%%%%%%%%*=:                                               ",
];

pub fn run() {
    vga::clear_screen();

    let user   = auth::current_name();
    let os     = "PINDOS 0.1";
    let kernel = "pindos-kernel 0.1.0";
    let shell  = "pindosh";
    let arch   = "i686";
    let cpu    = "i686 (32-bit protected mode)";
    let mem    = "32MB total";

    let mut files = 0usize;
    let mut dirs  = 0usize;
    let mut used  = 0usize;
    for e in fs::list() {
        if e.is_dir() { dirs += 1; }
        else { files += 1; used += e.content_len; }
    }

    // Заголовок
    vga::print_colored(user, 0x0E);
    vga::print_colored("@pindos\n", 0x0E);
    // Подчёркивание длиной user + "@pindos"
    let sep_len = auth::current_name().len() + 7;
    for _ in 0..sep_len { vga::print_colored("-", 0x08); }
    vga::put_char(b'\n');

    // Лого
    for line in LOGO.iter() {
        vga::print_colored(line, 0x0A);
        vga::put_char(b'\n');
    }

    vga::put_char(b'\n');

    // Инфо
    print_info("OS",     os);
    print_info("Kernel", kernel);
    print_info("Shell",  shell);
    print_info("Arch",   arch);
    print_info("CPU",    cpu);
    print_info("Memory", mem);

    // FS статистика
    vga::print_colored("Files", 0x0B);
    vga::print_colored(": ", 0x08);
    print_usize(files);
    vga::print_colored(" files  ", 0x0F);
    print_usize(dirs);
    vga::print_colored(" dirs  ", 0x0F);
    print_usize(used);
    vga::print_colored(" bytes\n", 0x0F);

    // Цветовая палитра
    vga::put_char(b'\n');
    for c in 0u8..8  { vga::print_colored("   ", c); }
    vga::put_char(b'\n');
    for c in 8u8..16 { vga::print_colored("   ", c); }
    vga::put_char(b'\n');
}

fn print_info(label: &str, value: &str) {
    vga::print_colored(label, 0x0B);
    vga::print_colored(": ", 0x08);
    vga::print_colored(value, 0x0F);
    vga::put_char(b'\n');
}

fn print_usize(n: usize) {
    if n == 0 { vga::put_char(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    let mut n = n;
    while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { vga::put_char(buf[j]); }
}

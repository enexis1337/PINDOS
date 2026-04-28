// fastfetch — системная информация с ASCII артом

use crate::vga;
use crate::fs;
use crate::auth;
                            
const LOGO: &[&str] = &[
    r"   ....                   ...",            
    r"   .+@%=.               .=**%",
    r"   .+@@@@+. ..:---:. .-*=:.++",
    r"   .=@@@@@@@@+#.%--#:#*.   #:",
    r"    -%@@@@@@:-#.#=-+   -%+:#" ,
    r"    .=@@@*:   :..:.     .=+." ,
    r"    :+..                  ==" ,
    r"   .=-                    .#...",
    r"  .#+*:::...   .::..  ..::=%%-.",
    r"   .=:...-+*: .@@*. .+-....*:",
    r".-=**=---==.  .=.  .=----=*--=..",
    r"    .**%#+-:.       .::-+#%:",
    r" .+:..:#..             .-*. .:.",
    r"      .-+=:        .-++.",
    r"         .:========-..",
];

pub fn run() {
    vga::clear_screen();
    let shell  = "uglyshell (ush)";
    let user   = auth::current_name();
    let os     = crate::version::OS_FULL;
    let kernel = crate::version::KERNEL_FULL;
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
    vga::print_colored("@", 0x0E);
    vga::print_colored(crate::auth::get_hostname(), 0x0E);
    vga::put_char(b'\n');
    // Подчёркивание
    let sep_len = auth::current_name().len() + 1 + crate::auth::get_hostname().len();
    for _ in 0..sep_len { vga::print_colored("-", 0x08); }
    vga::put_char(b'\n');

    // Подготавливаем информационные строки
    let info_lines = [
        ("OS",       os),
        ("Kernel",   kernel),
        ("Host",     crate::auth::get_hostname()),
        ("Shell",    shell),
        ("Arch",     arch),
        ("CPU",      cpu),
        ("Memory",   mem),
    ];

    // Выводим лого и информацию параллельно
    let max_lines = LOGO.len().max(info_lines.len() + 1); // +1 для строки с файлами
    
    for i in 0..max_lines {
        // Выводим строку ASCII арта (если есть)
        if i < LOGO.len() {
            vga::print_colored(LOGO[i], 0x0A);
        } else {
            // Пустая строка
            for _ in 0..40 { vga::put_char(b' '); }
        }
        
        // Добавляем отступ между артом и информацией
        vga::print("  ");
        
        // Выводим информационную строку (если есть)
        if i < info_lines.len() {
            let (label, value) = info_lines[i];
            vga::print_colored(label, 0x0B);
            vga::print_colored(": ", 0x08);
            vga::print_colored(value, 0x0F);
        } else if i == info_lines.len() {
            // Строка с файлами
            vga::print_colored("Files", 0x0B);
            vga::print_colored(": ", 0x08);
            print_usize(files);
            vga::print_colored(" files, ", 0x0F);
            print_usize(dirs);
            vga::print_colored(" dirs, ", 0x0F);
            print_usize(used);
            vga::print_colored(" bytes", 0x0F);
        }
        
        vga::put_char(b'\n');
    }

    vga::put_char(b'\n');

    // Цветовая палитра
    for c in 0u8..8  { vga::print_colored("   ", c); }
    vga::put_char(b'\n');
    for c in 8u8..16 { vga::print_colored("   ", c); }
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

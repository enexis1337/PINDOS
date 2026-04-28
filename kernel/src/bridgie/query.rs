// Запросы к реестру: list, search, info

use super::package::PackageState;
use super::registry;

/// bridgie list — показать установленные пакеты
pub fn cmd_list(_args: &str) {
    let reg = registry::get();
    let mut found = false;

    crate::vga::print_colored("Installed packages:\n", 0x0E);
    crate::vga::print("------------------------------------\n");

    for i in 0..reg.count {
        let pkg = &reg.packages[i];
        if pkg.state == PackageState::Installed {
            found = true;
            let status = match pkg.state {
                PackageState::Installed => "[installed]",
                PackageState::Broken    => "[broken]   ",
                PackageState::Available => "[available]",
            };
            crate::vga::print_colored(status, 0x0A);
            crate::vga::print(" ");
            crate::vga::print(pkg.meta.name_str());
            crate::vga::print(" ");
            crate::vga::print_colored(pkg.meta.version_str(), 0x08);
            crate::vga::put_char(b'\n');
        }
    }

    if !found {
        crate::vga::print("No packages installed.\n");
        crate::vga::print("Run 'bridgie search <name>' to find packages.\n");
    }
}

/// bridgie search <query> — поиск по реестру
pub fn cmd_search(args: &str) {
    let query = args.trim();
    let reg = registry::get();
    let mut found = false;

    if query.is_empty() {
        // Показать все доступные
        crate::vga::print_colored("Available packages:\n", 0x0E);
        crate::vga::print("------------------------------------\n");
        for i in 0..reg.count {
            print_pkg_line(&reg.packages[i]);
            found = true;
        }
    } else {
        crate::vga::print_colored("Search results for '", 0x0E);
        crate::vga::print_colored(query, 0x0E);
        crate::vga::print_colored("':\n", 0x0E);
        crate::vga::print("------------------------------------\n");
        for i in 0..reg.count {
            let pkg = &reg.packages[i];
            if pkg.meta.name_str().contains(query) || pkg.meta.desc_str().contains(query) {
                print_pkg_line(pkg);
                found = true;
            }
        }
    }

    if !found {
        crate::vga::print("No packages found");
        if !query.is_empty() {
            crate::vga::print(" matching '");
            crate::vga::print(query);
            crate::vga::print("'");
        }
        crate::vga::put_char(b'\n');
    }
}

/// bridgie info <name> — подробная информация о пакете
pub fn cmd_info(args: &str) {
    let name = args.trim();
    if name.is_empty() {
        crate::vga::print("Usage: bridgie info <package>\n");
        return;
    }

    let reg = registry::get();
    match reg.find(name) {
        None => {
            crate::vga::print("bridgie: package '");
            crate::vga::print(name);
            crate::vga::print("' not found\n");
        }
        Some(idx) => {
            let pkg = &reg.packages[idx];
            crate::vga::print_colored("Package:     ", 0x0B);
            crate::vga::print(pkg.meta.name_str());
            crate::vga::put_char(b'\n');

            crate::vga::print_colored("Version:     ", 0x0B);
            crate::vga::print(pkg.meta.version_str());
            crate::vga::put_char(b'\n');

            crate::vga::print_colored("Status:      ", 0x0B);
            let (status, color) = match pkg.state {
                PackageState::Installed => ("installed", 0x0Au8),
                PackageState::Available => ("available", 0x07u8),
                PackageState::Broken    => ("broken",    0x0Cu8),
            };
            crate::vga::print_colored(status, color);
            crate::vga::put_char(b'\n');

            crate::vga::print_colored("Size:        ", 0x0B);
            print_size(pkg.meta.size_kb);
            crate::vga::put_char(b'\n');

            crate::vga::print_colored("Description: ", 0x0B);
            crate::vga::print(pkg.meta.desc_str());
            crate::vga::put_char(b'\n');
        }
    }
}

fn print_pkg_line(pkg: &super::package::Package) {
    let (marker, color) = match pkg.state {
        PackageState::Installed => ("[I]", 0x0Au8),
        PackageState::Available => ("[ ]", 0x07u8),
        PackageState::Broken    => ("[!]", 0x0Cu8),
    };
    crate::vga::print_colored(marker, color);
    crate::vga::print(" ");
    crate::vga::print(pkg.meta.name_str());
    // Выравниваем по 16 символов
    let pad = 16usize.saturating_sub(pkg.meta.name_len);
    for _ in 0..pad { crate::vga::put_char(b' '); }
    crate::vga::print_colored(pkg.meta.version_str(), 0x08);
    crate::vga::print("  ");
    crate::vga::print(pkg.meta.desc_str());
    crate::vga::put_char(b'\n');
}

fn print_size(kb: u32) {
    if kb >= 1024 {
        print_u32(kb / 1024);
        crate::vga::print(" MB");
    } else {
        print_u32(kb);
        crate::vga::print(" KB");
    }
}

fn print_u32(n: u32) {
    if n == 0 { crate::vga::put_char(b'0'); return; }
    let mut buf = [0u8; 10];
    let mut i = 0;
    let mut n = n;
    while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { crate::vga::put_char(buf[j]); }
}

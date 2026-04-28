// Установка пакетов

use super::package::PackageState;
use super::registry;

pub fn cmd_install(args: &str) {
    let name = args.trim();
    if name.is_empty() {
        crate::vga::print("Usage: bridgie install <package>\n");
        return;
    }

    let reg = registry::get();

    match reg.find(name) {
        None => {
            crate::vga::print("bridgie: package '");
            crate::vga::print(name);
            crate::vga::print("' not found in registry\n");
            crate::vga::print("Hint: run 'bridgie search ");
            crate::vga::print(name);
            crate::vga::print("' to search\n");
        }
        Some(idx) => {
            let pkg = &mut reg.packages[idx];
            match pkg.state {
                PackageState::Installed => {
                    crate::vga::print("bridgie: '");
                    crate::vga::print(pkg.meta.name_str());
                    crate::vga::print("' is already installed (");
                    crate::vga::print(pkg.meta.version_str());
                    crate::vga::print(")\n");
                }
                PackageState::Available | PackageState::Broken => {
                    crate::vga::print("bridgie: installing '");
                    crate::vga::print(pkg.meta.name_str());
                    crate::vga::print("' ");
                    crate::vga::print(pkg.meta.version_str());
                    crate::vga::print(" (");
                    print_size(pkg.meta.size_kb);
                    crate::vga::print(")...\n");

                    // TODO: реальная загрузка и распаковка пакета
                    // Сейчас просто меняем статус
                    pkg.state = PackageState::Installed;

                    crate::vga::print_colored("bridgie: '", 0x0A);
                    crate::vga::print_colored(pkg.meta.name_str(), 0x0A);
                    crate::vga::print_colored("' installed successfully\n", 0x0A);
                }
            }
        }
    }
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

// Удаление пакетов

use super::package::PackageState;
use super::registry;

pub fn cmd_remove(args: &str) {
    let name = args.trim();
    if name.is_empty() {
        crate::vga::print("Usage: bridgie remove <package>\n");
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
            let pkg = &mut reg.packages[idx];
            match pkg.state {
                PackageState::Available => {
                    crate::vga::print("bridgie: '");
                    crate::vga::print(pkg.meta.name_str());
                    crate::vga::print("' is not installed\n");
                }
                PackageState::Installed | PackageState::Broken => {
                    crate::vga::print("bridgie: removing '");
                    crate::vga::print(pkg.meta.name_str());
                    crate::vga::print("'...\n");

                    // TODO: реальное удаление файлов пакета
                    pkg.state = PackageState::Available;

                    crate::vga::print_colored("bridgie: '", 0x0C);
                    crate::vga::print_colored(pkg.meta.name_str(), 0x0C);
                    crate::vga::print_colored("' removed\n", 0x0C);
                }
            }
        }
    }
}

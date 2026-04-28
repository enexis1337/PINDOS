// Реестр пакетов — хранит список доступных и установленных пакетов

use super::package::{Package, PackageMeta, PackageState};

pub const MAX_PACKAGES: usize = 64;

pub struct Registry {
    pub packages: [Package; MAX_PACKAGES],
    pub count:    usize,
}

// Глобальный реестр
static mut REGISTRY: Registry = Registry {
    packages: [Package::empty(); MAX_PACKAGES],
    count:    0,
};

impl Registry {
    /// Инициализация реестра со встроенными пакетами-заготовками
    pub fn init(&mut self) {
        self.count = 0;
        // Встроенные пакеты-заглушки (будущие компоненты системы)
        let builtins: &[(&str, &str, &str, u32)] = &[
            ("coreutils",  "1.0.0", "Basic file and shell utilities",        12),
            ("libc-pindos","0.1.0", "PINDOS C runtime stub library",         8),
            ("sh-posix",   "0.2.0", "POSIX-compatible shell extensions",     4),
            ("net-tools",  "0.1.0", "Network utilities (ping, ifconfig)",    6),
            ("vim-nano",   "0.1.0", "Minimal text editor (nano-like)",       10),
            ("python3",    "3.11.0","Python 3 interpreter (stub)",           512),
            ("gcc-i686",   "13.0.0","GCC cross-compiler for i686",           2048),
            ("busybox",    "1.36.0","Tiny versions of common UNIX utilities", 256),
        ];
        for &(name, ver, desc, size) in builtins {
            if self.count < MAX_PACKAGES {
                self.packages[self.count] = Package {
                    meta:  PackageMeta::from_strs(name, ver, desc, size),
                    state: PackageState::Available,
                };
                self.count += 1;
            }
        }
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        for i in 0..self.count {
            if self.packages[i].meta.name_str() == name {
                return Some(i);
            }
        }
        None
    }

    pub fn add(&mut self, meta: PackageMeta, state: PackageState) -> bool {
        if self.count >= MAX_PACKAGES { return false; }
        self.packages[self.count] = Package { meta, state };
        self.count += 1;
        true
    }
}

// ── Глобальный интерфейс ──────────────────────────────────────────────────

pub fn init() {
    unsafe { REGISTRY.init(); }
    crate::vga::print_colored("bridgie: registry initialized\n", 0x08);
}

pub fn get() -> &'static mut Registry {
    unsafe { &mut REGISTRY }
}

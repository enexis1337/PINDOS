// Bridgie — пакетный менеджер PINDOS
// Управляет установкой, удалением и обновлением пакетов

pub mod package;
pub mod registry;
pub mod install;
pub mod remove;
pub mod query;

pub use package::{Package, PackageState, PackageMeta};
pub use registry::Registry;

// ── Версия Bridgie ────────────────────────────────────────────────────────

pub const BRIDGIE_VERSION: &str = "0.1.0";

// ── Точка входа из шелла ──────────────────────────────────────────────────

/// bridgie <command> [args]
pub fn run(args: &str) {
    let (cmd, rest) = crate::uglyshell::split_first_pub(args);
    match cmd {
        "install" | "i"   => { if check_root() { install::cmd_install(rest); } }
        "remove"  | "rm"  => { if check_root() { remove::cmd_remove(rest); } }
        "update"          => { if check_root() { cmd_update(rest); } }
        "list"    | "ls"  => { if check_root() { query::cmd_list(rest); } }
        "search"  | "s"   => { if check_root() { query::cmd_search(rest); } }
        "info"            => query::cmd_info(rest),
        "version" | "-v"  => {
            crate::vga::print("Bridgie ");
            crate::vga::print(BRIDGIE_VERSION);
            crate::vga::print(" - PINDOS package manager\n");
        }
        "help" | "" => print_help(),
        _ => {
            crate::vga::print("bridgie: unknown command '");
            crate::vga::print(cmd);
            crate::vga::print("'\nRun 'bridgie help' for usage.\n");
        }
    }
}

/// Проверяет что текущий пользователь — root или имеет SU привилегии.
/// Возвращает true если доступ разрешён, иначе печатает ошибку и false.
fn check_root() -> bool {
    if crate::auth::is_su() || crate::auth::current_name() == "root" {
        return true;
    }
    crate::vga::print_colored("bridgie: ", 0x0C);
    crate::vga::print_colored("permission denied\n", 0x0C);
    crate::vga::print("This command requires root privileges.\n");
    crate::vga::print("Try: sudo bridgie <command>\n");
    false
}

fn cmd_update(_args: &str) {
    crate::vga::print("bridgie: update — refreshing package registry...\n");
    // TODO: загрузка индекса с сервера / диска
    crate::vga::print("bridgie: registry is up to date (stub)\n");
}

fn print_help() {
    crate::vga::print_colored("Bridgie ", 0x0B);
    crate::vga::print(BRIDGIE_VERSION);
    crate::vga::print(" — PINDOS package manager\n\n");
    crate::vga::print("Usage: bridgie <command> [args]\n\n");
    crate::vga::print_colored("Commands:\n", 0x0E);
    crate::vga::print("  install <name>   Install a package\n");
    crate::vga::print("  remove  <name>   Remove a package\n");
    crate::vga::print("  update           Refresh package registry\n");
    crate::vga::print("  list             List installed packages\n");
    crate::vga::print("  search  <query>  Search available packages\n");
    crate::vga::print("  info    <name>   Show package details\n");
    crate::vga::print("  version          Show Bridgie version\n");
    crate::vga::print("  help             Show this help\n");
}

// PINDOS auth — пользователи, пароли, сессии

use crate::vga;

pub const MAX_USERS:    usize = 16;
pub const MAX_USERNAME: usize = 32;
pub const MAX_PASSWORD: usize = 64;

#[derive(Copy, Clone)]
pub struct User {
    pub name:     [u8; MAX_USERNAME],
    pub name_len: usize,
    pub pass:     [u8; MAX_PASSWORD],
    pub pass_len: usize,
    pub is_su:    bool,   // superuser (root-like)
    pub used:     bool,
}

impl User {
    const fn empty() -> Self {
        User {
            name: [0u8; MAX_USERNAME], name_len: 0,
            pass: [0u8; MAX_PASSWORD], pass_len: 0,
            is_su: false,
            used: false,
        }
    }
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("?")
    }
    fn pass_str(&self) -> &str {
        core::str::from_utf8(&self.pass[..self.pass_len]).unwrap_or("")
    }
}

static mut USERS: [User; MAX_USERS] = [User::empty(); MAX_USERS];
static mut USER_COUNT: usize = 0;

// Текущий залогиненный пользователь
static mut CURRENT_USER: usize = 0;
static mut LOGGED_IN: bool = false;

// Флаг первого запуска
static mut FIRST_RUN: bool = true;

// Hostname системы
const MAX_HOSTNAME: usize = 64;
static mut HOSTNAME: [u8; MAX_HOSTNAME] = *b"pindos\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
static mut HOSTNAME_LEN: usize = 6;

pub fn get_hostname() -> &'static str {
    unsafe { core::str::from_utf8(&HOSTNAME[..HOSTNAME_LEN]).unwrap_or("pindos") }
}

pub fn set_hostname(name: &str) {
    unsafe {
        let b = name.as_bytes();
        let l = b.len().min(MAX_HOSTNAME);
        HOSTNAME[..l].copy_from_slice(&b[..l]);
        HOSTNAME_LEN = l;
    }
}

pub fn is_first_run() -> bool { unsafe { FIRST_RUN } }

pub fn current_user() -> &'static User {
    unsafe { &USERS[CURRENT_USER] }
}

pub fn current_is_su() -> bool {
    unsafe { USERS[CURRENT_USER].is_su }
}

pub fn is_su() -> bool {
    current_is_su()
}

pub fn current_name() -> &'static str {
    unsafe { USERS[CURRENT_USER].name_str() }
}

pub fn switch_user(username: &str) {
    unsafe {
        for (i, user) in USERS.iter().enumerate() {
            if user.used && user.name_str() == username {
                CURRENT_USER = i;
                return;
            }
        }
    }
}

pub fn switch_to_root() {
    unsafe {
        // Находим пользователя root
        for (i, user) in USERS.iter().enumerate() {
            if user.used && user.name_str() == "root" {
                CURRENT_USER = i;
                return;
            }
        }
    }
}

pub fn switch_back_from_root(orig_name: &str, _orig_is_su: bool) {
    unsafe {
        // Находим оригинального пользователя
        for (i, user) in USERS.iter().enumerate() {
            if user.used && user.name_str() == orig_name {
                CURRENT_USER = i;
                return;
            }
        }
    }
}

// ── Добавить пользователя ─────────────────────────────────────────────────

pub fn add_user(name: &str, pass: &str, is_su: bool) -> bool {
    unsafe {
        if USER_COUNT >= MAX_USERS { return false; }
        // Проверяем дубликат
        for u in USERS.iter() {
            if u.used && u.name_str() == name { return false; }
        }
        let slot = &mut USERS[USER_COUNT];
        slot.used = true;
        slot.is_su = is_su;
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_USERNAME);
        slot.name[..nl].copy_from_slice(&nb[..nl]);
        slot.name_len = nl;
        let pb = pass.as_bytes();
        let pl = pb.len().min(MAX_PASSWORD);
        slot.pass[..pl].copy_from_slice(&pb[..pl]);
        slot.pass_len = pl;
        USER_COUNT += 1;
        true
    }
}

pub fn delete_user(name: &str) -> bool {
    unsafe {
        for u in USERS.iter_mut() {
            if u.used && u.name_str() == name {
                *u = User::empty();
                return true;
            }
        }
        false
    }
}

pub fn list_users() {
    unsafe {
        vga::print_colored("USERNAME         SU\n", 0x0F);
        vga::print_colored("--------         --\n", 0x08);
        for u in USERS.iter() {
            if !u.used { continue; }
            vga::print(u.name_str());
            // выравнивание
            let pad = 17usize.saturating_sub(u.name_len);
            for _ in 0..pad { vga::put_char(b' '); }
            if u.is_su {
                vga::print_colored("yes\n", 0x0C);
            } else {
                vga::print("no\n");
            }
        }
    }
}

pub fn change_password(name: &str, new_pass: &str) -> bool {
    unsafe {
        for u in USERS.iter_mut() {
            if u.used && u.name_str() == name {
                let pb = new_pass.as_bytes();
                let pl = pb.len().min(MAX_PASSWORD);
                u.pass = [0u8; MAX_PASSWORD];
                u.pass[..pl].copy_from_slice(&pb[..pl]);
                u.pass_len = pl;
                return true;
            }
        }
        false
    }
}

fn print_banner() {
    vga::print_colored("_____ _____ _   _ _____   ____   _____\n", 0x0A);
    vga::print_colored("|  __ \\_   _| \\ | |  __ \\ / __ \\ / ____|\n", 0x0A);
    vga::print_colored("| |__) || | |  \\| | |  | | |  | | (___\n", 0x0A);
    vga::print_colored("|  ___/ | | | . ` | |  | | |  | |\\___ \\\n", 0x0A);
    vga::print_colored("| |    _| |_| |\\  | |__| | |__| |____) |\n", 0x0A);
    vga::print_colored("|_|   |_____|_| \\_|_____/ \\____/|_____/\n", 0x0A);
}

fn print_divider() {
    vga::print_colored("----------------------------------------\n", 0x08);
}

// ── Первый запуск ─────────────────────────────────────────────────────────

pub fn drun() {
    vga::clear_screen();
    print_banner();
    vga::print_colored("=============[", 0x0E);
    vga::print_colored(crate::version::OS_FULL, 0x0E);
    vga::print_colored("]=============\n", 0x0E);
    print_divider();
    vga::print_colored("           First Drun Setup\n", 0x0F);
    print_divider();
    vga::put_char(b'\n');

    // Hostname
    // print_divider();
    // vga::print_colored("        System hostname\n", 0x0F);
    // print_divider();
    vga::print_colored("Hostname [enter=pindos]: ", 0x0B);
    vga::sync_hw_cursor();
    let hn_input = crate::uglyshell::read_line_no_prompt();
    let hn = hn_input.as_str().trim();
    if !hn.is_empty() {
        set_hostname(hn);
        vga::print_colored("[+] Hostname set to '", 0x0A);
        vga::print_colored(hn, 0x0E);
        vga::print_colored("'\n", 0x0A);
    } else {
        vga::print_colored("[+] Hostname: pindos (default)\n", 0x08);
    }
    vga::put_char(b'\n');

    // Root пароль
    vga::print_colored("[ROOT] ", 0x0C);
    vga::print_colored("Set root password: ", 0x0F);
    let root_pass = read_password();
    vga::print_colored("[ROOT] ", 0x0C);
    vga::print_colored("Confirm password:  ", 0x0F);
    let root_pass2 = read_password();

    if root_pass.as_str() != root_pass2.as_str() {
        vga::print_colored("[!] Passwords do not match! Using 'root'\n", 0x0C);
        add_user("root", "root", true);
    } else {
        add_user("root", root_pass.as_str(), true);
    }
    vga::print_colored("[+] Root account created.\n", 0x0A);
    vga::put_char(b'\n');

    // Новый пользователь
    // print_divider();
    // vga::print_colored("        Create a new user\n", 0x0F);
    // print_divider();
    vga::print_colored("Username: ", 0x0B);
    vga::sync_hw_cursor();
    let username = crate::uglyshell::read_line_no_prompt();
    let username = username.as_str().trim();

    if !username.is_empty() {
        vga::print_colored("Password: ", 0x0B);
        let pass = read_password();
        vga::print_colored("Confirm:  ", 0x0B);
        let pass2 = read_password();

        vga::print_colored("Superuser (su) privileges? [y/N]: ", 0x0B);
        let su_ans = vga::read_char();
        vga::put_char(su_ans);
        vga::put_char(b'\n');
        let is_su = su_ans == b'y' || su_ans == b'Y';

        if pass.as_str() != pass2.as_str() {
            vga::print_colored("[!] Passwords do not match! Using empty password.\n", 0x0C);
            add_user(username, "", is_su);
        } else {
            add_user(username, pass.as_str(), is_su);
        }
        vga::put_char(b'\n');
        vga::print_colored("[+] User '", 0x0A);
        vga::print_colored(username, 0x0E);
        vga::print_colored("' created", 0x0A);
        if is_su { vga::print_colored(" [SU]", 0x0C); }
        vga::print_colored(".\n", 0x0A);
    } else {
        vga::print_colored("[-] Skipped user creation.\n", 0x08);
    }

    unsafe { FIRST_RUN = false; }
    vga::put_char(b'\n');
    print_divider();
    vga::print_colored(" Setup complete. Press any key to login...\n", 0x0E);
    print_divider();
    vga::read_char();
}

// ── Логин ─────────────────────────────────────────────────────────────────

/// Возвращает true если логин успешен
pub fn login() -> bool {
    vga::clear_screen();
    print_banner();
    vga::print_colored("=============[", 0x0E);
    vga::print_colored(crate::version::OS_FULL, 0x0E);
    vga::print_colored("]=============\n", 0x0E);
    print_divider();
    vga::print_colored("         Welcome to PINDOS!\n", 0x0F);
    print_divider();
    vga::put_char(b'\n');

    // 3 попытки
    for attempt in 0..3u8 {
        if attempt > 0 {
            vga::print_colored("[!] Login incorrect. ", 0x0C);
            vga::print_colored("Attempts left: ", 0x08);
            vga::put_char(b'0' + (3 - attempt));
            vga::put_char(b'\n');
            vga::put_char(b'\n');
        }
        vga::print_colored("login:    ", 0x0B);
        vga::sync_hw_cursor();
        let username = crate::uglyshell::read_line_no_prompt();
        let username = username.as_str().trim();

        vga::print_colored("password: ", 0x0B);
        let password = read_password();
        let password = password.as_str();

        if try_login(username, password) {
            vga::put_char(b'\n');
            print_divider();
            vga::print_colored("  Welcome to PINDOS, ", 0x0A);
            vga::print_colored(username, 0x0E);
            vga::print_colored("!\n", 0x0A);
            print_divider();
            vga::put_char(b'\n');
            return true;
        }
    }

    vga::print_colored("[!] Too many failed attempts.\n", 0x0C);
    false
}

fn try_login(name: &str, pass: &str) -> bool {
    unsafe {
        for (i, u) in USERS.iter().enumerate() {
            if u.used && u.name_str() == name && u.pass_str() == pass {
                CURRENT_USER = i;
                LOGGED_IN = true;
                return true;
            }
        }
        false
    }
}

// ── Чтение пароля (без эха) ───────────────────────────────────────────────

pub fn read_password() -> crate::uglyshell::InputBuf {
    let mut buf = crate::uglyshell::InputBuf::new();
    loop {
        let c = vga::read_char();
        match c {
            b'\n' => { vga::put_char(b'\n'); break; }
            b'\x08' => {
                if buf.len > 0 {
                    buf.len -= 1;
                    vga::put_char(b'\x08');
                }
            }
            _ => {
                if buf.len < crate::uglyshell::MAX_INPUT - 1 {
                    buf.push(c);
                    vga::put_char(b'*');
                }
            }
        }
    }
    buf
}

pub fn verify_user(name: &str, pass: &str) -> bool {
    unsafe {
        for u in USERS.iter() {
            if u.used && u.name_str() == name && u.pass_str() == pass {
                return true;
            }
        }
        false
    }
}

pub fn user_is_su(name: &str) -> bool {
    unsafe {
        for u in USERS.iter() {
            if u.used && u.name_str() == name {
                return u.is_su;
            }
        }
        false
    }
}

/// Вывод списка пользователей в GUI (по координатам VGA)
pub fn list_users_at(x: usize, y: usize) {
    use crate::mell::vga_gui::{put_str_at, put_char_at, LGRAY, BLACK, LRED};
    unsafe {
        let mut row = 0;
        for u in USERS.iter() {
            if !u.used { continue; }
            put_str_at(x, y + row, u.name_str(), BLACK);
            let pad = 20usize.saturating_sub(u.name_len);
            for i in 0..pad { put_char_at(x + u.name_len + i, y + row, b' ', LGRAY); }
            if u.is_su {
                put_str_at(x + 20, y + row, "[SU]", LRED);
            } else {
                put_str_at(x + 20, y + row, "    ", BLACK);
            }
            row += 1;
        }
    }
}

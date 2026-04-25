// PINDOS shell — UNIX-подобный шелл

use crate::vga;
use crate::fs;
use crate::auth;

pub const MAX_INPUT: usize = 256;

static mut CURRENT_NAME: [u8; 32] = [0u8; 32];
static mut CURRENT_NAME_LEN: usize = 0;
static mut CURRENT_IS_SU: bool = false;

pub fn run_as(username: &str, is_su: bool) {
    unsafe {
        let b = username.as_bytes();
        let len = b.len().min(32);
        CURRENT_NAME[..len].copy_from_slice(&b[..len]);
        CURRENT_NAME_LEN = len;
        CURRENT_IS_SU = is_su;
    }
    loop {
        print_prompt();
        let input = read_line();
        let cmd = input.as_str().trim();
        if cmd == "logout" || cmd == "exit" {
            vga::print_colored("Logged out.\n", 0x08);
            return;
        }
        handle_command(cmd);
    }
}

fn current_name() -> &'static str {
    unsafe { core::str::from_utf8(&CURRENT_NAME[..CURRENT_NAME_LEN]).unwrap_or("user") }
}

fn current_is_su() -> bool { unsafe { CURRENT_IS_SU } }

fn print_prompt() {
    vga::print_colored(current_name(), 0x0A);
    vga::print_colored("@pindos:", 0x0A);
    vga::print_colored(fs::cwd(), 0x0B);
    if current_is_su() {
        vga::print_colored("# ", 0x0C);
    } else {
        vga::print_colored("$ ", 0x0F);
    }
}

// Обратная совместимость (не используется, но нужна для линковки)
pub fn run() -> ! {
    fs::init();
    run_as("root", true);
    loop {}
}

fn handle_command(cmd: &str) {
    let cmd = cmd.trim();
    if cmd.is_empty() { return; }

    // Поддержка пайпа: cmd1 | cmd2 (базовый — только вывод первой в буфер)
    // Поддержка редиректа: cmd > file, cmd >> file
    if let Some(pos) = cmd.find(" > ") {
        let (left, right) = (&cmd[..pos], cmd[pos+3..].trim());
        let out = capture_output(left);
        if fs::get(right).is_some() {
            fs::write(right, out.as_str());
        } else {
            fs::create(right, out.as_str());
        }
        return;
    }
    if let Some(pos) = cmd.find(" >> ") {
        let (left, right) = (&cmd[..pos], cmd[pos+4..].trim());
        let out = capture_output(left);
        if fs::get(right).is_none() { fs::create(right, ""); }
        fs::append(right, out.as_str());
        return;
    }

    dispatch(cmd);
}

fn dispatch(cmd: &str) {
    let (name, args) = split_first(cmd);
    match name {
        "help"  => cmd_help(),
        "clear" | "cls" => vga::clear_screen(),

        // Навигация
        "pwd"   => { vga::print(fs::cwd()); vga::put_char(b'\n'); }
        "cd"    => cmd_cd(args),
        "ls"    => cmd_ls(args),
        "ll"    => cmd_ll(args),

        // Файловые операции
        "cat"   => cmd_cat(args),
        "touch" => cmd_touch(args),
        "mkdir" => cmd_mkdir(args),
        "rm"    => cmd_rm(args),
        "rmdir" => cmd_rmdir(args),
        "cp"    => cmd_cp(args),
        "mv"    => cmd_mv(args),
        "echo"  => cmd_echo(args),
        "head"  => cmd_head(args),
        "tail"  => cmd_tail(args),
        "wc"    => cmd_wc(args),
        "grep"  => cmd_grep(args),
        "find"  => cmd_find(args),

        // Система
        "uname"   => cmd_uname(args),
        "whoami"  => { vga::print(current_name()); vga::put_char(b'\n'); }
        "uptime"  => { vga::print("up forever, 1 user\n"); }
        "free"    => cmd_free(),
        "df"      => cmd_df(),
        "ps"      => cmd_ps(),
        "env"     => cmd_env(),
        "history" => { vga::print("(history not implemented)\n"); }

        // Пользователи
        "users"   => auth::list_users(),
        "useradd" => cmd_useradd(args),
        "userdel" => cmd_userdel(args),
        "passwd"  => cmd_passwd(args),
        "su"      => cmd_su(args),

        // Утилиты
        "mocha" => crate::utils::mocha::run(),
        "fastfetch" | "ff" => crate::utils::fastfetch::run(),
        "qinn"  => {
            if args.is_empty() { vga::print("Usage: qinn <file>\n"); }
            else { crate::utils::qinn::run(args); }
        }
        "run"   => cmd_run(args),
        "exec"  => cmd_exec(args),

        // Алиасы
        "exit" | "logout" => { vga::print("There's no escape from PINDOS.\n"); }

        _ => {
            // Попробуем запустить как .com файл
            if name.ends_with(".com") {
                cmd_run(name);
            } else if fs::get(name).is_some() {
                // Попробуем как ELF
                cmd_exec(name);
            } else {
                vga::print(name);
                vga::print(": command not found\n");
            }
        }
    }
}

// ── Команды ───────────────────────────────────────────────────────────────

fn cmd_help() {
    vga::print_colored("PINDOS shell commands:\n", 0x0E);
    vga::print_colored("  Navigation:\n", 0x0B);
    vga::print("    pwd, cd <dir>, ls [dir], ll [dir]\n");
    vga::print_colored("  Files:\n", 0x0B);
    vga::print("    cat <f>, touch <f>, mkdir <d>, rm <f>, rmdir <d>\n");
    vga::print("    cp <src> <dst>, mv <src> <dst>, echo <text>\n");
    vga::print("    head <f>, tail <f>, wc <f>, grep <pat> <f>, find <name>\n");
    vga::print_colored("  System:\n", 0x0B);
    vga::print("    uname [-a], whoami, uptime, free, df, ps, env\n");
    vga::print_colored("  Users:\n", 0x0B);
    vga::print("    users, useradd <name>, userdel <name>, passwd [name], su [name]\n");
    vga::print_colored("  Apps:\n", 0x0B);
    vga::print("    mocha, qinn <f>, run <f.com>, exec <elf>, fastfetch\n");
    vga::print_colored("  Redirect:\n", 0x0B);
    vga::print("    cmd > file   (overwrite)\n");
    vga::print("    cmd >> file  (append)\n");
}

pub fn cmd_cd_pub(args: &str) { cmd_cd(args); }

fn cmd_cd(args: &str) {
    let target = if args.is_empty() { "/" } else { args };

    // Обработка ".."
    let mut pathbuf = [0u8; 128];
    let abs = if target == ".." {
        let cwd = fs::cwd();
        if cwd == "/" {
            "/"
        } else {
            let (parent, _) = fs::split_path(cwd);
            let b = parent.as_bytes();
            let len = b.len().min(128);
            pathbuf[..len].copy_from_slice(&b[..len]);
            core::str::from_utf8(&pathbuf[..len]).unwrap_or("/")
        }
    } else {
        let mut buf = [0u8; 128];
        let r = fs::resolve(target, &mut buf);
        let b = r.as_bytes();
        let len = b.len().min(128);
        pathbuf[..len].copy_from_slice(&b[..len]);
        core::str::from_utf8(&pathbuf[..len]).unwrap_or("/")
    };

    // Проверяем что директория существует (или это корень)
    if abs == "/" {
        fs::set_cwd("/");
        return;
    }
    match fs::get(abs) {
        Some(e) if e.is_dir() => fs::set_cwd(abs),
        Some(_) => { vga::print("cd: not a directory: "); vga::print(target); vga::put_char(b'\n'); }
        None    => { vga::print("cd: no such directory: "); vga::print(target); vga::put_char(b'\n'); }
    }
}

fn cmd_ls(args: &str) {
    let dir = if args.is_empty() { fs::cwd() } else { args };
    let mut found = false;
    for e in fs::list_dir(dir) {
        found = true;
        if e.is_dir() {
            vga::print_colored(e.name_str(), 0x0B); // голубой для директорий
            vga::print_colored("/  ", 0x0B);
        } else {
            vga::print(e.name_str());
            vga::print("  ");
        }
    }
    if found { vga::put_char(b'\n'); }
    else { vga::print("(empty)\n"); }
}

fn cmd_ll(args: &str) {
    let dir = if args.is_empty() { fs::cwd() } else { args };
    let mut found = false;
    for e in fs::list_dir(dir) {
        found = true;
        if e.is_dir() {
            vga::print_colored("d  ", 0x0B);
            vga::print_colored(e.name_str(), 0x0B);
            vga::print_colored("/\n", 0x0B);
        } else {
            vga::print_colored("-  ", 0x08);
            vga::print(e.name_str());
            vga::print("  ");
            print_usize(e.content_len);
            vga::print(" B\n");
        }
    }
    if !found { vga::print("(empty)\n"); }
}

fn cmd_cat(args: &str) {
    if args.is_empty() { vga::print("Usage: cat <file>\n"); return; }
    match fs::get(args) {
        Some(e) if !e.is_dir() => { vga::print(e.content_str()); vga::put_char(b'\n'); }
        Some(_) => { vga::print("cat: "); vga::print(args); vga::print(": Is a directory\n"); }
        None    => { vga::print("cat: "); vga::print(args); vga::print(": No such file\n"); }
    }
}

fn cmd_touch(args: &str) {
    if args.is_empty() { vga::print("Usage: touch <file>\n"); return; }
    if fs::get(args).is_some() { return; } // уже существует
    if !fs::create(args, "") {
        vga::print("touch: cannot create: "); vga::print(args); vga::put_char(b'\n');
    }
}

fn cmd_mkdir(args: &str) {
    if args.is_empty() { vga::print("Usage: mkdir <dir>\n"); return; }
    if !fs::mkdir(args) {
        vga::print("mkdir: cannot create: "); vga::print(args); vga::put_char(b'\n');
    }
}

fn cmd_rm(args: &str) {
    if args.is_empty() { vga::print("Usage: rm <file>\n"); return; }
    match fs::get(args) {
        Some(e) if e.is_dir() => { vga::print("rm: "); vga::print(args); vga::print(": Is a directory (use rmdir)\n"); }
        Some(_) => { fs::delete(args); }
        None    => { vga::print("rm: "); vga::print(args); vga::print(": No such file\n"); }
    }
}

fn cmd_rmdir(args: &str) {
    if args.is_empty() { vga::print("Usage: rmdir <dir>\n"); return; }
    // Проверяем что директория пуста
    let mut buf = [0u8; 128];
    let abs = fs::resolve(args, &mut buf);
    let mut empty = true;
    for e in fs::list_dir(abs) {
        let _ = e;
        empty = false;
        break;
    }
    if !empty { vga::print("rmdir: directory not empty\n"); return; }
    match fs::get(args) {
        Some(e) if e.is_dir() => { fs::delete(args); }
        Some(_) => { vga::print("rmdir: not a directory\n"); }
        None    => { vga::print("rmdir: no such directory\n"); }
    }
}

fn cmd_cp(args: &str) {
    let (src, dst) = split_first(args);
    if src.is_empty() || dst.is_empty() { vga::print("Usage: cp <src> <dst>\n"); return; }
    if !fs::copy_file(src, dst) {
        vga::print("cp: failed\n");
    }
}

fn cmd_mv(args: &str) {
    let (src, dst) = split_first(args);
    if src.is_empty() || dst.is_empty() { vga::print("Usage: mv <src> <dst>\n"); return; }
    if !fs::rename(src, dst) {
        vga::print("mv: failed\n");
    }
}

fn cmd_echo(args: &str) {
    vga::print(args);
    vga::put_char(b'\n');
}

fn cmd_head(args: &str) {
    let (file, rest) = split_first(args);
    let n: usize = if rest.is_empty() { 10 } else { parse_usize(rest).unwrap_or(10) };
    match fs::get(file) {
        Some(e) if !e.is_dir() => {
            let mut count = 0;
            for line in e.content_str().split('\n') {
                if count >= n { break; }
                vga::print(line); vga::put_char(b'\n');
                count += 1;
            }
        }
        _ => { vga::print("head: "); vga::print(file); vga::print(": No such file\n"); }
    }
}

fn cmd_tail(args: &str) {
    let (file, rest) = split_first(args);
    let n: usize = if rest.is_empty() { 10 } else { parse_usize(rest).unwrap_or(10) };
    match fs::get(file) {
        Some(e) if !e.is_dir() => {
            // Собираем строки
            let content = e.content_str();
            let mut lines: [&str; 256] = [""; 256];
            let mut count = 0;
            for line in content.split('\n') {
                if count < 256 { lines[count] = line; count += 1; }
            }
            let start = if count > n { count - n } else { 0 };
            for i in start..count {
                vga::print(lines[i]); vga::put_char(b'\n');
            }
        }
        _ => { vga::print("tail: "); vga::print(file); vga::print(": No such file\n"); }
    }
}

fn cmd_wc(args: &str) {
    if args.is_empty() { vga::print("Usage: wc <file>\n"); return; }
    match fs::get(args) {
        Some(e) if !e.is_dir() => {
            let content = e.content_str();
            let lines = content.split('\n').count();
            let words = content.split(|c: char| c.is_ascii_whitespace())
                               .filter(|w| !w.is_empty()).count();
            let bytes = e.content_len;
            print_usize(lines); vga::print(" ");
            print_usize(words); vga::print(" ");
            print_usize(bytes); vga::print(" ");
            vga::print(args); vga::put_char(b'\n');
        }
        _ => { vga::print("wc: "); vga::print(args); vga::print(": No such file\n"); }
    }
}

fn cmd_grep(args: &str) {
    let (pattern, file) = split_first(args);
    if pattern.is_empty() || file.is_empty() {
        vga::print("Usage: grep <pattern> <file>\n"); return;
    }
    match fs::get(file) {
        Some(e) if !e.is_dir() => {
            for line in e.content_str().split('\n') {
                if line.contains(pattern) {
                    vga::print(line); vga::put_char(b'\n');
                }
            }
        }
        _ => { vga::print("grep: "); vga::print(file); vga::print(": No such file\n"); }
    }
}

fn cmd_find(args: &str) {
    if args.is_empty() { vga::print("Usage: find <name>\n"); return; }
    let mut found = false;
    for e in fs::list() {
        if e.name_str().contains(args) {
            vga::print(e.path_str());
            if e.path_str() != "/" { vga::put_char(b'/'); }
            vga::print(e.name_str());
            vga::put_char(b'\n');
            found = true;
        }
    }
    if !found { vga::print("(not found)\n"); }
}

fn cmd_uname(args: &str) {
    if args.contains('a') || args == "-a" {
        vga::print("PINDOS pindos 0.1 #1 i686 PINDOS\n");
    } else {
        vga::print("PINDOS\n");
    }
}

fn cmd_free() {
    vga::print("              total        used        free\n");
    vga::print("Mem:          32768        4096       28672\n");
    vga::print("Swap:             0           0           0\n");
}

fn cmd_df() {
    vga::print("Filesystem     Size  Used Avail Use% Mounted on\n");
    vga::print("pindosfs        256K  ");
    // подсчитываем использованное
    let mut used = 0usize;
    for e in fs::list() { used += e.content_len; }
    print_usize(used / 1024);
    vga::print("K  ");
    print_usize((256 * 1024 - used) / 1024);
    vga::print("K   /\n");
}

fn cmd_ps() {
    vga::print("  PID TTY      STAT   CMD\n");
    vga::print("    1 tty0     S      init\n");
    vga::print("    2 tty0     S      shell\n");
}

fn cmd_env() {
    vga::print("PATH=/bin\n");
    vga::print("HOME=/home\n");
    vga::print("USER=root\n");
    vga::print("SHELL=/bin/sh\n");
    vga::print("OS=PINDOS\n");
    vga::print("PWD=");
    vga::print(fs::cwd());
    vga::put_char(b'\n');
}

fn cmd_run(args: &str) {
    if args.is_empty() { vga::print("Usage: run <file.com>\n"); return; }
    match crate::dos::loader::load_com(args) {
        Some(prog) => {
            vga::print("Running "); vga::print(args); vga::print("...\n");
            crate::dos::v86::run_com(prog.segment);
            vga::print("\nProgram exited.\n");
        }
        None => { vga::print("Failed to load: "); vga::print(args); vga::put_char(b'\n'); }
    }
}

fn cmd_exec(args: &str) {
    if args.is_empty() {
        vga::print("Usage: exec <elf-file>\n");
        vga::print("  Runs a statically linked i386 ELF binary.\n");
        vga::print("  Compile with: gcc -m32 -static -o prog prog.c\n");
        return;
    }
    let code = crate::linux::process::run(args);
    vga::print("Process exited with code ");
    crate::shell::print_usize(code.unsigned_abs() as usize);
    vga::put_char(b'\n');
}

// ── Управление пользователями ─────────────────────────────────────────────

fn cmd_useradd(args: &str) {
    if !current_is_su() {
        vga::print_colored("Permission denied. SU required.\n", 0x0C);
        return;
    }
    let name = args.trim();
    if name.is_empty() { vga::print("Usage: useradd <username>\n"); return; }

    vga::print_colored("Password: ", 0x0F);
    let pass = auth::read_password();
    vga::print_colored("Confirm:  ", 0x0F);
    let pass2 = auth::read_password();

    if pass.as_str() != pass2.as_str() {
        vga::print_colored("Passwords do not match.\n", 0x0C);
        return;
    }

    vga::print_colored("Grant superuser privileges? [y/N]: ", 0x0F);
    let c = vga::read_char();
    vga::put_char(c); vga::put_char(b'\n');
    let is_su = c == b'y' || c == b'Y';

    if auth::add_user(name, pass.as_str(), is_su) {
        vga::print_colored("User '", 0x0A);
        vga::print_colored(name, 0x0A);
        vga::print_colored("' created.\n", 0x0A);
    } else {
        vga::print_colored("Failed: user exists or limit reached.\n", 0x0C);
    }
}

fn cmd_userdel(args: &str) {
    if !current_is_su() {
        vga::print_colored("Permission denied. SU required.\n", 0x0C);
        return;
    }
    let name = args.trim();
    if name.is_empty() { vga::print("Usage: userdel <username>\n"); return; }
    if name == "root" { vga::print_colored("Cannot delete root.\n", 0x0C); return; }

    if auth::delete_user(name) {
        vga::print("User '"); vga::print(name); vga::print("' deleted.\n");
    } else {
        vga::print("User not found: "); vga::print(name); vga::put_char(b'\n');
    }
}

fn cmd_passwd(args: &str) {
    let target = if args.is_empty() { current_name() } else { args.trim() };

    // Менять чужой пароль может только SU
    if target != current_name() && !current_is_su() {
        vga::print_colored("Permission denied.\n", 0x0C);
        return;
    }

    vga::print_colored("New password: ", 0x0F);
    let pass = auth::read_password();
    vga::print_colored("Confirm:      ", 0x0F);
    let pass2 = auth::read_password();

    if pass.as_str() != pass2.as_str() {
        vga::print_colored("Passwords do not match.\n", 0x0C);
        return;
    }

    if auth::change_password(target, pass.as_str()) {
        vga::print_colored("Password updated.\n", 0x0A);
    } else {
        vga::print_colored("User not found.\n", 0x0C);
    }
}

fn cmd_su(args: &str) {
    let target = if args.is_empty() { "root" } else { args.trim() };

    vga::print_colored("Password: ", 0x0F);
    let pass = auth::read_password();

    if auth::verify_user(target, pass.as_str()) {
        let is_su = auth::user_is_su(target);
        vga::print_colored("Switched to '", 0x0A);
        vga::print_colored(target, 0x0A);
        vga::print_colored("'\n", 0x0A);
        // Запускаем вложенный шелл от имени нового пользователя
        run_as(target, is_su);
    } else {
        vga::print_colored("Authentication failure.\n", 0x0C);
    }
}

// ── capture_output — запускает команду и возвращает вывод в буфере ────────
// Упрощённая версия: только для echo, cat, uname, whoami, pwd
fn capture_output(cmd: &str) -> InputBuf {
    let mut buf = InputBuf::new();
    let (name, args) = split_first(cmd.trim());
    match name {
        "echo"  => { for b in args.bytes() { buf.push(b); } buf.push(b'\n'); }
        "pwd"   => { for b in fs::cwd().bytes() { buf.push(b); } buf.push(b'\n'); }
        "whoami"=> { for b in current_name().bytes() { buf.push(b); } buf.push(b'\n'); }
        "uname" => { for b in b"PINDOS\n".iter() { buf.push(*b); } }
        "cat"   => {
            if let Some(e) = fs::get(args) {
                for b in e.content_str().bytes() { buf.push(b); }
            }
        }
        _ => { dispatch(cmd); } // fallback — просто выполняем
    }
    buf
}

// ── Утилиты ───────────────────────────────────────────────────────────────

pub fn read_line() -> InputBuf {
    let mut buf = InputBuf::new();
    loop {
        let c = vga::read_char();
        match c {
            b'\n' => { vga::put_char(b'\n'); break; }
            b'\x08' => {
                if buf.len > 0 { buf.len -= 1; vga::put_char(b'\x08'); }
            }
            _ => {
                if buf.len < MAX_INPUT - 1 {
                    buf.data[buf.len] = c;
                    buf.len += 1;
                    vga::put_char(c);
                }
            }
        }
    }
    buf
}

pub struct InputBuf {
    pub data: [u8; MAX_INPUT],
    pub len: usize,
}

impl InputBuf {
    pub fn new() -> Self { InputBuf { data: [0u8; MAX_INPUT], len: 0 } }
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.data[..self.len]).unwrap_or("")
    }
    pub fn push(&mut self, b: u8) {
        if self.len < MAX_INPUT - 1 { self.data[self.len] = b; self.len += 1; }
    }
}

fn split_first(s: &str) -> (&str, &str) {
    match s.find(' ') {
        Some(i) => (&s[..i], s[i+1..].trim()),
        None    => (s, ""),
    }
}

pub fn print_usize(n: usize) {
    if n == 0 { vga::put_char(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    let mut n = n;
    while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { vga::put_char(buf[j]); }
}

fn parse_usize(s: &str) -> Option<usize> {
    let mut n = 0usize;
    for b in s.bytes() {
        if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as usize; }
        else { return None; }
    }
    Some(n)
}

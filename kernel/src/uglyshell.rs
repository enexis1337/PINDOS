// uglyshell — UNIX-подобный шелл

use crate::vga;
use crate::fs;
use crate::auth;

pub const MAX_INPUT: usize = 256;

pub fn run_as(username: &str, _is_su: bool) {
    // Переключаемся на указанного пользователя
    auth::switch_user(username);
    
    let mut tick_counter = 0u32;
    
    loop {
        print_prompt();
        let input = read_line_no_prompt();
        let cmd = input.as_str().trim();
        if cmd == "logout" || cmd == "exit" {
            vga::print_colored("Logged out.\n", 0x08);
            return;
        }
        handle_command(cmd);
        
        // Периодически вызываем планировщик задач
        tick_counter += 1;
        if tick_counter % 10 == 0 { // Каждые 10 команд
            crate::dealduckd::tick();
        }
    }
}

fn print_prompt() {
    vga::print_colored(crate::auth::current_name(), 0x0A);
    vga::print_colored("@pindos:", 0x0A);
    vga::print_colored(fs::cwd(), 0x0B);
    if crate::auth::is_su() {
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
            fs::write(right, out);
        } else {
            fs::create(right, out);
        }
        return;
    }
    if let Some(pos) = cmd.find(" >> ") {
        let (left, right) = (&cmd[..pos], cmd[pos+4..].trim());
        let out = capture_output(left);
        if fs::get(right).is_none() { fs::create(right, ""); }
        fs::append(right, out);
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
        "pwd" => { vga::print(fs::cwd()); vga::put_char(b'\n'); },
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
        "whoami"  => { vga::print(crate::auth::current_name()); vga::put_char(b'\n'); },
        "uptime"  => { vga::print("up forever, 1 user\n"); },
        "free"    => cmd_free(),
        "df"      => cmd_df(),
        "ps"      => cmd_ps(),
        "env"     => cmd_env(),
        "date"    => cmd_date(),
        "time"    => cmd_time(),
        "history" => cmd_history(),

        // Диски и FAT
        "disks"   => cmd_disks(),
        "mount"   => cmd_mount(args),
        "fatls"   => cmd_fatls(args),
        "fatcat"  => cmd_fatcat(args),
        "fatcp"   => cmd_fatcp(args),

        // Пользователи
        "users"   => auth::list_users(),
        "useradd" => cmd_useradd(args),
        "userdel" => cmd_userdel(args),
        "passwd"  => cmd_passwd(args),
        "su"      => cmd_su(args),
        "sudo"    => cmd_sudo(args),

        // Утилиты
        "mocha" => crate::utils::mocha::run(),
        "fastfetch" | "ff" => crate::utils::fastfetch::run(),
        "mell"  => crate::mell::run(),
        "dealduckd" => cmd_dealduckd(args),
        "play"  => cmd_play(args),
        "view"  => cmd_view(args),
        "beep"  => { crate::drivers::speaker::play_melody(crate::drivers::speaker::MELODY_NOTIFY); }
        "qinn"  => {
            if args.is_empty() { vga::print("Usage: qinn <file>\n"); }
            else { crate::utils::qinn::run(args); }
        }
        "run"   => cmd_run(args),
        "exec"  => cmd_exec(args),

        // Алиасы
        "exit" | "logout" => { vga::print("There's no escape from PINDOS.\n"); }
        "dir" => cmd_ls(args), // DOS alias
        "type" => cmd_cat(args), // DOS alias
        "copy" => cmd_cp(args), // DOS alias
        "move" => cmd_mv(args), // DOS alias
        "del" => cmd_rm(args), // DOS alias
        "md" => cmd_mkdir(args), // DOS alias
        "rd" => cmd_rmdir(args), // DOS alias
        "more" | "less" => cmd_less(args),
        "which" => cmd_which(args),
        "file" => cmd_file(args),
        "id" => cmd_id(),

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
    vga::print_colored("PINDOS uglyshell - UNIX-like command line interface\n\n", 0x0F);
    
    vga::print_colored("Navigation & Files:\n", 0x0E);
    vga::print("  pwd, cd, ls, ll, dir          - directory navigation\n");
    vga::print("  cat, type, less, more         - view file contents\n");
    vga::print("  head, tail, wc, grep, find    - text processing\n");
    vga::print("  touch, mkdir, rm, rmdir       - create/delete files/dirs\n");
    vga::print("  cp, copy, mv, move, ln        - copy/move/link files\n");
    vga::print("  file, which, whereis          - file information\n");
    
    vga::print_colored("\nText Processing:\n", 0x0E);
    vga::print("  echo, printf                  - output text\n");
    vga::print("  sort, uniq, cut, tr           - text manipulation\n");
    
    vga::print_colored("\nSystem Information:\n", 0x0E);
    vga::print("  uname, whoami, id, uptime     - system info\n");
    vga::print("  date, time, free, df, ps      - status commands\n");
    vga::print("  top, jobs, history            - monitoring\n");
    vga::print("  env, disks, mount             - environment\n");
    
    vga::print_colored("\nUser Management:\n", 0x0E);
    vga::print("  su, sudo, passwd              - user switching\n");
    vga::print("  useradd, userdel, groups      - user management\n");
    vga::print("  chmod, chown                  - permissions (stubs)\n");
    
    vga::print_colored("\nSpecial Commands:\n", 0x0E);
    vga::print("  fastfetch, mell, dealduckd    - PINDOS utilities\n");
    vga::print("  play, view, beep              - multimedia\n");
    vga::print("  fatls, fatcat, fatcp          - FAT filesystem\n");
    vga::print("  run, exec                     - program execution\n");
    
    vga::print_colored("\nKeyboard Shortcuts:\n", 0x0B);
    vga::print("  Ctrl+C    - interrupt command\n");
    vga::print("  Ctrl+D    - EOF / exit (if line empty)\n");
    vga::print("  Ctrl+A    - beginning of line\n");
    vga::print("  Ctrl+E    - end of line\n");
    vga::print("  Ctrl+K    - kill to end of line\n");
    vga::print("  Ctrl+U    - clear entire line\n");
    vga::print("  Ctrl+L    - clear screen\n");
    vga::print("  Up/Down   - command history\n");
    vga::print("  Left/Right- cursor movement\n");
    
    vga::print_colored("\nRedirection:\n", 0x0B);
    vga::print("  cmd > file    - redirect output to file\n");
    vga::print("  cmd >> file   - append output to file\n");
    
    vga::print_colored("\nSudo Examples:\n", 0x0B);
    vga::print("  sudo ls /root     - list root directory as root\n");
    vga::print("  sudo useradd bob  - add user as root\n");
    vga::print("  sudo rm /etc/file - delete system file as root\n");
    
    vga::print_colored("\nType 'exit' or Ctrl+D to quit.\n", 0x08);
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
        vga::print(crate::version::OS_NAME);
        vga::print(" pindos ");
        vga::print(crate::version::OS_VERSION);
        vga::print(" #1 i686 ");
        vga::print(crate::version::OS_NAME);
        vga::put_char(b'\n');
    } else {
        vga::print(crate::version::OS_NAME);
        vga::put_char(b'\n');
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
    if args.is_empty() { vga::print("Usage: run <file.com|file.exe>\n"); return; }
    match crate::dos::loader::load_auto(args) {
        Some(prog) => {
            vga::print("Running "); vga::print(args); vga::print("...\n");
            crate::dos::v86::run_com(prog.cs);
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
    if !crate::auth::is_su() {
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
    if !crate::auth::is_su() {
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
    let target = if args.is_empty() { crate::auth::current_name() } else { args.trim() };

    // Менять чужой пароль может только SU
    if target != crate::auth::current_name() && !crate::auth::is_su() {
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

fn cmd_sudo(args: &str) {
    if args.is_empty() {
        vga::print("Usage: sudo <command> [args...]\n");
        vga::print("Execute command as root (requires current user password)\n");
        return;
    }

    // Если уже root, просто выполняем команду
    if crate::auth::is_su() {
        handle_command(args);
        return;
    }

    // Запрашиваем пароль текущего пользователя
    vga::print_colored("[sudo] password for ", 0x0E);
    vga::print_colored(crate::auth::current_name(), 0x0E);
    vga::print_colored(": ", 0x0E);
    let pass = auth::read_password();

    // Проверяем пароль текущего пользователя
    if !auth::verify_user(crate::auth::current_name(), pass.as_str()) {
        vga::print_colored("Sorry, try again.\n", 0x0C);
        return;
    }

    // Проверяем, есть ли у пользователя sudo права
    if !user_can_sudo(crate::auth::current_name()) {
        vga::print_colored(crate::auth::current_name(), 0x0C);
        vga::print_colored(" is not in the sudoers file. This incident will be reported.\n", 0x0C);
        return;
    }

    // Сохраняем текущее состояние пользователя
    let orig_name = crate::auth::current_name();
    let orig_is_su = crate::auth::is_su();

    // Временно переключаемся на root
    auth::switch_to_root();

    // Выполняем команду
    vga::print_colored("[sudo] executing: ", 0x08);
    vga::print_colored(args, 0x08);
    vga::put_char(b'\n');
    handle_command(args);

    // Восстанавливаем исходного пользователя
    auth::switch_back_from_root(orig_name, orig_is_su);
}

// Проверяет, может ли пользователь использовать sudo
fn user_can_sudo(username: &str) -> bool {
    // В простой реализации все пользователи могут использовать sudo
    // В реальной системе это проверялось бы через /etc/sudoers
    match username {
        "root" => true,  // root всегда может
        _ => true,       // пока разрешаем всем (можно ограничить)
    }
}

fn capture_output(cmd: &str) -> &'static str {
    capture_command(cmd)
}

/// Публичная функция для Burmalda — выполняет команду и возвращает вывод
pub fn capture_command(cmd: &str) -> &'static str {
    // Включаем перехват вывода
    crate::vga::capture_start();
    dispatch(cmd);
    crate::vga::capture_end()
}

// ── Утилиты ───────────────────────────────────────────────────────────────

// История команд
const HISTORY_SIZE: usize = 32;
static mut HISTORY: [[u8; MAX_INPUT]; HISTORY_SIZE] = [[0u8; MAX_INPUT]; HISTORY_SIZE];
static mut HISTORY_LENS: [usize; HISTORY_SIZE] = [0; HISTORY_SIZE];
static mut HISTORY_COUNT: usize = 0;
static mut HISTORY_POS: usize = 0;

// Состояние редактирования строки
struct LineEditor {
    buf: InputBuf,
    cursor: usize,
    history_pos: usize,
    saved_line: InputBuf,
}

impl LineEditor {
    fn new() -> Self {
        LineEditor {
            buf: InputBuf::new(),
            cursor: 0,
            history_pos: unsafe { HISTORY_COUNT },
            saved_line: InputBuf::new(),
        }
    }

    fn insert_char(&mut self, c: u8) {
        if self.buf.len >= MAX_INPUT - 1 { return; }
        // Сдвигаем символы вправо
        for i in (self.cursor..self.buf.len).rev() {
            self.buf.data[i + 1] = self.buf.data[i];
        }
        self.buf.data[self.cursor] = c;
        self.buf.len += 1;
        self.cursor += 1;
    }

    fn delete_char(&mut self) {
        if self.cursor == 0 { return; }
        // Сдвигаем символы влево
        for i in self.cursor..self.buf.len {
            self.buf.data[i - 1] = self.buf.data[i];
        }
        self.buf.len -= 1;
        self.cursor -= 1;
    }

    fn delete_forward(&mut self) {
        if self.cursor >= self.buf.len { return; }
        for i in self.cursor + 1..self.buf.len {
            self.buf.data[i - 1] = self.buf.data[i];
        }
        self.buf.len -= 1;
    }

    fn move_left(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
    }

    fn move_right(&mut self) {
        if self.cursor < self.buf.len { self.cursor += 1; }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.buf.len;
    }

    fn clear_line(&mut self) {
        self.buf.len = 0;
        self.cursor = 0;
    }

    fn kill_to_end(&mut self) {
        self.buf.len = self.cursor;
    }

    fn history_up(&mut self) {
        unsafe {
            if HISTORY_COUNT == 0 { return; }
            if self.history_pos == HISTORY_COUNT {
                // Сохраняем текущую строку
                self.saved_line = self.buf.clone();
            }
            if self.history_pos > 0 {
                self.history_pos -= 1;
                let len = HISTORY_LENS[self.history_pos];
                self.buf.len = len;
                self.buf.data[..len].copy_from_slice(&HISTORY[self.history_pos][..len]);
                self.cursor = len;
            }
        }
    }

    fn history_down(&mut self) {
        unsafe {
            if HISTORY_COUNT == 0 { return; }
            if self.history_pos < HISTORY_COUNT {
                self.history_pos += 1;
                if self.history_pos == HISTORY_COUNT {
                    // Восстанавливаем сохранённую строку
                    self.buf = self.saved_line.clone();
                    self.cursor = self.buf.len;
                } else {
                    let len = HISTORY_LENS[self.history_pos];
                    self.buf.len = len;
                    self.buf.data[..len].copy_from_slice(&HISTORY[self.history_pos][..len]);
                    self.cursor = len;
                }
            }
        }
    }

    fn add_to_history(&self) {
        if self.buf.len == 0 { return; }
        unsafe {
            // Проверяем, не дублируется ли с последней командой
            if HISTORY_COUNT > 0 {
                let last_idx = HISTORY_COUNT - 1;
                if HISTORY_LENS[last_idx] == self.buf.len &&
                   HISTORY[last_idx][..self.buf.len] == self.buf.data[..self.buf.len] {
                    return;
                }
            }
            
            let idx = HISTORY_COUNT % HISTORY_SIZE;
            HISTORY_LENS[idx] = self.buf.len;
            HISTORY[idx][..self.buf.len].copy_from_slice(&self.buf.data[..self.buf.len]);
            if HISTORY_COUNT < HISTORY_SIZE {
                HISTORY_COUNT += 1;
            }
        }
    }

    fn redraw_line(&self) {
        self.redraw_line_with_prompt(true);
    }
    
    fn redraw_line_no_prompt(&self) {
        self.redraw_line_with_prompt(false);
    }
    
    fn redraw_line_with_prompt(&self, show_prompt: bool) {
        // Сохраняем текущую позицию курсора
        let (_, start_y) = vga::get_cursor_pos();
        
        // Перемещаемся в начало строки
        vga::set_cursor_pos(0, start_y);
        
        // Очищаем всю строку
        for _ in 0..80 {
            vga::put_char(b' ');
        }
        
        // Возвращаемся в начало строки
        vga::set_cursor_pos(0, start_y);
        
        let prompt_len = if show_prompt {
            // Выводим промпт
            print_prompt();
            
            // Вычисляем длину промпта (только видимые символы)
            let user = crate::auth::current_name();
            let cwd = crate::fs::cwd();
            let suffix = if crate::auth::is_su() { "# " } else { "$ " };
            user.len() + 8 + cwd.len() + suffix.len() // user@pindos:cwd$ 
        } else {
            0
        };
        
        // Выводим весь текст
        for i in 0..self.buf.len {
            vga::put_char(self.buf.data[i]);
        }
        
        // Устанавливаем курсор в правильную позицию
        let target_x = prompt_len + self.cursor;
        // Убеждаемся, что не выходим за границы экрана
        if target_x < 80 {
            vga::set_cursor_pos(target_x, start_y);
        } else {
            vga::set_cursor_pos(79, start_y);
        }
    }
}

pub fn read_line() -> InputBuf {
    read_line_with_prompt(true)
}

pub fn read_line_no_prompt() -> InputBuf {
    read_line_with_prompt(false)
}

fn read_line_with_prompt(show_prompt: bool) -> InputBuf {
    let mut editor = LineEditor::new();
    
    if show_prompt {
        print_prompt();
    }
    
    loop {
        let c = vga::read_char();
        match c {
            // Enter
            b'\n' => {
                vga::put_char(b'\n');
                editor.add_to_history();
                return editor.buf;
            }
            
            // Ctrl+C - прерывание
            0x03 => {
                vga::print_colored("^C\n", 0x0C);
                editor.clear_line();
                return editor.buf;
            }
            
            // Ctrl+D - EOF (если строка пустая - выход)
            0x04 => {
                if editor.buf.len == 0 {
                    vga::print_colored("exit\n", 0x08);
                    editor.buf.data[0] = b'e'; editor.buf.data[1] = b'x'; 
                    editor.buf.data[2] = b'i'; editor.buf.data[3] = b't';
                    editor.buf.len = 4;
                    return editor.buf;
                } else {
                    editor.delete_forward();
                    editor.redraw_line();
                }
            }
            
            // Ctrl+A - начало строки
            0x01 => {
                editor.move_home();
                editor.redraw_line();
            }
            
            // Ctrl+E - конец строки
            0x05 => {
                editor.move_end();
                editor.redraw_line();
            }
            
            // Ctrl+K - удалить до конца строки
            0x0B => {
                editor.kill_to_end();
                editor.redraw_line();
            }
            
            // Ctrl+U - очистить всю строку
            0x15 => {
                editor.clear_line();
                editor.redraw_line();
            }
            
            // Ctrl+L - очистить экран
            0x0C => {
                vga::clear_screen();
                editor.redraw_line();
            }
            
            // Backspace
            b'\x08' => {
                editor.delete_char();
                editor.redraw_line();
            }
            
            // Delete (если поддерживается)
            0x7F => {
                editor.delete_forward();
                editor.redraw_line();
            }
            
            // Escape sequences (стрелки)
            0x1B => {
                let c2 = vga::read_char();
                if c2 == b'[' {
                    let c3 = vga::read_char();
                    match c3 {
                        b'A' => { // Up arrow
                            editor.history_up();
                            editor.redraw_line();
                        }
                        b'B' => { // Down arrow
                            editor.history_down();
                            editor.redraw_line();
                        }
                        b'C' => { // Right arrow
                            editor.move_right();
                            editor.redraw_line();
                        }
                        b'D' => { // Left arrow
                            editor.move_left();
                            editor.redraw_line();
                        }
                        b'H' => { // Home
                            editor.move_home();
                            editor.redraw_line();
                        }
                        b'F' => { // End
                            editor.move_end();
                            editor.redraw_line();
                        }
                        b'3' => { // Delete key (ESC[3~)
                            let c4 = vga::read_char();
                            if c4 == b'~' {
                                editor.delete_forward();
                                editor.redraw_line();
                            }
                        }
                        _ => {} // Игнорируем неизвестные последовательности
                    }
                }
            }
            
            // Tab - автодополнение (пока заглушка)
            b'\t' => {
                // TODO: реализовать автодополнение команд и файлов
                vga::put_char(b' '); // Пока просто пробел
                editor.insert_char(b' ');
            }
            
            // Обычные символы
            c if c >= 0x20 && c < 0x7F => {
                editor.insert_char(c);
                editor.redraw_line();
            }
            
            _ => {} // Игнорируем остальные управляющие символы
        }
    }
}

#[derive(Clone)]
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

fn cmd_date() {
    let dt = crate::drivers::rtc::read();
    let mut dbuf = [0u8; 10];
    crate::drivers::rtc::format_date(&dt, &mut dbuf);
    vga::print(dt.weekday_str());
    vga::put_char(b' ');
    vga::print(core::str::from_utf8(&dbuf).unwrap_or(""));
    vga::put_char(b'\n');
}

fn cmd_time() {
    let dt = crate::drivers::rtc::read();
    let mut tbuf = [0u8; 8];
    crate::drivers::rtc::format_time(&dt, &mut tbuf);
    vga::print(core::str::from_utf8(&tbuf).unwrap_or(""));
    vga::put_char(b'\n');
}

fn cmd_play(args: &str) {
    if args.is_empty() {
        vga::print("Usage: play <file.wav|file.mp3>\n");
        vga::print("  Plays WAV/MP3 through PC speaker\n");
        return;
    }
    match fs::get(args) {
        Some(f) if !f.is_dir() => {
            vga::print("Playing: ");
            vga::print(args);
            vga::print(" (press any key to stop)\n");
            let data = f.content_bytes();
            let result = if args.ends_with(".mp3") || args.ends_with(".MP3") {
                crate::drivers::speaker::play_mp3(data)
            } else {
                crate::drivers::speaker::play_wav(data)
            };
            match result {
                Ok(_) => vga::print("Done.\n"),
                Err(crate::drivers::speaker::AudioError::NotWav) =>
                    vga::print("Error: not a WAV file\n"),
                Err(crate::drivers::speaker::AudioError::NotMp3) =>
                    vga::print("Error: not an MP3 file\n"),
                Err(crate::drivers::speaker::AudioError::MissingFmt) =>
                    vga::print("Error: broken WAV fmt chunk\n"),
                Err(crate::drivers::speaker::AudioError::MissingData) =>
                    vga::print("Error: WAV has no data chunk\n"),
                Err(crate::drivers::speaker::AudioError::NotPcm) =>
                    vga::print("Error: only PCM WAV supported\n"),
                Err(crate::drivers::speaker::AudioError::UnsupportedChannels) =>
                    vga::print("Error: only mono/stereo audio supported\n"),
                Err(crate::drivers::speaker::AudioError::UnsupportedBits) =>
                    vga::print("Error: only 8-bit or 16-bit WAV supported\n"),
                Err(crate::drivers::speaker::AudioError::NoAudioFrames) =>
                    vga::print("Error: no playable audio frames found\n"),
                Err(_) => vga::print("Error: cannot play file\n"),
            }
        }
        _ => { vga::print("File not found: "); vga::print(args); vga::put_char(b'\n'); }
    }
}

fn cmd_view(args: &str) {
    if args.is_empty() {
        vga::print("Usage: view <file.png>\n");
        return;
    }
    match fs::get(args) {
        Some(f) if !f.is_dir() => {
            // Создаём временное окно для отображения
            let win = crate::mell::vga_gui::Window::new(2, 1, 76, 22, "Image Viewer");
            win.draw();
            let data = f.content_bytes();
            match crate::drivers::png::render_png_ascii(data, &win) {
                Ok(_) => {
                    crate::mell::vga_gui::put_str_at(2, 23,
                        "Press any key to close...",
                        crate::mell::vga_gui::WHITE);
                    vga::read_char();
                    vga::clear_screen();
                }
                Err(_) => { vga::print("Cannot display image\n"); }
            }
        }
        _ => { vga::print("File not found: "); vga::print(args); vga::put_char(b'\n'); }
    }
}

fn cmd_disks() {
    let count = crate::drivers::ata::disk_count();
    if count == 0 { vga::print("No ATA disks found\n"); return; }
    for i in 0..count {
        if let Some(d) = crate::drivers::ata::get_disk(i) {
            vga::put_char(b'C' + i as u8);
            vga::print(": ");
            vga::print(d.model_str());
            vga::print("  ");
            print_usize((d.sectors / 2048) as usize);
            vga::print(" MB  ");
            vga::print(if d.lba48 { "LBA48" } else { "LBA28" });
            vga::put_char(b'\n');
        }
    }
    vga::print("\nMounted FAT volumes: ");
    print_usize(crate::fs_fat::file::volume_count());
    vga::put_char(b'\n');
}

fn cmd_mount(args: &str) {
    // mount C:  — монтировать диск C
    if args.is_empty() {
        // Показать смонтированные тома
        for i in 0..crate::drivers::ata::disk_count() {
            let letter = b'C' + i as u8;
            if let Some(fs) = crate::fs_fat::file::get_volume(letter) {
                vga::put_char(letter);
                vga::print(": FAT");
                vga::print(match fs.geo.fat_type {
                    crate::fs_fat::bpb::FatType::Fat12 => "12",
                    crate::fs_fat::bpb::FatType::Fat16 => "16",
                    crate::fs_fat::bpb::FatType::Fat32 => "32",
                });
                vga::print("  ");
                print_usize(fs.geo.total_clusters as usize);
                vga::print(" clusters\n");
            }
        }
        return;
    }
    vga::print("Usage: mount (no args = show volumes)\n");
}

fn cmd_fatls(args: &str) {
    // fatls C:/path
    let (vol, path) = parse_fat_path(args);
    let letter = vol.unwrap_or(b'C');

    let fs = match crate::fs_fat::file::get_volume(letter) {
        Some(f) => f,
        None => { vga::print("Volume not mounted\n"); return; }
    };

    let dir_cluster = if path.is_empty() || path == "/" {
        0u32
    } else {
        match fs.resolve_path(path) {
            Some(e) if e.is_dir() => e.cluster(),
            Some(_) => { vga::print("Not a directory\n"); return; }
            None => { vga::print("Path not found\n"); return; }
        }
    };

    fs.list_dir_entries(dir_cluster, |entry, name| {
        if entry.is_dir() {
            vga::print_colored("[DIR] ", 0x0B); // LCYAN on BLACK
            vga::print(name);
        } else {
            vga::print("      ");
            vga::print(name);
            vga::print("  ");
            print_usize(entry.file_size as usize);
            vga::print(" B");
        }
        vga::put_char(b'\n');
    });
}

fn cmd_fatcat(args: &str) {
    let (vol, path) = parse_fat_path(args);
    let letter = vol.unwrap_or(b'C');

    let fs = match crate::fs_fat::file::get_volume(letter) {
        Some(f) => f,
        None => { vga::print("Volume not mounted\n"); return; }
    };

    let mut file = match crate::fs_fat::file::FatFile::open(fs, path, crate::fs_fat::file::OpenMode::Read) {
        Some(f) => f,
        None => { vga::print("File not found\n"); return; }
    };

    let mut buf = [0u8; 512];
    loop {
        let n = file.read(&mut buf);
        if n == 0 { break; }
        for &b in &buf[..n] {
            vga::put_char(b);
        }
    }
    vga::put_char(b'\n');
}

fn cmd_fatcp(args: &str) {
    // fatcp C:/source.txt /dest.txt  — копирует с FAT в PINDOS FS
    let (src_str, dst) = split_first(args);
    if src_str.is_empty() || dst.is_empty() {
        vga::print("Usage: fatcp <C:/src> <dst>\n");
        return;
    }

    let (vol, path) = parse_fat_path(src_str);
    let letter = vol.unwrap_or(b'C');

    let fs = match crate::fs_fat::file::get_volume(letter) {
        Some(f) => f,
        None => { vga::print("Volume not mounted\n"); return; }
    };

    let mut file = match crate::fs_fat::file::FatFile::open(fs, path, crate::fs_fat::file::OpenMode::Read) {
        Some(f) => f,
        None => { vga::print("Source file not found\n"); return; }
    };

    // Читаем содержимое
    let mut content = [0u8; 4096];
    let n = file.read(&mut content);
    let s = core::str::from_utf8(&content[..n]).unwrap_or("");

    if fs::get(dst).is_some() {
        fs::write(dst, s);
    } else {
        fs::create(dst, s);
    }

    vga::print("Copied ");
    print_usize(n);
    vga::print(" bytes to ");
    vga::print(dst);
    vga::put_char(b'\n');
}

fn parse_fat_path(s: &str) -> (Option<u8>, &str) {
    // "C:/path" → (Some(b'C'), "/path")
    // "/path"   → (None, "/path")
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let letter = s.as_bytes()[0].to_ascii_uppercase();
        let path = if s.len() > 2 { &s[2..] } else { "/" };
        (Some(letter), path)
    } else {
        (None, s)
    }
}
fn cmd_dealduckd(args: &str) {
    // Парсим аргументы в статический массив
    let mut args_array = [""; 8];
    let mut count = 0;
    
    for arg in args.split_whitespace() {
        if count < 8 {
            args_array[count] = arg;
            count += 1;
        }
    }
    
    crate::dealduckd::dealduckd_command(&args_array[..count]);
}
// ── Новые UNIX-подобные команды ──────────────────────────────────────────

fn cmd_less(args: &str) {
    if args.is_empty() {
        vga::print("Usage: less <file>\n");
        return;
    }
    // Пока просто алиас для cat
    cmd_cat(args);
}

fn cmd_which(args: &str) {
    if args.is_empty() {
        vga::print("Usage: which <command>\n");
        return;
    }
    
    // Список встроенных команд
    let builtins = [
        "help", "clear", "cls", "pwd", "cd", "ls", "ll", "cat", "touch", "mkdir",
        "rm", "rmdir", "cp", "mv", "echo", "head", "tail", "wc", "grep", "find",
        "uname", "whoami", "uptime", "free", "df", "ps", "env", "date", "time",
        "history", "disks", "mount", "fatls", "fatcat", "fatcp", "users", "useradd",
        "userdel", "passwd", "su", "sudo", "mocha", "fastfetch", "mell", "dealduckd",
        "play", "view", "beep", "qinn", "run", "exec"
    ];
    
    for &builtin in &builtins {
        if builtin == args {
            vga::print(args); vga::print(": shell builtin\n");
            return;
        }
    }
    
    // Проверяем файлы в текущей директории
    if fs::get(args).is_some() {
        vga::print("./"); vga::print(args); vga::put_char(b'\n');
        return;
    }
    
    vga::print(args); vga::print(": not found\n");
}

fn cmd_file(args: &str) {
    if args.is_empty() {
        vga::print("Usage: file <filename>\n");
        return;
    }
    
    if let Some(f) = fs::get(args) {
        vga::print(args); vga::print(": ");
        if f.is_dir() {
            vga::print("directory\n");
        } else {
            let content = f.content_str();
            if content.len() > 4 {
                let header = &content.as_bytes()[..4];
                if header == b"\x7fELF" {
                    vga::print("ELF executable\n");
                } else if header[..2] == [0x4D, 0x5A] { // MZ
                    vga::print("DOS executable\n");
                } else if content.chars().all(|c| c.is_ascii() && (c.is_ascii_graphic() || c.is_ascii_whitespace())) {
                    vga::print("ASCII text\n");
                } else {
                    vga::print("binary data\n");
                }
            } else {
                vga::print("empty file\n");
            }
        }
    } else {
        vga::print(args); vga::print(": No such file or directory\n");
    }
}

fn cmd_id() {
    vga::print("uid=0("); vga::print(crate::auth::current_name()); vga::print(") gid=0(");
    if crate::auth::is_su() {
        vga::print("root");
    } else {
        vga::print("users");
    }
    vga::print(")\n");
}

fn cmd_chmod(_args: &str) {
    vga::print("chmod: not implemented (PINDOS has no file permissions)\n");
}

fn cmd_chown(_args: &str) {
    vga::print("chown: not implemented (PINDOS has no file ownership)\n");
}

fn cmd_ln(_args: &str) {
    vga::print("ln: not implemented (PINDOS has no symbolic links)\n");
}

fn cmd_printf(args: &str) {
    // Простая реализация printf без форматирования
    vga::print(args);
}

fn cmd_jobs() {
    vga::print("No active jobs\n");
}

fn cmd_kill(_args: &str) {
    vga::print("kill: not implemented (PINDOS has no process management)\n");
}

fn cmd_top() {
    vga::print("top - PINDOS system monitor\n");
    vga::print("Tasks: 1 total, 1 running, 0 sleeping\n");
    vga::print("CPU usage: 100% kernel\n");
    cmd_free();
    vga::print("\nPID  COMMAND\n");
    vga::print("  1  kernel\n");
}

fn cmd_ping(_args: &str) {
    vga::print("ping: network not available\n");
}

fn cmd_tar(_args: &str) {
    vga::print("tar: not implemented\n");
}

fn cmd_zip(_args: &str) {
    vga::print("zip: not implemented\n");
}

fn cmd_unzip(_args: &str) {
    vga::print("unzip: not implemented\n");
}

fn cmd_history() {
    unsafe {
        if HISTORY_COUNT == 0 {
            vga::print("No commands in history\n");
            return;
        }
        
        let start = if HISTORY_COUNT > HISTORY_SIZE {
            HISTORY_COUNT - HISTORY_SIZE
        } else {
            0
        };
        
        for i in 0..HISTORY_COUNT.min(HISTORY_SIZE) {
            let idx = i % HISTORY_SIZE;
            if HISTORY_LENS[idx] > 0 {
                print_usize(start + i + 1);
                vga::print("  ");
                let cmd = core::str::from_utf8(&HISTORY[idx][..HISTORY_LENS[idx]]).unwrap_or("");
                vga::print(cmd);
                vga::put_char(b'\n');
            }
        }
    }
}


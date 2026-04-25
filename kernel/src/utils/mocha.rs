// mocha - файловый менеджер PINDOS
// Команды: ls, copy, paste, delete, cut, rename, edit, view, exit

use crate::vga;
use crate::fs;
use crate::shell::{read_line, print_usize};

// Буфер для copy/cut
static mut CLIPBOARD: Option<ClipEntry> = None;

struct ClipEntry {
    name: [u8; 32],
    name_len: usize,
    is_cut: bool,
}

pub fn run() {
    vga::clear_screen();
    vga::print_colored("=== mocha - PINDOS File Manager ===\n", 0x0B);
    vga::print("Commands: ls, copy <f>, cut <f>, paste <dst>, delete <f>,\n");
    vga::print("          rename <f> <new>, edit <f>, view <f>, cd <dir>, exit\n\n");

    list_files();

    loop {
        vga::print_colored("mocha:", 0x0E);
        vga::print_colored(fs::cwd(), 0x0B);
        vga::print_colored("> ", 0x0E);        let input = read_line();
        let cmd = input.as_str().trim();

        if cmd.is_empty() {
            continue;
        }

        let (name, args) = split_first(cmd);

        match name {
            "exit" | "q" => {
                vga::print("Returning to shell...\n");
                return;
            }
            "ls" => list_files(),
            "cd" => {
                if args.is_empty() { vga::print("Usage: cd <dir>\n"); }
                else { crate::shell::cmd_cd_pub(args); list_files(); }
            }
            "view" => cmd_view(args),
            "copy" => cmd_copy(args),
            "cut" => cmd_cut(args),
            "paste" => cmd_paste(args),
            "delete" | "del" | "rm" => cmd_delete(args),
            "rename" | "mv" => cmd_rename(args),
            "edit" => {
                if args.is_empty() {
                    vga::print("Usage: edit <file>\n");
                } else {
                    crate::utils::qinn::run(args);
                    // После редактора возвращаемся в mocha
                    vga::clear_screen();
                    vga::print_colored("=== mocha ===\n", 0x0B);
                    list_files();
                }
            }
            "new" => cmd_new(args),
            "help" => {
                vga::print("ls              - list files\n");
                vga::print("cd <dir>        - change directory\n");
                vga::print("view <file>     - view file contents\n");
                vga::print("copy <file>     - copy to clipboard\n");
                vga::print("cut  <file>     - cut to clipboard\n");
                vga::print("paste <name>    - paste from clipboard\n");
                vga::print("delete <file>   - delete file\n");
                vga::print("rename <f> <n>  - rename file\n");
                vga::print("edit <file>     - open in qinn\n");
                vga::print("new <file>      - create empty file\n");
                vga::print("exit            - back to shell\n");
            }
            _ => {
                vga::print("Unknown command. Type 'help'\n");
            }
        }
    }
}

fn list_files() {
    vga::print_colored("Dir: ", 0x08);
    vga::print_colored(fs::cwd(), 0x0E);
    vga::put_char(b'\n');
    vga::print_colored("Files:\n", 0x0F);
    let mut count = 0;
    for f in fs::list_dir(fs::cwd()) {
        if f.is_dir() {
            vga::print("  ");
            vga::print_colored(f.name_str(), 0x0B);
            vga::print_colored("/\n", 0x0B);
        } else {
            vga::print("  ");
            vga::print_colored(f.name_str(), 0x0A);
            vga::print("  (");
            print_usize(f.content_len);
            vga::print(" bytes)\n");
        }
        count += 1;
    }
    if count == 0 {
        vga::print("  (empty)\n");
    }
    vga::print("\n");
}

fn cmd_view(name: &str) {
    if name.is_empty() {
        vga::print("Usage: view <file>\n");
        return;
    }
    if let Some(f) = fs::get(name) {
        vga::print_colored("--- ", 0x08);
        vga::print_colored(name, 0x0F);
        vga::print_colored(" ---\n", 0x08);
        vga::print(f.content_str());
        if !f.content_str().ends_with('\n') {
            vga::put_char(b'\n');
        }
        vga::print_colored("--- end ---\n", 0x08);
    } else {
        vga::print("File not found: ");
        vga::print(name);
        vga::print("\n");
    }
}

fn cmd_copy(name: &str) {
    if name.is_empty() {
        vga::print("Usage: copy <file>\n");
        return;
    }
    if fs::get(name).is_some() {
        set_clipboard(name, false);
        vga::print("Copied: ");
        vga::print(name);
        vga::print("\n");
    } else {
        vga::print("File not found\n");
    }
}

fn cmd_cut(name: &str) {
    if name.is_empty() {
        vga::print("Usage: cut <file>\n");
        return;
    }
    if fs::get(name).is_some() {
        set_clipboard(name, true);
        vga::print("Cut: ");
        vga::print(name);
        vga::print("\n");
    } else {
        vga::print("File not found\n");
    }
}

fn cmd_paste(dst: &str) {
    if dst.is_empty() {
        vga::print("Usage: paste <destination_name>\n");
        return;
    }
    unsafe {
        if let Some(ref clip) = CLIPBOARD {
            let src = core::str::from_utf8(&clip.name[..clip.name_len]).unwrap_or("");
            let is_cut = clip.is_cut;
            if fs::copy_file(src, dst) {
                vga::print("Pasted as: ");
                vga::print(dst);
                vga::print("\n");
                if is_cut {
                    fs::delete(src);
                    CLIPBOARD = None;
                }
            } else {
                vga::print("Paste failed\n");
            }
        } else {
            vga::print("Clipboard is empty\n");
        }
    }
}

fn cmd_delete(name: &str) {
    if name.is_empty() {
        vga::print("Usage: delete <file>\n");
        return;
    }
    vga::print("Delete '");
    vga::print(name);
    vga::print("'? (y/n): ");
    let c = vga::read_char();
    vga::put_char(c);
    vga::put_char(b'\n');
    if c == b'y' || c == b'Y' {
        if fs::delete(name) {
            vga::print("Deleted.\n");
        } else {
            vga::print("File not found.\n");
        }
    } else {
        vga::print("Cancelled.\n");
    }
}

fn cmd_rename(args: &str) {
    let (old, new) = split_first(args);
    if old.is_empty() || new.is_empty() {
        vga::print("Usage: rename <old> <new>\n");
        return;
    }
    if fs::rename(old, new) {
        vga::print("Renamed '");
        vga::print(old);
        vga::print("' -> '");
        vga::print(new);
        vga::print("'\n");
    } else {
        vga::print("Rename failed (file not found or name taken)\n");
    }
}

fn cmd_new(name: &str) {
    if name.is_empty() {
        vga::print("Usage: new <file>\n");
        return;
    }
    if fs::create(name, "") {
        vga::print("Created: ");
        vga::print(name);
        vga::print("\n");
    } else {
        vga::print("File already exists\n");
    }
}

fn set_clipboard(name: &str, is_cut: bool) {
    unsafe {
        let mut entry = ClipEntry {
            name: [0u8; 32],
            name_len: 0,
            is_cut,
        };
        let nb = name.as_bytes();
        let len = nb.len().min(32);
        entry.name[..len].copy_from_slice(&nb[..len]);
        entry.name_len = len;
        CLIPBOARD = Some(entry);
    }
}

fn split_first(s: &str) -> (&str, &str) {
    if let Some(pos) = s.find(' ') {
        (&s[..pos], s[pos + 1..].trim())
    } else {
        (s, "")
    }
}

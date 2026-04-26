// Mell GUI — Mocha файловый менеджер

use crate::mell::vga_gui::*;
use crate::fs;

const MAX_ENTRIES: usize = 32;

pub struct MochaApp {
    pub entries:   [EntryInfo; MAX_ENTRIES],
    pub count:     usize,
    pub selected:  usize,
    pub scroll:    usize,
    pub clipboard: Option<ClipEntry>,
    pub status:    [u8; 64],
    pub status_len: usize,
    pub mode:      MochaMode,
    pub input_buf: [u8; 64],
    pub input_len: usize,
}

#[derive(Copy, Clone)]
pub struct EntryInfo {
    pub name: [u8; 32],
    pub name_len: usize,
    pub is_dir: bool,
    pub size: usize,
}

#[derive(Copy, Clone)]
pub struct ClipEntry {
    pub name: [u8; 32],
    pub name_len: usize,
    pub is_cut: bool,
}

#[derive(Copy, Clone, PartialEq)]
pub enum MochaMode {
    Browse,
    Rename,
    NewFile,
    NewDir,
    Confirm(ConfirmAction),
}

#[derive(Copy, Clone, PartialEq)]
pub enum ConfirmAction { Delete }

impl MochaApp {
    pub fn new() -> Self {
        let mut app = MochaApp {
            entries: [EntryInfo { name: [0u8;32], name_len: 0, is_dir: false, size: 0 }; MAX_ENTRIES],
            count: 0, selected: 0, scroll: 0,
            clipboard: None,
            status: [0u8; 64], status_len: 0,
            mode: MochaMode::Browse,
            input_buf: [0u8; 64], input_len: 0,
        };
        app.refresh();
        app
    }

    pub fn refresh(&mut self) {
        self.count = 0;
        for e in fs::list_dir(fs::cwd()) {
            if self.count >= MAX_ENTRIES { break; }
            let nb = e.name_str().as_bytes();
            let nl = nb.len().min(32);
            self.entries[self.count].name[..nl].copy_from_slice(&nb[..nl]);
            self.entries[self.count].name_len = nl;
            self.entries[self.count].is_dir = e.is_dir();
            self.entries[self.count].size = e.content_len;
            self.count += 1;
        }
        if self.selected >= self.count && self.count > 0 {
            self.selected = self.count - 1;
        }
    }

    fn set_status(&mut self, msg: &str) {
        let b = msg.as_bytes();
        let l = b.len().min(64);
        self.status[..l].copy_from_slice(&b[..l]);
        self.status_len = l;
    }

    fn selected_name(&self) -> &str {
        if self.count == 0 { return ""; }
        core::str::from_utf8(&self.entries[self.selected].name[..self.entries[self.selected].name_len])
            .unwrap_or("")
    }

    // ── Отрисовка ─────────────────────────────────────────────────────────

    pub fn draw(&self, win: &crate::mell::vga_gui::Window) {
        let ix = win.inner_x();
        let iy = win.inner_y();
        let iw = win.inner_w();
        let ih = win.inner_h();

        // Очищаем внутреннюю область
        fill_rect(ix, iy, iw, ih, b' ', MELL_WINDOW);

        // Путь вверху
        let cwd = fs::cwd();
        put_str_at(ix, iy, cwd, color(CYAN, LGRAY));
        draw_hline(ix, iy + 1, iw, color(DGRAY, LGRAY));

        // Список файлов — иконки в сетке (4 колонки)
        let cols = 4;
        let col_w = iw / cols;
        let list_y = iy + 2;
        let visible = (ih - 3).min(self.count);

        for i in 0..visible {
            let idx = i + self.scroll;
            if idx >= self.count { break; }
            let col = i % cols;
            let row = i / cols;
            let ex = ix + col * col_w + 1;
            let ey = list_y + row * 3;
            if ey + 2 >= iy + ih { break; }

            let name = core::str::from_utf8(
                &self.entries[idx].name[..self.entries[idx].name_len]
            ).unwrap_or("?");

            let selected = idx == self.selected;
            if self.entries[idx].is_dir {
                draw_folder_icon(ex, ey, name, selected);
            } else {
                draw_file_icon(ex, ey, name, selected);
            }
        }

        // Статусная строка
        let status_y = iy + ih - 1;
        fill_rect(ix, status_y, iw, 1, b' ', color(WHITE, DGRAY));
        let status = core::str::from_utf8(&self.status[..self.status_len]).unwrap_or("");
        put_str_at(ix, status_y, status, color(WHITE, DGRAY));

        // Строка ввода (если в режиме ввода)
        match self.mode {
            MochaMode::Rename | MochaMode::NewFile | MochaMode::NewDir => {
                let prompt = match self.mode {
                    MochaMode::Rename  => "Rename: ",
                    MochaMode::NewFile => "New file: ",
                    MochaMode::NewDir  => "New dir: ",
                    _ => "",
                };
                let input_y = iy + ih - 2;
                fill_rect(ix, input_y, iw, 1, b' ', WHITE);
                put_str_at_bg(ix, input_y, prompt, BLUE, WHITE);
                let input = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
                put_str_at_bg(ix + prompt.len(), input_y, input, BLACK, WHITE);
                // Курсор
                put_char_at_bg(ix + prompt.len() + self.input_len, input_y, b'_', BLACK, WHITE);
            }
            MochaMode::Confirm(_) => {
                let confirm_y = iy + ih - 2;
                fill_rect(ix, confirm_y, iw, 1, b' ', LRED);
                put_str_at_bg(ix, confirm_y, "Delete? [Enter=Yes  Esc=No]", WHITE, LRED);
            }
            _ => {}
        }

        // Подсказка горячих клавиш
        let hint_y = iy + ih;
        if hint_y < VGA_HEIGHT - 2 {
            put_str_at(ix, hint_y,
                "Enter=Open  D=Del  R=Ren  C=Copy  X=Cut  V=Paste  N=New  M=Mkdir",
                color(DGRAY, LGRAY));
        }
    }

    // ── Обработка клавиш ──────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: Key) {
        match self.mode {
            MochaMode::Browse => self.handle_browse(key),
            MochaMode::Rename | MochaMode::NewFile | MochaMode::NewDir => self.handle_input(key),
            MochaMode::Confirm(action) => self.handle_confirm(key, action),
        }
    }

    fn handle_browse(&mut self, key: Key) {
        match key {
            Key::Right | Key::Down => {
                if self.selected + 1 < self.count { self.selected += 1; }
            }
            Key::Left | Key::Up => {
                if self.selected > 0 { self.selected -= 1; }
            }
            Key::Enter => {
                if self.count == 0 { return; }
                if self.entries[self.selected].is_dir {
                    let name = self.selected_name();
                    let mut buf = [0u8; 128];
                    let new_path = fs::resolve(name, &mut buf);
                    fs::set_cwd(new_path);
                    self.selected = 0;
                    self.scroll = 0;
                    self.refresh();
                    self.set_status("Opened directory");
                }
            }
            Key::Backspace => {
                // Вверх по директории
                let cwd = fs::cwd();
                if cwd != "/" {
                    let (parent, _) = fs::split_path(cwd);
                    fs::set_cwd(parent);
                    self.selected = 0;
                    self.scroll = 0;
                    self.refresh();
                }
            }
            Key::Char(b'd') | Key::Char(b'D') => {
                if self.count > 0 {
                    self.mode = MochaMode::Confirm(ConfirmAction::Delete);
                }
            }
            Key::Char(b'r') | Key::Char(b'R') => {
                if self.count > 0 {
                    self.input_len = 0;
                    self.mode = MochaMode::Rename;
                }
            }
            Key::Char(b'n') | Key::Char(b'N') => {
                self.input_len = 0;
                self.mode = MochaMode::NewFile;
            }
            Key::Char(b'm') | Key::Char(b'M') => {
                self.input_len = 0;
                self.mode = MochaMode::NewDir;
            }
            Key::Char(b'c') | Key::Char(b'C') => {
                if self.count > 0 {
                    let name = self.selected_name();
                    let mut clip = ClipEntry { name: [0u8;32], name_len: 0, is_cut: false };
                    let b = name.as_bytes();
                    let l = b.len().min(32);
                    clip.name[..l].copy_from_slice(&b[..l]);
                    clip.name_len = l;
                    self.clipboard = Some(clip);
                    self.set_status("Copied to clipboard");
                }
            }
            Key::Char(b'x') | Key::Char(b'X') => {
                if self.count > 0 {
                    let name = self.selected_name();
                    let mut clip = ClipEntry { name: [0u8;32], name_len: 0, is_cut: true };
                    let b = name.as_bytes();
                    let l = b.len().min(32);
                    clip.name[..l].copy_from_slice(&b[..l]);
                    clip.name_len = l;
                    self.clipboard = Some(clip);
                    self.set_status("Cut to clipboard");
                }
            }
            Key::Char(b'v') | Key::Char(b'V') => {
                if let Some(clip) = self.clipboard {
                    let src = core::str::from_utf8(&clip.name[..clip.name_len]).unwrap_or("");
                    // Вставляем в текущую директорию с тем же именем
                    if fs::copy_file(src, src) {
                        if clip.is_cut { fs::delete(src); self.clipboard = None; }
                        self.set_status("Pasted");
                        self.refresh();
                    }
                }
            }
            Key::Char(b'f') | Key::Char(b'F') => {
                self.refresh();
                self.set_status("Refreshed");
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Esc => { self.mode = MochaMode::Browse; self.input_len = 0; }
            Key::Backspace => { if self.input_len > 0 { self.input_len -= 1; } }
            Key::Enter => {
                let input = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
                match self.mode {
                    MochaMode::Rename => {
                        let old = self.selected_name();
                        if fs::rename(old, input) { self.set_status("Renamed"); }
                        else { self.set_status("Rename failed"); }
                    }
                    MochaMode::NewFile => {
                        if fs::create(input, "") { self.set_status("File created"); }
                        else { self.set_status("Create failed"); }
                    }
                    MochaMode::NewDir => {
                        if fs::mkdir(input) { self.set_status("Dir created"); }
                        else { self.set_status("Mkdir failed"); }
                    }
                    _ => {}
                }
                self.mode = MochaMode::Browse;
                self.input_len = 0;
                self.refresh();
            }
            Key::Char(c) => {
                if self.input_len < 63 {
                    self.input_buf[self.input_len] = c;
                    self.input_len += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_confirm(&mut self, key: Key, action: ConfirmAction) {
        match key {
            Key::Enter => {
                match action {
                    ConfirmAction::Delete => {
                        let name = self.selected_name();
                        if fs::delete(name) { self.set_status("Deleted"); }
                        else { self.set_status("Delete failed"); }
                        self.refresh();
                    }
                }
                self.mode = MochaMode::Browse;
            }
            Key::Esc => { self.mode = MochaMode::Browse; }
            _ => {}
        }
    }

    // ── Обработка мыши ────────────────────────────────────────────────────

    pub fn handle_mouse(&mut self, x: usize, y: usize) {
        let cols = 4;
        let col_w = 12;
        let list_y = 2;
        if y >= list_y {
            let row = (y - list_y) / 3;
            let col = x / col_w;
            let idx = row * cols + col;
            if idx < self.count {
                self.selected = idx;
                if self.entries[self.selected].is_dir {
                    let name = self.selected_name();
                    let mut buf = [0u8; 128];
                    let new_path = fs::resolve(name, &mut buf);
                    fs::set_cwd(new_path);
                    self.selected = 0;
                    self.scroll = 0;
                    self.refresh();
                    self.set_status("Opened directory");
                }
            }
        }
    }
}
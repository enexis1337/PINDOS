// Mell GUI — Qinn текстовый редактор

use crate::mell::vga_gui::*;
use crate::fs;

const MAX_LINES: usize = 64;   // было 128 — экономим ~16KB
const MAX_LINE:  usize = 128;  // было 256 — экономим ещё ~8KB

pub struct QinnApp {
    pub lines:      [[u8; MAX_LINE]; MAX_LINES],
    pub line_lens:  [usize; MAX_LINES],
    pub line_count: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub filename:   [u8; 64],
    pub fname_len:  usize,
    pub modified:   bool,
    pub mode:       QinnMode,
    pub input_buf:  [u8; 64],
    pub input_len:  usize,
    pub status:     [u8; 80],
    pub status_len: usize,
}

#[derive(Copy, Clone, PartialEq)]
pub enum QinnMode {
    Edit,
    OpenFile,
    SaveAs,
    Confirm, // выход без сохранения
}

impl QinnApp {
    pub fn new() -> Self {
        let mut app = QinnApp {
            lines: [[0u8; MAX_LINE]; MAX_LINES],
            line_lens: [0usize; MAX_LINES],
            line_count: 1,
            cursor_row: 0, cursor_col: 0, scroll_row: 0,
            filename: [0u8; 64], fname_len: 0,
            modified: false,
            mode: QinnMode::Edit,
            input_buf: [0u8; 64], input_len: 0,
            status: [0u8; 80], status_len: 0,
        };
        app.set_status("Ctrl+S=Save  Ctrl+O=Open  Ctrl+W=Close  Ctrl+N=New");
        app
    }

    pub fn open_file(&mut self, name: &str) {
        if let Some(f) = fs::get(name) {
            self.line_count = 0;
            let mut row = 0;
            let mut col = 0;
            for b in f.content_str().bytes() {
                if b == b'\n' {
                    self.line_lens[row] = col;
                    row += 1; col = 0;
                    if row >= MAX_LINES { break; }
                } else if col < MAX_LINE {
                    self.lines[row][col] = b; col += 1;
                }
            }
            self.line_lens[row] = col;
            self.line_count = row + 1;
            let nb = name.as_bytes();
            let nl = nb.len().min(64);
            self.filename[..nl].copy_from_slice(&nb[..nl]);
            self.fname_len = nl;
            self.cursor_row = 0; self.cursor_col = 0; self.scroll_row = 0;
            self.modified = false;
            self.set_status("File opened");
        } else {
            self.set_status("File not found");
        }
    }

    pub fn save(&mut self) {
        if self.fname_len == 0 {
            self.mode = QinnMode::SaveAs;
            self.input_len = 0;
            return;
        }
        self.do_save();
    }

    fn do_save(&mut self) {
        let fname = core::str::from_utf8(&self.filename[..self.fname_len]).unwrap_or("");
        let mut buf = [0u8; 16384];
        let mut pos = 0;
        for i in 0..self.line_count {
            for j in 0..self.line_lens[i] {
                if pos < buf.len() { buf[pos] = self.lines[i][j]; pos += 1; }
            }
            if pos < buf.len() { buf[pos] = b'\n'; pos += 1; }
        }
        let content = core::str::from_utf8(&buf[..pos]).unwrap_or("");
        if fs::get(fname).is_some() { fs::write(fname, content); }
        else { fs::create(fname, content); }
        self.modified = false;
        self.set_status("Saved");
    }

    fn set_status(&mut self, msg: &str) {
        let b = msg.as_bytes();
        let l = b.len().min(80);
        self.status[..l].copy_from_slice(&b[..l]);
        self.status_len = l;
    }

    fn fname_str(&self) -> &str {
        core::str::from_utf8(&self.filename[..self.fname_len]).unwrap_or("untitled")
    }

    // ── Отрисовка ─────────────────────────────────────────────────────────

    pub fn draw(&self, win: &Window) {
        let ix = win.inner_x();
        let iy = win.inner_y();
        let iw = win.inner_w();
        let ih = win.inner_h();

        fill_rect(ix, iy, iw, ih, b' ', color(WHITE, BLACK));

        // Строка с именем файла
        let header = color(WHITE, DGRAY);
        fill_rect(ix, iy, iw, 1, b' ', header);
        put_str_at(ix, iy, self.fname_str(), header);
        if self.modified { put_str_at(ix + self.fname_len + 1, iy, "[+]", color(YELLOW, DGRAY)); }

        // Текст
        let text_h = ih.saturating_sub(3);
        for row in 0..text_h {
            let line_idx = row + self.scroll_row;
            if line_idx >= self.line_count { break; }

            // Номер строки
            let ln_attr = color(DGRAY, BLACK);
            draw_line_num(ix, iy + 1 + row, line_idx + 1, ln_attr);

            let text_x = ix + 4;
            let len = self.line_lens[line_idx];
            for col in 0..len.min(iw.saturating_sub(5)) {
                let c = self.lines[line_idx][col];
                let is_cursor = line_idx == self.cursor_row && col == self.cursor_col;
                put_char_at_bg(text_x + col, iy + 1 + row, c,
                    if is_cursor { BLACK } else { WHITE },
                    if is_cursor { WHITE } else { BLACK });
            }
            // Курсор в конце строки
            if line_idx == self.cursor_row && self.cursor_col == len {
                put_char_at_bg(text_x + len, iy + 1 + row, b'_', BLACK, WHITE);
            }
        }

        // Статусная строка
        let st_y = iy + ih - 2;
        fill_rect(ix, st_y, iw, 1, b' ', color(WHITE, DGRAY));
        let status = core::str::from_utf8(&self.status[..self.status_len]).unwrap_or("");
        put_str_at(ix, st_y, status, color(WHITE, DGRAY));

        // Позиция курсора
        let pos_y = iy + ih - 1;
        fill_rect(ix, pos_y, iw, 1, b' ', color(DGRAY, BLACK));
        put_str_at(ix, pos_y, "Ln:", color(CYAN, BLACK));
        put_usize(ix + 3, pos_y, self.cursor_row + 1, color(WHITE, BLACK));
        put_str_at(ix + 7, pos_y, "Col:", color(CYAN, BLACK));
        put_usize(ix + 11, pos_y, self.cursor_col + 1, color(WHITE, BLACK));

        // Диалог ввода
        match self.mode {
            QinnMode::OpenFile | QinnMode::SaveAs => {
                let prompt = if self.mode == QinnMode::OpenFile { "Open: " } else { "Save as: " };
                let dlg_y = iy + ih / 2;
                fill_rect(ix + 2, dlg_y, iw - 4, 1, b' ', WHITE);
                put_str_at_bg(ix + 2, dlg_y, prompt, BLUE, WHITE);
                let input = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
                put_str_at_bg(ix + 2 + prompt.len(), dlg_y, input, BLACK, WHITE);
                put_char_at_bg(ix + 2 + prompt.len() + self.input_len, dlg_y, b'_', BLACK, WHITE);
            }
            QinnMode::Confirm => {
                let dlg_y = iy + ih / 2;
                fill_rect(ix + 2, dlg_y, iw - 4, 1, b' ', LRED);
                put_str_at_bg(ix + 2, dlg_y, "Unsaved changes! Close? [Enter/Esc]", WHITE, LRED);
            }
            _ => {}
        }
    }

    // ── Обработка клавиш ──────────────────────────────────────────────────

    /// Возвращает true если нужно закрыть окно
    pub fn handle_key(&mut self, key: Key) -> bool {
        match self.mode {
            QinnMode::Edit    => self.handle_edit(key),
            QinnMode::OpenFile | QinnMode::SaveAs => self.handle_dialog(key),
            QinnMode::Confirm => self.handle_confirm(key),
        }
    }

    fn handle_edit(&mut self, key: Key) -> bool {
        match key {
            Key::Char(0x13) => { self.save(); false } // Ctrl+S
            Key::Char(0x0F) => { // Ctrl+O
                self.mode = QinnMode::OpenFile;
                self.input_len = 0;
                false
            }
            Key::Char(0x0E) => { // Ctrl+N
                self.line_count = 1;
                self.line_lens[0] = 0;
                self.cursor_row = 0; self.cursor_col = 0;
                self.fname_len = 0;
                self.modified = false;
                self.set_status("New file");
                false
            }
            Key::Char(0x17) => { // Ctrl+W — закрыть
                if self.modified {
                    self.mode = QinnMode::Confirm;
                    false
                } else { true }
            }
            Key::Up    => { self.move_cursor(-1, 0); false }
            Key::Down  => { self.move_cursor(1, 0); false }
            Key::Left  => { self.move_cursor(0, -1); false }
            Key::Right => { self.move_cursor(0, 1); false }
            Key::Home  => { self.cursor_col = 0; false }
            Key::End   => { self.cursor_col = self.line_lens[self.cursor_row]; false }
            Key::PageUp => {
                self.cursor_row = self.cursor_row.saturating_sub(10);
                self.adjust_scroll();
                false
            }
            Key::PageDown => {
                self.cursor_row = (self.cursor_row + 10).min(self.line_count - 1);
                self.adjust_scroll();
                false
            }
            Key::Enter => { self.insert_newline(); false }
            Key::Backspace => { self.backspace(); false }
            Key::Char(c) if c >= 0x20 => { self.insert_char(c); false }
            _ => false,
        }
    }

    fn handle_dialog(&mut self, key: Key) -> bool {
        match key {
            Key::Esc => { self.mode = QinnMode::Edit; false }
            Key::Backspace => { if self.input_len > 0 { self.input_len -= 1; } false }
            Key::Enter => {
                let mut input_copy = [0u8; 64];
                let input_len = self.input_len;
                input_copy[..input_len].copy_from_slice(&self.input_buf[..input_len]);
                let input = core::str::from_utf8(&input_copy[..input_len]).unwrap_or("");
                match self.mode {
                    QinnMode::OpenFile => self.open_file(input),
                    QinnMode::SaveAs => {
                        let nb = input.as_bytes();
                        let nl = nb.len().min(64);
                        self.filename[..nl].copy_from_slice(&nb[..nl]);
                        self.fname_len = nl;
                        self.do_save();
                    }
                    _ => {}
                }
                self.mode = QinnMode::Edit;
                self.input_len = 0;
                false
            }
            Key::Char(c) => {
                if self.input_len < 63 { self.input_buf[self.input_len] = c; self.input_len += 1; }
                false
            }
            _ => false,
        }
    }

    fn handle_confirm(&mut self, key: Key) -> bool {
        match key {
            Key::Enter => true,
            Key::Esc   => { self.mode = QinnMode::Edit; false }
            _ => false,
        }
    }

    // ── Редактирование ────────────────────────────────────────────────────

    fn insert_char(&mut self, c: u8) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let len = self.line_lens[row];
        if len >= MAX_LINE - 1 { return; }
        let mut i = len;
        while i > col { self.lines[row][i] = self.lines[row][i-1]; i -= 1; }
        self.lines[row][col] = c;
        self.line_lens[row] += 1;
        self.cursor_col += 1;
        self.modified = true;
    }

    fn insert_newline(&mut self) {
        if self.line_count >= MAX_LINES { return; }
        let row = self.cursor_row;
        let col = self.cursor_col;
        let old_len = self.line_lens[row];
        // Сдвигаем строки вниз
        let mut i = self.line_count;
        while i > row + 1 {
            self.lines[i] = self.lines[i-1];
            self.line_lens[i] = self.line_lens[i-1];
            i -= 1;
        }
        // Разбиваем строку
        self.line_lens[row] = col;
        let new_len = old_len - col;
        for j in 0..new_len { self.lines[row+1][j] = self.lines[row][col+j]; }
        self.line_lens[row+1] = new_len;
        self.line_count += 1;
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.adjust_scroll();
        self.modified = true;
    }

    fn backspace(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        if col > 0 {
            let len = self.line_lens[row];
            let mut i = col - 1;
            while i < len - 1 { self.lines[row][i] = self.lines[row][i+1]; i += 1; }
            self.line_lens[row] -= 1;
            self.cursor_col -= 1;
        } else if row > 0 {
            let prev_len = self.line_lens[row-1];
            let cur_len = self.line_lens[row];
            for j in 0..cur_len {
                if prev_len + j < MAX_LINE { self.lines[row-1][prev_len+j] = self.lines[row][j]; }
            }
            self.line_lens[row-1] = prev_len + cur_len;
            let mut i = row;
            while i < self.line_count - 1 {
                self.lines[i] = self.lines[i+1];
                self.line_lens[i] = self.line_lens[i+1];
                i += 1;
            }
            self.line_count -= 1;
            self.cursor_row -= 1;
            self.cursor_col = prev_len;
        }
        self.modified = true;
    }

    fn move_cursor(&mut self, dr: i32, dc: i32) {
        let new_row = (self.cursor_row as i32 + dr)
            .max(0).min(self.line_count as i32 - 1) as usize;
        self.cursor_row = new_row;
        let len = self.line_lens[self.cursor_row];
        let new_col = (self.cursor_col as i32 + dc).max(0).min(len as i32) as usize;
        self.cursor_col = new_col;
        self.adjust_scroll();
    }

    fn adjust_scroll(&mut self) {
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        }
    }

    pub fn handle_mouse(&mut self, x: usize, y: usize) {
        if y >= 1 && x >= 4 {
            let text_row = y - 1 + self.scroll_row;
            let text_col = x - 4;
            if text_row < self.line_count {
                self.cursor_row = text_row;
                self.cursor_col = text_col.min(self.line_lens[text_row]);
                self.adjust_scroll();
            }
        }
    }
}

fn draw_line_num(x: usize, y: usize, n: usize, attr: u32) {
    let mut buf = [b' '; 3];
    if n < 10 { buf[2] = b'0' + n as u8; }
    else if n < 100 { buf[1] = b'0' + (n/10) as u8; buf[2] = b'0' + (n%10) as u8; }
    else { buf[0] = b'0' + (n/100) as u8; buf[1] = b'0' + ((n/10)%10) as u8; buf[2] = b'0' + (n%10) as u8; }
    for (i, &c) in buf.iter().enumerate() { put_char_at(x+i, y, c, attr); }
    put_char_at(x+3, y, b'|', attr);
}

fn put_usize(x: usize, y: usize, n: usize, attr: u32) {
    let mut buf = [0u8; 8];
    let mut i = 0;
    let mut n = n;
    if n == 0 { put_char_at(x, y, b'0', attr); return; }
    while n > 0 { buf[i] = b'0' + (n%10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { put_char_at(x + (i-1-j), y, buf[j], attr); }
}
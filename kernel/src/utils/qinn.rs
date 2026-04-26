// qinn - текстовый редактор PINDOS
// qinn is not notepad
//
// Управление:
//   Ctrl+S  - сохранить
//   Ctrl+Q  - выйти без сохранения
//   Ctrl+X  - сохранить и выйти
//   Стрелки - перемещение (через escape sequences)
//   Backspace - удалить символ

use crate::vga;
use crate::fs;

const ROWS: usize = 22;   // строк для текста (оставляем 2 для статусбара)
const COLS: usize = 80;
const MAX_LINES: usize = 256;
const MAX_LINE_LEN: usize = 256;

struct Editor {
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    line_lens: [usize; MAX_LINES],
    line_count: usize,
    cursor_row: usize,
    cursor_col: usize,
    scroll_row: usize,
    filename: [u8; 32],
    filename_len: usize,
    modified: bool,
}

impl Editor {
    fn new() -> Self {
        Editor {
            lines: [[0u8; MAX_LINE_LEN]; MAX_LINES],
            line_lens: [0usize; MAX_LINES],
            line_count: 1,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            filename: [0u8; 32],
            filename_len: 0,
            modified: false,
        }
    }

    fn load(&mut self, name: &str, content: &str) {
        let nb = name.as_bytes();
        let nlen = nb.len().min(32);
        self.filename[..nlen].copy_from_slice(&nb[..nlen]);
        self.filename_len = nlen;

        self.line_count = 0;
        let mut row = 0;
        let mut col = 0;

        for b in content.bytes() {
            if b == b'\n' {
                self.line_lens[row] = col;
                row += 1;
                col = 0;
                if row >= MAX_LINES { break; }
            } else if col < MAX_LINE_LEN {
                self.lines[row][col] = b;
                col += 1;
            }
        }
        self.line_lens[row] = col;
        self.line_count = row + 1;
        if self.line_count == 0 { self.line_count = 1; }
    }

    fn to_string(&self, buf: &mut [u8; 65536]) -> usize {
        let mut pos = 0;
        for i in 0..self.line_count {
            let len = self.line_lens[i];
            for j in 0..len {
                if pos < buf.len() {
                    buf[pos] = self.lines[i][j];
                    pos += 1;
                }
            }
            if pos < buf.len() {
                buf[pos] = b'\n';
                pos += 1;
            }
        }
        pos
    }

    fn filename_str(&self) -> &str {
        core::str::from_utf8(&self.filename[..self.filename_len]).unwrap_or("untitled")
    }

    fn insert_char(&mut self, c: u8) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let len = self.line_lens[row];
        if len < MAX_LINE_LEN - 1 {
            // Сдвигаем символы вправо
            let mut i = len;
            while i > col {
                self.lines[row][i] = self.lines[row][i - 1];
                i -= 1;
            }
            self.lines[row][col] = c;
            self.line_lens[row] += 1;
            self.cursor_col += 1;
            self.modified = true;
        }
    }

    fn insert_newline(&mut self) {
        if self.line_count >= MAX_LINES { return; }
        let row = self.cursor_row;
        let col = self.cursor_col;

        // Сдвигаем строки вниз
        let mut i = self.line_count;
        while i > row + 1 {
            self.lines[i] = self.lines[i - 1];
            self.line_lens[i] = self.line_lens[i - 1];
            i -= 1;
        }

        // Разбиваем текущую строку
        let old_len = self.line_lens[row];
        self.line_lens[row] = col;

        let new_len = old_len - col;
        for j in 0..new_len {
            self.lines[row + 1][j] = self.lines[row][col + j];
        }
        self.line_lens[row + 1] = new_len;

        self.line_count += 1;
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.modified = true;
    }

    fn backspace(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;

        if col > 0 {
            // Удаляем символ в строке
            let len = self.line_lens[row];
            let mut i = col - 1;
            while i < len - 1 {
                self.lines[row][i] = self.lines[row][i + 1];
                i += 1;
            }
            self.line_lens[row] -= 1;
            self.cursor_col -= 1;
            self.modified = true;
        } else if row > 0 {
            // Объединяем с предыдущей строкой
            let prev_len = self.line_lens[row - 1];
            let cur_len = self.line_lens[row];
            for j in 0..cur_len {
                if prev_len + j < MAX_LINE_LEN {
                    self.lines[row - 1][prev_len + j] = self.lines[row][j];
                }
            }
            self.line_lens[row - 1] = prev_len + cur_len;

            // Сдвигаем строки вверх
            let mut i = row;
            while i < self.line_count - 1 {
                self.lines[i] = self.lines[i + 1];
                self.line_lens[i] = self.line_lens[i + 1];
                i += 1;
            }
            self.line_count -= 1;
            self.cursor_row -= 1;
            self.cursor_col = prev_len;
            self.modified = true;
        }
    }

    fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let len = self.line_lens[self.cursor_row];
            if self.cursor_col > len { self.cursor_col = len; }
            if self.cursor_row < self.scroll_row {
                self.scroll_row = self.cursor_row;
            }
        }
    }

    fn move_down(&mut self) {
        if self.cursor_row + 1 < self.line_count {
            self.cursor_row += 1;
            let len = self.line_lens[self.cursor_row];
            if self.cursor_col > len { self.cursor_col = len; }
            if self.cursor_row >= self.scroll_row + ROWS {
                self.scroll_row += 1;
            }
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.line_lens[self.cursor_row];
        }
    }

    fn move_right(&mut self) {
        let len = self.line_lens[self.cursor_row];
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.line_count {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }
}

pub fn run(filename: &str) {
    let mut editor = Editor::new();

    // Загружаем файл или создаём новый
    if let Some(f) = fs::get(filename) {
        editor.load(filename, f.content_str());
    } else {
        fs::create(filename, "");
        editor.load(filename, "");
    }

    render(&editor);

    loop {
        let c = vga::read_char();

        match c {
            // Ctrl+S = 0x13 (19)
            0x13 => {
                save(&editor);
                render(&editor);
            }
            // Ctrl+Q = 0x11 (17)
            0x11 => {
                return;
            }
            // Ctrl+X = 0x18 (24)
            0x18 => {
                save(&editor);
                return;
            }
            // Escape - обрабатываем стрелки
            0x1B => {
                handle_escape(&mut editor);
                render(&editor);
            }
            b'\n' => {
                editor.insert_newline();
                render(&editor);
            }
            b'\x08' => {
                editor.backspace();
                render(&editor);
            }
            32..=126 => {
                editor.insert_char(c);
                render(&editor);
            }
            _ => {}
        }
    }
}

fn handle_escape(editor: &mut Editor) {
    // Читаем escape sequence для стрелок: ESC [ A/B/C/D
    let b1 = vga::read_char();
    if b1 != b'[' { return; }
    let b2 = vga::read_char();
    match b2 {
        b'A' => editor.move_up(),
        b'B' => editor.move_down(),
        b'C' => editor.move_right(),
        b'D' => editor.move_left(),
        _ => {}
    }
}

fn save(editor: &Editor) {
    let mut buf = [0u8; 65536];
    let len = editor.to_string(&mut buf);
    let content = core::str::from_utf8(&buf[..len]).unwrap_or("");
    fs::write(editor.filename_str(), content);
    // Показываем статус
    draw_status(editor, true);
}

fn render(editor: &Editor) {
    vga::clear_screen();

    // Заголовок
    vga::print_colored("qinn | ", 0x0B);
    vga::print_colored(editor.filename_str(), 0x0F);
    if editor.modified {
        vga::print_colored(" [modified]", 0x0C);
    }
    vga::print_colored(" | ^S save  ^X save+exit  ^Q quit\n", 0x08);

    // Разделитель
    for _ in 0..COLS {
        vga::print_colored("-", 0x08);
    }
    vga::put_char(b'\n');

    // Строки текста
    let end = (editor.scroll_row + ROWS).min(editor.line_count);
    for row in editor.scroll_row..end {
        let len = editor.line_lens[row];
        // Номер строки
        print_line_num(row + 1);
        vga::print_colored(" | ", 0x08);

        for col in 0..len {
            // Подсвечиваем курсор
            if row == editor.cursor_row && col == editor.cursor_col {
                vga::print_colored(
                    core::str::from_utf8(&[editor.lines[row][col]]).unwrap_or(" "),
                    0x70, // инвертированный
                );
            } else {
                vga::put_char(editor.lines[row][col]);
            }
        }

        // Курсор в конце строки
        if row == editor.cursor_row && editor.cursor_col == len {
            vga::print_colored("_", 0x70);
        }

        vga::put_char(b'\n');
    }

    draw_status(editor, false);
}

fn draw_status(editor: &Editor, saved: bool) {
    // Статусная строка внизу
    vga::print_colored("Ln:", 0x08);
    crate::uglyshell::print_usize(editor.cursor_row + 1);
    vga::print_colored(" Col:", 0x08);
    crate::uglyshell::print_usize(editor.cursor_col + 1);
    if saved {
        vga::print_colored("  [Saved]", 0x0A);
    }
}

fn print_line_num(n: usize) {
    // Выводим номер строки с выравниванием (3 цифры)
    if n < 10 {
        vga::print_colored("  ", 0x08);
        let buf = [b'0' + n as u8];
        vga::print_colored(core::str::from_utf8(&buf).unwrap_or("?"), 0x08);
    } else if n < 100 {
        vga::print_colored(" ", 0x08);
        let buf = [b'0' + (n / 10) as u8, b'0' + (n % 10) as u8];
        vga::print_colored(core::str::from_utf8(&buf).unwrap_or("??"), 0x08);
    } else {
        let buf = [
            b'0' + (n / 100) as u8,
            b'0' + ((n / 10) % 10) as u8,
            b'0' + (n % 10) as u8,
        ];
        vga::print_colored(core::str::from_utf8(&buf).unwrap_or("???"), 0x08);
    }
}

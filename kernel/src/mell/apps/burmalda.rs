// Mell GUI — Burmalda (полноценный эмулятор терминала)

use crate::mell::vga_gui::*;

const TERMINAL_ROWS: usize = 25;
const TERMINAL_COLS: usize = 80;
const HISTORY_LINES: usize = 200;
const MAX_ESC_SEQ: usize = 32;

// Конвертация VGA цветов (0-15) в RGB
fn vga_to_rgb(vga_color: u8) -> u32 {
    match vga_color {
        0  => BLACK,    // 0x000000
        1  => BLUE,     // 0x0055AA
        2  => GREEN,    // 0x008800
        3  => CYAN,     // 0x008888
        4  => RED,      // 0xAA0000
        5  => MAGENTA,  // 0xAA00AA
        6  => BROWN,    // 0xAA5500
        7  => LGRAY,    // 0xC0C0C0
        8  => DGRAY,    // 0x404040
        9  => LBLUE,    // 0x5555FF
        10 => LGREEN,   // 0x55FF55
        11 => LCYAN,    // 0x55FFFF
        12 => LRED,     // 0xFF5555
        13 => LMAGENTA, // 0xFF55FF
        14 => YELLOW,   // 0xFFFF55
        15 => WHITE,    // 0xFFFFFF
        _  => LGRAY,    // По умолчанию
    }
}

#[derive(Copy, Clone)]
pub struct TerminalCell {
    pub ch: u8,
    pub fg: u8,
    pub bg: u8,
}

impl Default for TerminalCell {
    fn default() -> Self {
        TerminalCell { ch: b' ', fg: 7, bg: 0 } // VGA цвета как u8
    }
}

#[derive(Copy, Clone)]
pub enum TerminalState {
    Normal,
    Escape,
    CSI,
    OSC,
}

pub struct BurmaldaApp {
    // Терминальный буфер
    pub screen: [[TerminalCell; TERMINAL_COLS]; TERMINAL_ROWS],
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_visible: bool,
    pub cursor_blink: bool,
    
    // Цвета и атрибуты
    pub current_fg: u8,
    pub current_bg: u8,
    pub bold: bool,
    pub underline: bool,
    pub reverse: bool,
    
    // Скролл и история
    pub scroll_top: usize,
    pub scroll_bottom: usize,
    pub history: [[TerminalCell; TERMINAL_COLS]; HISTORY_LINES],
    pub history_count: usize,
    pub scroll_offset: usize,
    
    // ANSI escape sequence парсер
    pub state: TerminalState,
    pub esc_buffer: [u8; MAX_ESC_SEQ],
    pub esc_len: usize,
    
    // Ввод
    pub input_mode: bool,
    pub input_buffer: [u8; 256],
    pub input_len: usize,
    pub input_pos: usize,
    
    // История команд
    pub cmd_history: [[u8; 256]; 32],
    pub cmd_lens: [usize; 32],
    pub cmd_count: usize,
    pub cmd_pos: usize,
    
    // Табуляторы
    pub tab_stops: [bool; TERMINAL_COLS],
    
    // Сохраненная позиция курсора
    pub saved_x: usize,
    pub saved_y: usize,
}

impl BurmaldaApp {
    pub fn new() -> Self {
        let mut app = BurmaldaApp {
            screen: [[TerminalCell::default(); TERMINAL_COLS]; TERMINAL_ROWS],
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            cursor_blink: true,
            
            current_fg: 7, // LGRAY
            current_bg: 0, // BLACK
            bold: false,
            underline: false,
            reverse: false,
            
            scroll_top: 0,
            scroll_bottom: TERMINAL_ROWS - 1,
            history: [[TerminalCell::default(); TERMINAL_COLS]; HISTORY_LINES],
            history_count: 0,
            scroll_offset: 0,
            
            state: TerminalState::Normal,
            esc_buffer: [0; MAX_ESC_SEQ],
            esc_len: 0,
            
            input_mode: true,
            input_buffer: [0; 256],
            input_len: 0,
            input_pos: 0,
            
            cmd_history: [[0; 256]; 32],
            cmd_lens: [0; 32],
            cmd_count: 0,
            cmd_pos: 0,
            
            tab_stops: [false; TERMINAL_COLS],
            saved_x: 0,
            saved_y: 0,
        };
        
        // Устанавливаем табуляторы каждые 8 позиций
        for i in (8..TERMINAL_COLS).step_by(8) {
            app.tab_stops[i] = true;
        }
        
        app.clear_screen();
        app.print_welcome();
        app
    }
    
    fn print_welcome(&mut self) {
        self.write_str("Burmalda v1.0\r\n");
        self.write_str("ANSI/VT100 compatible\r\n");
        self.write_str("Type 'help' for commands\r\n\r\n");
        self.show_prompt();
    }
    
    fn show_prompt(&mut self) {
        let cwd = crate::fs::cwd();
        let user = crate::auth::current_name();
        
        // Цветной промпт
        self.set_color(10, 0); // LGREEN
        self.write_str(user);
        self.set_color(7, 0);  // LGRAY
        self.write_str("@");
        self.set_color(10, 0); // LGREEN
        self.write_str("pindos");
        self.set_color(7, 0);  // LGRAY
        self.write_str(":");
        self.set_color(11, 0); // LCYAN
        self.write_str(cwd);
        self.set_color(15, 0); // WHITE
        self.write_str("$ ");
        
        self.input_mode = true;
        self.input_len = 0;
        self.input_pos = 0;
    }

    // ── Основные терминальные функции ─────────────────────────────────────

    pub fn write_char(&mut self, ch: u8) {
        match self.state {
            TerminalState::Normal => self.handle_normal_char(ch),
            TerminalState::Escape => self.handle_escape_char(ch),
            TerminalState::CSI => self.handle_csi_char(ch),
            TerminalState::OSC => self.handle_osc_char(ch),
        }
    }
    
    pub fn write_str(&mut self, s: &str) {
        for &ch in s.as_bytes() {
            self.write_char(ch);
        }
    }
    
    fn handle_normal_char(&mut self, ch: u8) {
        match ch {
            0x1B => { // ESC
                self.state = TerminalState::Escape;
                self.esc_len = 0;
            }
            b'\r' => {
                self.cursor_x = 0;
            }
            b'\n' => {
                self.cursor_x = 0;
                self.cursor_y += 1;
                if self.cursor_y > self.scroll_bottom {
                    self.scroll_up();
                    self.cursor_y = self.scroll_bottom;
                }
            }
            b'\t' => {
                // Переход к следующему табулятору
                loop {
                    self.cursor_x += 1;
                    if self.cursor_x >= TERMINAL_COLS {
                        self.cursor_x = 0;
                        self.cursor_y += 1;
                        if self.cursor_y > self.scroll_bottom {
                            self.scroll_up();
                            self.cursor_y = self.scroll_bottom;
                        }
                        break;
                    }
                    if self.tab_stops[self.cursor_x] {
                        break;
                    }
                }
            }
            b'\x08' => { // Backspace
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            b'\x07' => { // Bell - игнорируем
            }
            _ if ch >= 0x20 => {
                self.put_char_at_cursor(ch);
                self.cursor_x += 1;
                if self.cursor_x >= TERMINAL_COLS {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                    if self.cursor_y > self.scroll_bottom {
                        self.scroll_up();
                        self.cursor_y = self.scroll_bottom;
                    }
                }
            }
            _ => {} // Игнорируем управляющие символы
        }
    }
    
    fn handle_escape_char(&mut self, ch: u8) {
        match ch {
            b'[' => {
                self.state = TerminalState::CSI;
                self.esc_len = 0;
            }
            b']' => {
                self.state = TerminalState::OSC;
                self.esc_len = 0;
            }
            b'D' => { // Index (IND)
                self.cursor_y += 1;
                if self.cursor_y > self.scroll_bottom {
                    self.scroll_up();
                    self.cursor_y = self.scroll_bottom;
                }
                self.state = TerminalState::Normal;
            }
            b'M' => { // Reverse Index (RI)
                if self.cursor_y > self.scroll_top {
                    self.cursor_y -= 1;
                } else {
                    self.scroll_down();
                }
                self.state = TerminalState::Normal;
            }
            b'E' => { // Next Line (NEL)
                self.cursor_x = 0;
                self.cursor_y += 1;
                if self.cursor_y > self.scroll_bottom {
                    self.scroll_up();
                    self.cursor_y = self.scroll_bottom;
                }
                self.state = TerminalState::Normal;
            }
            b'7' => { // Save cursor (DECSC)
                self.saved_x = self.cursor_x;
                self.saved_y = self.cursor_y;
                self.state = TerminalState::Normal;
            }
            b'8' => { // Restore cursor (DECRC)
                self.cursor_x = self.saved_x;
                self.cursor_y = self.saved_y;
                self.state = TerminalState::Normal;
            }
            b'c' => { // Reset (RIS)
                self.reset_terminal();
                self.state = TerminalState::Normal;
            }
            _ => {
                self.state = TerminalState::Normal;
            }
        }
    }
    
    fn handle_csi_char(&mut self, ch: u8) {
        if self.esc_len < MAX_ESC_SEQ - 1 {
            self.esc_buffer[self.esc_len] = ch;
            self.esc_len += 1;
        }
        
        // Проверяем завершающий символ
        if ch >= 0x40 && ch <= 0x7E {
            self.execute_csi_sequence();
            self.state = TerminalState::Normal;
        }
    }
    
    fn handle_osc_char(&mut self, ch: u8) {
        if ch == 0x07 || ch == 0x1B { // BEL или ESC
            // Завершение OSC последовательности
            self.state = TerminalState::Normal;
        } else if self.esc_len < MAX_ESC_SEQ - 1 {
            self.esc_buffer[self.esc_len] = ch;
            self.esc_len += 1;
        }
    }
    
    fn execute_csi_sequence(&mut self) {
        if self.esc_len == 0 { return; }
        
        let cmd = self.esc_buffer[self.esc_len - 1];
        let params = &self.esc_buffer[..self.esc_len - 1];
        
        // Парсим параметры
        let mut args = [0u16; 8];
        let mut arg_count = 0;
        let mut current_arg = 0u16;
        
        for &b in params {
            if b >= b'0' && b <= b'9' {
                current_arg = current_arg * 10 + (b - b'0') as u16;
            } else if b == b';' {
                if arg_count < 8 {
                    args[arg_count] = current_arg;
                    arg_count += 1;
                }
                current_arg = 0;
            }
        }
        if arg_count < 8 {
            args[arg_count] = current_arg;
            arg_count += 1;
        }
        
        match cmd {
            b'A' => { // Cursor Up (CUU)
                let n = if args[0] == 0 { 1 } else { args[0] as usize };
                self.cursor_y = self.cursor_y.saturating_sub(n).max(self.scroll_top);
            }
            b'B' => { // Cursor Down (CUD)
                let n = if args[0] == 0 { 1 } else { args[0] as usize };
                self.cursor_y = (self.cursor_y + n).min(self.scroll_bottom);
            }
            b'C' => { // Cursor Forward (CUF)
                let n = if args[0] == 0 { 1 } else { args[0] as usize };
                self.cursor_x = (self.cursor_x + n).min(TERMINAL_COLS - 1);
            }
            b'D' => { // Cursor Backward (CUB)
                let n = if args[0] == 0 { 1 } else { args[0] as usize };
                self.cursor_x = self.cursor_x.saturating_sub(n);
            }
            b'H' | b'f' => { // Cursor Position (CUP)
                let row = if args[0] == 0 { 1 } else { args[0] } as usize;
                let col = if arg_count > 1 && args[1] > 0 { args[1] } else { 1 } as usize;
                self.cursor_y = (row - 1).min(TERMINAL_ROWS - 1);
                self.cursor_x = (col - 1).min(TERMINAL_COLS - 1);
            }
            b'J' => { // Erase in Display (ED)
                let mode = args[0];
                match mode {
                    0 => self.erase_from_cursor_to_end(),
                    1 => self.erase_from_start_to_cursor(),
                    2 => self.clear_screen(),
                    _ => {}
                }
            }
            b'K' => { // Erase in Line (EL)
                let mode = args[0];
                match mode {
                    0 => self.erase_line_from_cursor(),
                    1 => self.erase_line_to_cursor(),
                    2 => self.erase_entire_line(),
                    _ => {}
                }
            }
            b'm' => { // Select Graphic Rendition (SGR)
                self.handle_sgr_sequence(&args[..arg_count]);
            }
            b'r' => { // Set Scrolling Region (DECSTBM)
                if arg_count >= 2 {
                    let top = (args[0] as usize).saturating_sub(1);
                    let bottom = (args[1] as usize).saturating_sub(1).min(TERMINAL_ROWS - 1);
                    if top < bottom {
                        self.scroll_top = top;
                        self.scroll_bottom = bottom;
                        self.cursor_x = 0;
                        self.cursor_y = self.scroll_top;
                    }
                }
            }
            b's' => { // Save cursor position
                self.saved_x = self.cursor_x;
                self.saved_y = self.cursor_y;
            }
            b'u' => { // Restore cursor position
                self.cursor_x = self.saved_x;
                self.cursor_y = self.saved_y;
            }
            _ => {} // Неизвестная команда
        }
    }
    
    fn handle_sgr_sequence(&mut self, args: &[u16]) {
        for &arg in args {
            match arg {
                0 => { // Reset
                    self.current_fg = 7; // LGRAY
                    self.current_bg = 0; // BLACK
                    self.bold = false;
                    self.underline = false;
                    self.reverse = false;
                }
                1 => self.bold = true,
                4 => self.underline = true,
                7 => self.reverse = true,
                22 => self.bold = false,
                24 => self.underline = false,
                27 => self.reverse = false,
                30..=37 => { // Foreground colors
                    self.current_fg = match arg {
                        30 => 0,  // BLACK
                        31 => 4,  // RED
                        32 => 2,  // GREEN
                        33 => 14, // YELLOW
                        34 => 1,  // BLUE
                        35 => 5,  // MAGENTA
                        36 => 3,  // CYAN
                        37 => 7,  // LGRAY
                        _ => 7,
                    };
                }
                40..=47 => { // Background colors
                    self.current_bg = match arg {
                        40 => 0,  // BLACK
                        41 => 4,  // RED
                        42 => 2,  // GREEN
                        43 => 14, // YELLOW
                        44 => 1,  // BLUE
                        45 => 5,  // MAGENTA
                        46 => 3,  // CYAN
                        47 => 7,  // LGRAY
                        _ => 0,
                    };
                }
                90..=97 => { // Bright foreground colors
                    self.current_fg = match arg {
                        90 => 8,  // DGRAY
                        91 => 12, // LRED
                        92 => 10, // LGREEN
                        93 => 14, // YELLOW
                        94 => 9,  // LBLUE
                        95 => 13, // LMAGENTA
                        96 => 11, // LCYAN
                        97 => 15, // WHITE
                        _ => 7,
                    };
                }
                _ => {} // Неизвестный атрибут
            }
        }
    }

    // ── Вспомогательные функции ───────────────────────────────────────────

    fn put_char_at_cursor(&mut self, ch: u8) {
        if self.cursor_y < TERMINAL_ROWS && self.cursor_x < TERMINAL_COLS {
            let mut fg = self.current_fg;
            let mut bg = self.current_bg;
            
            if self.bold && fg < 8 {
                fg += 8; // Яркие цвета для bold
            }
            
            if self.reverse {
                core::mem::swap(&mut fg, &mut bg);
            }
            
            self.screen[self.cursor_y][self.cursor_x] = TerminalCell { ch, fg, bg };
        }
    }
    
    fn set_color(&mut self, fg: u8, bg: u8) {
        self.current_fg = fg;
        self.current_bg = bg;
    }
    
    fn clear_screen(&mut self) {
        for row in 0..TERMINAL_ROWS {
            for col in 0..TERMINAL_COLS {
                self.screen[row][col] = TerminalCell::default();
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }
    
    fn scroll_up(&mut self) {
        // Сохраняем верхнюю строку в историю
        if self.history_count < HISTORY_LINES {
            self.history[self.history_count] = self.screen[self.scroll_top];
            self.history_count += 1;
        } else {
            // Сдвигаем историю
            for i in 0..HISTORY_LINES - 1 {
                self.history[i] = self.history[i + 1];
            }
            self.history[HISTORY_LINES - 1] = self.screen[self.scroll_top];
        }
        
        // Сдвигаем строки вверх
        for row in self.scroll_top..self.scroll_bottom {
            self.screen[row] = self.screen[row + 1];
        }
        
        // Очищаем нижнюю строку
        for col in 0..TERMINAL_COLS {
            self.screen[self.scroll_bottom][col] = TerminalCell::default();
        }
    }
    
    fn scroll_down(&mut self) {
        // Сдвигаем строки вниз
        for row in (self.scroll_top + 1..=self.scroll_bottom).rev() {
            self.screen[row] = self.screen[row - 1];
        }
        
        // Очищаем верхнюю строку
        for col in 0..TERMINAL_COLS {
            self.screen[self.scroll_top][col] = TerminalCell::default();
        }
    }
    
    fn erase_from_cursor_to_end(&mut self) {
        // Очищаем от курсора до конца экрана
        for col in self.cursor_x..TERMINAL_COLS {
            self.screen[self.cursor_y][col] = TerminalCell::default();
        }
        for row in (self.cursor_y + 1)..TERMINAL_ROWS {
            for col in 0..TERMINAL_COLS {
                self.screen[row][col] = TerminalCell::default();
            }
        }
    }
    
    fn erase_from_start_to_cursor(&mut self) {
        // Очищаем от начала экрана до курсора
        for row in 0..self.cursor_y {
            for col in 0..TERMINAL_COLS {
                self.screen[row][col] = TerminalCell::default();
            }
        }
        for col in 0..=self.cursor_x {
            self.screen[self.cursor_y][col] = TerminalCell::default();
        }
    }
    
    fn erase_line_from_cursor(&mut self) {
        for col in self.cursor_x..TERMINAL_COLS {
            self.screen[self.cursor_y][col] = TerminalCell::default();
        }
    }
    
    fn erase_line_to_cursor(&mut self) {
        for col in 0..=self.cursor_x {
            self.screen[self.cursor_y][col] = TerminalCell::default();
        }
    }
    
    fn erase_entire_line(&mut self) {
        for col in 0..TERMINAL_COLS {
            self.screen[self.cursor_y][col] = TerminalCell::default();
        }
    }
    
    fn reset_terminal(&mut self) {
        self.clear_screen();
        self.current_fg = 7; // LGRAY
        self.current_bg = 0; // BLACK
        self.bold = false;
        self.underline = false;
        self.reverse = false;
        self.scroll_top = 0;
        self.scroll_bottom = TERMINAL_ROWS - 1;
        self.cursor_visible = true;
    }

    // ── Отрисовка ─────────────────────────────────────────────────────────

    pub fn draw(&self, win: &Window) {
        let ix = win.inner_x();
        let iy = win.inner_y();
        let iw = win.inner_w();
        let ih = win.inner_h();

        // Отрисовываем терминальный буфер
        let visible_rows = ih.min(TERMINAL_ROWS);
        let visible_cols = iw.min(TERMINAL_COLS);
        
        for row in 0..visible_rows {
            for col in 0..visible_cols {
                let screen_row = row + self.scroll_offset;
                let cell = if screen_row < TERMINAL_ROWS {
                    self.screen[screen_row][col]
                } else {
                    TerminalCell::default()
                };
                
                let mut ch = cell.ch;
                let fg = cell.fg;
                let bg = cell.bg;
                
                // Подчеркивание (простая имитация)
                if self.underline && screen_row == self.cursor_y && col == self.cursor_x {
                    ch = b'_';
                }
                
                // Конвертируем VGA цвета в RGB для отображения
                let fg_rgb = vga_to_rgb(fg);
                let bg_rgb = vga_to_rgb(bg);
                
                put_char_at_bg(ix + col, iy + row, ch, fg_rgb, bg_rgb);
            }
        }
        
        // Отрисовываем курсор
        if self.cursor_visible && self.cursor_y >= self.scroll_offset && 
           self.cursor_y < self.scroll_offset + visible_rows &&
           self.cursor_x < visible_cols {
            let cursor_screen_y = self.cursor_y - self.scroll_offset;
            
            if self.cursor_blink {
                // Мигающий курсор
                put_char_at_bg(ix + self.cursor_x, iy + cursor_screen_y, b'_', BLACK, WHITE);
            } else {
                // Статичный курсор - используем ASCII блок
                put_char_at_bg(ix + self.cursor_x, iy + cursor_screen_y, b'#', WHITE, BLACK);
            }
        }
        
        // Показываем режим ввода в статусной строке (если есть место)
        if ih > TERMINAL_ROWS {
            let status_y = iy + TERMINAL_ROWS;
            fill_rect(ix, status_y, iw, 1, b' ', DGRAY);
            
            let status = if self.input_mode {
                "INPUT MODE - ESC to exit"
            } else {
                "TERMINAL MODE - Press i to enter input mode"
            };
            put_str_at_bg(ix, status_y, status, WHITE, DGRAY);
        }
    }

    // ── Обработка клавиш ──────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: Key) {
        if self.input_mode {
            self.handle_input_key(key);
        } else {
            self.handle_terminal_key(key);
        }
    }
    
    fn handle_input_key(&mut self, key: Key) {
        match key {
            Key::Enter => {
                // Копируем команду в отдельный буфер чтобы избежать проблем с заимствованием
                let mut cmd_buf = [0u8; 256];
                let cmd_len = self.input_len;
                cmd_buf[..cmd_len].copy_from_slice(&self.input_buffer[..cmd_len]);
                let cmd_str = core::str::from_utf8(&cmd_buf[..cmd_len])
                    .unwrap_or("");

                self.write_str("\r\n");
                
                // Сохраняем в историю команд
                if self.input_len > 0 && self.cmd_count < 32 {
                    self.cmd_history[self.cmd_count][..self.input_len]
                        .copy_from_slice(&self.input_buffer[..self.input_len]);
                    self.cmd_lens[self.cmd_count] = self.input_len;
                    self.cmd_count += 1;
                }
                self.cmd_pos = self.cmd_count;
                
                // Выполняем команду
                if !cmd_str.is_empty() {
                    self.execute_command(cmd_str);
                }
                
                self.show_prompt();
            }
            
            Key::Backspace => {
                if self.input_pos > 0 {
                    // Удаляем символ перед курсором
                    for i in self.input_pos - 1..self.input_len - 1 {
                        self.input_buffer[i] = self.input_buffer[i + 1];
                    }
                    self.input_len -= 1;
                    self.input_pos -= 1;
                    self.redraw_input_line();
                }
            }
            
            // Key::Delete не существует в enum Key, используем другой подход
            
            Key::Left => {
                if self.input_pos > 0 {
                    self.input_pos -= 1;
                }
            }
            
            Key::Right => {
                if self.input_pos < self.input_len {
                    self.input_pos += 1;
                }
            }
            
            Key::Up => {
                if self.cmd_pos > 0 {
                    self.cmd_pos -= 1;
                    self.load_history_command();
                }
            }
            
            Key::Down => {
                if self.cmd_pos < self.cmd_count {
                    self.cmd_pos += 1;
                    if self.cmd_pos == self.cmd_count {
                        self.input_len = 0;
                        self.input_pos = 0;
                    } else {
                        self.load_history_command();
                    }
                    self.redraw_input_line();
                }
            }
            
            Key::Home => {
                self.input_pos = 0;
            }
            
            Key::End => {
                self.input_pos = self.input_len;
            }
            
            Key::Esc => {
                self.input_mode = false;
            }
            
            Key::Char(c) if c >= 0x20 && c < 0x7F => {
                if self.input_len < 255 {
                    // Вставляем символ в позицию курсора
                    for i in (self.input_pos..self.input_len).rev() {
                        self.input_buffer[i + 1] = self.input_buffer[i];
                    }
                    self.input_buffer[self.input_pos] = c;
                    self.input_len += 1;
                    self.input_pos += 1;
                    self.redraw_input_line();
                }
            }
            
            _ => {}
        }
    }
    
    fn handle_terminal_key(&mut self, key: Key) {
        match key {
            Key::Up => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            
            Key::Down => {
                if self.scroll_offset < self.history_count {
                    self.scroll_offset += 1;
                }
            }
            
            Key::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            
            Key::PageDown => {
                self.scroll_offset = (self.scroll_offset + 10).min(self.history_count);
            }
            
            Key::Home => {
                self.scroll_offset = 0;
            }
            
            Key::End => {
                self.scroll_offset = self.history_count;
            }
            
            Key::Char(b'i') => {
                self.input_mode = true;
            }
            
            Key::Char(b'c') => {
                self.clear_screen();
                self.show_prompt();
            }
            
            Key::Char(b'r') => {
                self.reset_terminal();
                self.show_prompt();
            }
            
            _ => {}
        }
    }
    
    fn load_history_command(&mut self) {
        if self.cmd_pos < self.cmd_count {
            let len = self.cmd_lens[self.cmd_pos];
            self.input_buffer[..len].copy_from_slice(&self.cmd_history[self.cmd_pos][..len]);
            self.input_len = len;
            self.input_pos = len;
            self.redraw_input_line();
        }
    }
    
    fn redraw_input_line(&mut self) {
        // Очищаем текущую строку ввода
        for col in 0..TERMINAL_COLS {
            self.screen[self.cursor_y][col] = TerminalCell::default();
        }
        
        // Перерисовываем промпт и ввод
        self.cursor_x = 0;
        let cwd = crate::fs::cwd();
        let user = crate::auth::current_name();
        
        self.set_color(10, 0); // LGREEN
        self.write_str(user);
        self.set_color(7, 0);  // LGRAY
        self.write_str("@pindos:");
        self.set_color(11, 0); // LCYAN
        self.write_str(cwd);
        self.set_color(15, 0); // WHITE
        self.write_str("$ ");
        
        // Копируем ввод в отдельный буфер
        let mut input_buf = [0u8; 256];
        let input_len = self.input_len;
        input_buf[..input_len].copy_from_slice(&self.input_buffer[..input_len]);
        let input_str = core::str::from_utf8(&input_buf[..input_len])
            .unwrap_or("");
        self.write_str(input_str);
        
        // Устанавливаем курсор в правильную позицию
        self.cursor_x = user.len() + 10 + cwd.len() + self.input_pos;
    }

    fn execute_command(&mut self, cmd: &str) {
        let cmd = cmd.trim();
        
        // Встроенные команды терминала
        match cmd {
            "clear" | "cls" => {
                self.clear_screen();
                return;
            }
            "reset" => {
                self.reset_terminal();
                return;
            }
            "help" => {
                self.show_help();
                return;
            }
            "test-colors" => {
                self.test_colors();
                return;
            }
            "test-ansi" => {
                self.test_ansi();
                return;
            }
            _ => {}
        }
        
        // Выполняем команду через shell
        let output = crate::uglyshell::capture_command(cmd);
        if !output.is_empty() {
            self.write_str(output);
        }
    }
    
    fn show_help(&mut self) {
        self.write_str("Burmalda Terminal Emulator Commands:\r\n");
        self.write_str("  clear, cls     - Clear screen\r\n");
        self.write_str("  reset          - Reset terminal\r\n");
        self.write_str("  test-colors    - Test color support\r\n");
        self.write_str("  test-ansi      - Test ANSI sequences\r\n");
        self.write_str("  help           - Show this help\r\n");
        self.write_str("\r\nTerminal Mode Keys:\r\n");
        self.write_str("  i              - Enter input mode\r\n");
        self.write_str("  c              - Clear screen\r\n");
        self.write_str("  r              - Reset terminal\r\n");
        self.write_str("  Up/Down        - Scroll history\r\n");
        self.write_str("  PgUp/PgDn      - Page scroll\r\n");
        self.write_str("\r\nInput Mode Keys:\r\n");
        self.write_str("  ESC            - Exit to terminal mode\r\n");
        self.write_str("  Up/Down        - Command history\r\n");
        self.write_str("  Left/Right     - Move cursor\r\n");
        self.write_str("  Home/End       - Start/End of line\r\n");
        self.write_str("\r\n");
    }
    
    fn test_colors(&mut self) {
        self.write_str("Color Test:\r\n");
        
        // Обычные цвета
        self.write_str("Normal colors: ");
        for i in 30..38 {
            self.write_str("\x1B[");
            self.write_number(i);
            self.write_str("m###\x1B[0m ");
        }
        self.write_str("\r\n");
        
        // Яркие цвета
        self.write_str("Bright colors: ");
        for i in 90..98 {
            self.write_str("\x1B[");
            self.write_number(i);
            self.write_str("m###\x1B[0m ");
        }
        self.write_str("\r\n");
        
        // Фоновые цвета
        self.write_str("Background:    ");
        for i in 40..48 {
            self.write_str("\x1B[");
            self.write_number(i);
            self.write_str("m   \x1B[0m ");
        }
        self.write_str("\r\n\r\n");
    }
    
    fn write_number(&mut self, n: u32) {
        let mut buf = [0u8; 10];
        let mut len = 0;
        let mut num = n;
        
        if num == 0 {
            buf[0] = b'0';
            len = 1;
        } else {
            while num > 0 {
                buf[len] = b'0' + (num % 10) as u8;
                num /= 10;
                len += 1;
            }
            // Переворачиваем цифры
            for i in 0..len/2 {
                buf.swap(i, len - 1 - i);
            }
        }
        
        let num_str = core::str::from_utf8(&buf[..len]).unwrap_or("0");
        self.write_str(num_str);
    }
    
    fn test_ansi(&mut self) {
        self.write_str("ANSI Escape Sequence Test:\r\n");
        self.write_str("\x1B[1mBold text\x1B[0m\r\n");
        self.write_str("\x1B[4mUnderlined text\x1B[0m\r\n");
        self.write_str("\x1B[7mReverse text\x1B[0m\r\n");
        self.write_str("\x1B[31mRed \x1B[32mGreen \x1B[34mBlue\x1B[0m\r\n");
        self.write_str("\x1B[1;31mBold Red\x1B[0m\r\n");
        self.write_str("Cursor movement: ");
        self.write_str("\x1B[s");  // Save cursor
        self.write_str("SAVED");
        self.write_str("\x1B[u");  // Restore cursor
        self.write_str("RESTORED\r\n");
        self.write_str("\r\n");
    }

    // ── Обработка мыши ────────────────────────────────────────────────────

    pub fn handle_mouse(&mut self, x: usize, y: usize) {
        // Клик переключает в режим ввода
        if !self.input_mode {
            self.input_mode = true;
        }
        
        // Позиционируем курсор ввода (если в режиме ввода)
        if self.input_mode && y == self.cursor_y {
            let prompt_len = crate::auth::current_name().len() + 10 + crate::fs::cwd().len();
            if x >= prompt_len {
                let new_pos = (x - prompt_len).min(self.input_len);
                self.input_pos = new_pos;
            }
        }
    }
}

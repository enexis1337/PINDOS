// Mell GUI — Настройки

use crate::mell::vga_gui::*;
use crate::auth;

#[derive(Copy, Clone, PartialEq)]
pub enum SettingsTab { System, Users, Display, About }

pub struct SettingsApp {
    pub tab:       SettingsTab,
    pub selected:  usize,
    pub input_buf: [u8; 64],
    pub input_len: usize,
    pub input_mode: SettingsInput,
    pub status:    [u8; 80],
    pub status_len: usize,
}

#[derive(Copy, Clone, PartialEq)]
pub enum SettingsInput { None, NewUser, NewPass, Hostname }

impl SettingsApp {
    pub fn new() -> Self {
        SettingsApp {
            tab: SettingsTab::System,
            selected: 0,
            input_buf: [0u8; 64], input_len: 0,
            input_mode: SettingsInput::None,
            status: [0u8; 80], status_len: 0,
        }
    }

    fn set_status(&mut self, msg: &str) {
        let b = msg.as_bytes();
        let l = b.len().min(80);
        self.status[..l].copy_from_slice(&b[..l]);
        self.status_len = l;
    }

    // ── Отрисовка ─────────────────────────────────────────────────────────

    pub fn draw(&self, win: &Window) {
        let ix = win.inner_x();
        let iy = win.inner_y();
        let iw = win.inner_w();
        let ih = win.inner_h();

        fill_rect(ix, iy, iw, ih, b' ', MELL_WINDOW);

        // Вкладки
        let tabs = ["System", "Users", "Display", "About"];
        let mut tx = ix;
        for (i, &tab) in tabs.iter().enumerate() {
            let active = i == self.tab as usize;
            draw_button(tx, iy, tab, active);
            tx += tab.len() + 3;
        }
        draw_hline(ix, iy + 1, iw, color(DGRAY, LGRAY));

        // Содержимое вкладки
        match self.tab {
            SettingsTab::System  => self.draw_system(ix, iy + 2, iw, ih - 4),
            SettingsTab::Users   => self.draw_users(ix, iy + 2, iw, ih - 4),
            SettingsTab::Display => self.draw_display(ix, iy + 2, iw, ih - 4),
            SettingsTab::About   => self.draw_about(ix, iy + 2, iw, ih - 4),
        }

        // Статус
        let st_y = iy + ih - 1;
        fill_rect(ix, st_y, iw, 1, b' ', color(WHITE, DGRAY));
        let status = core::str::from_utf8(&self.status[..self.status_len]).unwrap_or("");
        put_str_at(ix, st_y, status, color(WHITE, DGRAY));

        // Диалог ввода
        if self.input_mode != SettingsInput::None {
            let prompt = match self.input_mode {
                SettingsInput::NewUser => "New username: ",
                SettingsInput::NewPass => "New password: ",
                SettingsInput::Hostname => "Hostname: ",
                SettingsInput::None => "",
            };
            let dlg_y = iy + ih / 2;
            fill_rect(ix + 2, dlg_y, iw - 4, 1, b' ', WHITE);
            put_str_at_bg(ix + 2, dlg_y, prompt, BLUE, WHITE);
            let input = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
            let mask = self.input_mode == SettingsInput::NewPass;
            if mask {
                for i in 0..self.input_len {
                    put_char_at_bg(ix + 2 + prompt.len() + i, dlg_y, b'*', BLACK, WHITE);
                }
            } else {
                put_str_at_bg(ix + 2 + prompt.len(), dlg_y, input, BLACK, WHITE);
            }
            put_char_at_bg(ix + 2 + prompt.len() + self.input_len, dlg_y, b'_', BLACK, WHITE);
        }
    }

    fn draw_system(&self, x: usize, y: usize, _w: usize, _h: usize) {
        put_str_at(x, y,     "OS:      PINDOS 0.1",          color(BLACK, LGRAY));
        put_str_at(x, y + 1, "Kernel:  pindos-kernel 0.1.0", color(BLACK, LGRAY));
        put_str_at(x, y + 2, "Arch:    i686 (32-bit)",        color(BLACK, LGRAY));
        put_str_at(x, y + 3, "Memory:  32MB",                 color(BLACK, LGRAY));
        put_str_at(x, y + 4, "GUI:     Mell 0.1",             color(BLACK, LGRAY));
        put_str_at(x, y + 6, "Press H to change hostname",    color(DGRAY, LGRAY));
    }

    fn draw_users(&self, x: usize, y: usize, _w: usize, _h: usize) {
        put_str_at(x, y, "Users:", color(BLACK, LGRAY));
        draw_hline(x, y + 1, 30, color(DGRAY, LGRAY));
        auth::list_users_at(x, y + 2);
        put_str_at(x, y + 10, "A=Add user  D=Delete  P=Change password", color(DGRAY, LGRAY));
    }

    fn draw_display(&self, x: usize, y: usize, _w: usize, _h: usize) {
        put_str_at(x, y,     "Display mode: VESA 1024x768",  MELL_TEXT);
        put_str_at(x, y + 1, "Colors:       32bpp",          MELL_TEXT);
        put_str_at(x, y + 2, "Desktop:      Mell Green",     MELL_TEXT);
        put_str_at(x, y + 4, "Color palette:", MELL_TEXT);
        // Цветовые плашки
        let palette: &[u32] = &[BLACK, RED, GREEN, BROWN, BLUE, MAGENTA, CYAN, LGRAY,
                                 DGRAY, LRED, LGREEN, YELLOW, LBLUE, LMAGENTA, LCYAN, WHITE];
        for (i, &c) in palette.iter().enumerate() {
            fill_rect(x + i * 3, y + 5, 2, 1, b' ', c);
        }
    }

    fn draw_about(&self, x: usize, y: usize, _w: usize, _h: usize) {
        put_str_at(x, y,      "PINDOS v0.1",                  color(LGREEN, LGRAY));
        put_str_at(x, y + 1,  "Mell Desktop Environment",     color(BLACK, LGRAY));
        put_str_at(x, y + 2,  "",                              color(BLACK, LGRAY));
        put_str_at(x, y + 3,  "Built with Rust + NASM",       color(BLACK, LGRAY));
        put_str_at(x, y + 4,  "Architecture: i686 bare metal",color(BLACK, LGRAY));
        put_str_at(x, y + 5,  "",                              color(BLACK, LGRAY));
        put_str_at(x, y + 6,  "Apps:",                        color(BLACK, LGRAY));
        put_str_at(x, y + 7,  "  Mocha    - File Manager",    color(BLACK, LGRAY));
        put_str_at(x, y + 8,  "  Qinn     - Text Editor",     color(BLACK, LGRAY));
        put_str_at(x, y + 9,  "  Burmalda - Terminal",        color(BLACK, LGRAY));
        put_str_at(x, y + 10, "  Settings - System Settings", color(BLACK, LGRAY));
    }

    // ── Обработка клавиш ──────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: Key) {
        if self.input_mode != SettingsInput::None {
            self.handle_input(key);
            return;
        }
        match key {
            Key::Tab => {
                self.tab = match self.tab {
                    SettingsTab::System  => SettingsTab::Users,
                    SettingsTab::Users   => SettingsTab::Display,
                    SettingsTab::Display => SettingsTab::About,
                    SettingsTab::About   => SettingsTab::System,
                };
            }
            Key::Char(b'1') => self.tab = SettingsTab::System,
            Key::Char(b'2') => self.tab = SettingsTab::Users,
            Key::Char(b'3') => self.tab = SettingsTab::Display,
            Key::Char(b'4') => self.tab = SettingsTab::About,
            // Users tab actions
            Key::Char(b'a') | Key::Char(b'A') if self.tab == SettingsTab::Users => {
                self.input_mode = SettingsInput::NewUser;
                self.input_len = 0;
            }
            Key::Char(b'p') | Key::Char(b'P') if self.tab == SettingsTab::Users => {
                self.input_mode = SettingsInput::NewPass;
                self.input_len = 0;
            }
            Key::Char(b'h') | Key::Char(b'H') if self.tab == SettingsTab::System => {
                self.input_mode = SettingsInput::Hostname;
                self.input_len = 0;
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, key: Key) {
        match key {
            Key::Esc => { self.input_mode = SettingsInput::None; self.input_len = 0; }
            Key::Backspace => { if self.input_len > 0 { self.input_len -= 1; } }
            Key::Enter => {
                let input = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
                match self.input_mode {
                    SettingsInput::NewUser => {
                        if auth::add_user(input, "", false) {
                            self.set_status("User created (no password)");
                        } else {
                            self.set_status("Failed: user exists");
                        }
                    }
                    SettingsInput::NewPass => {
                        let user = auth::current_name();
                        if auth::change_password(user, input) {
                            self.set_status("Password changed");
                        } else {
                            self.set_status("Failed");
                        }
                    }
                    SettingsInput::Hostname => {
                        self.set_status("Hostname updated (reboot to apply)");
                    }
                    SettingsInput::None => {}
                }
                self.input_mode = SettingsInput::None;
                self.input_len = 0;
            }
            Key::Char(c) => {
                if self.input_len < 63 { self.input_buf[self.input_len] = c; self.input_len += 1; }
            }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, x: usize, y: usize) {
        if y == 0 {
            let tabs = ["System", "Users", "Display", "About"];
            let mut tx = 0;
            for (i, &tab) in tabs.iter().enumerate() {
                if x >= tx && x < tx + tab.len() + 3 {
                    self.tab = match i {
                        0 => SettingsTab::System,
                        1 => SettingsTab::Users,
                        2 => SettingsTab::Display,
                        3 => SettingsTab::About,
                        _ => self.tab,
                    };
                    break;
                }
                tx += tab.len() + 3;
            }
        }
    }
}
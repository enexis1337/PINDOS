// Mell GUI — Image Viewer + Audio Player

use crate::mell::vga_gui::*;
use crate::fs;
use crate::drivers::{png, speaker};

#[derive(Copy, Clone, PartialEq)]
pub enum ViewerMode { Image, Audio, Empty }

pub struct ViewerApp {
    pub mode:      ViewerMode,
    pub filename:  [u8; 64],
    pub fname_len: usize,
    pub status:    [u8; 80],
    pub status_len: usize,
    pub playing:   bool,
    pub input_buf: [u8; 64],
    pub input_len: usize,
    pub show_open: bool,
}

impl ViewerApp {
    pub fn new() -> Self {
        let mut app = ViewerApp {
            mode: ViewerMode::Empty,
            filename: [0u8; 64], fname_len: 0,
            status: [0u8; 80], status_len: 0,
            playing: false,
            input_buf: [0u8; 64], input_len: 0,
            show_open: false,
        };
        app.set_status("O=Open file  Q=Close");
        app
    }

    fn set_status(&mut self, s: &str) {
        let b = s.as_bytes();
        let l = b.len().min(80);
        self.status[..l].copy_from_slice(&b[..l]);
        self.status_len = l;
    }

    fn fname_str(&self) -> &str {
        core::str::from_utf8(&self.filename[..self.fname_len]).unwrap_or("")
    }

    pub fn open(&mut self, name: &str) {
        let nb = name.as_bytes();
        let nl = nb.len().min(64);
        self.filename[..nl].copy_from_slice(&nb[..nl]);
        self.fname_len = nl;

        if name.ends_with(".png") || name.ends_with(".PNG") {
            self.mode = ViewerMode::Image;
            self.set_status("PNG image — press Q to close");
        } else if name.ends_with(".wav") || name.ends_with(".WAV") {
            self.mode = ViewerMode::Audio;
            self.set_status("WAV audio — P=Play  S=Stop  Q=Close");
        } else {
            self.mode = ViewerMode::Empty;
            self.set_status("Unknown format (supported: .png .wav)");
        }
    }

    pub fn draw(&self, win: &Window) {
        let ix = win.inner_x();
        let iy = win.inner_y();
        let iw = win.inner_w();
        let ih = win.inner_h();

        fill_rect(ix, iy, iw, ih, b' ', MELL_WINDOW);

        // Заголовок с именем файла
        fill_rect(ix, iy, iw, 1, b' ', color(WHITE, DGRAY));
        put_str_at(ix, iy, self.fname_str(), color(CYAN, DGRAY));

        match self.mode {
            ViewerMode::Empty => {
                put_str_at(ix + 2, iy + ih/2 - 1, "No file open", color(DGRAY, LGRAY));
                put_str_at(ix + 2, iy + ih/2,     "Press O to open a file", color(DGRAY, LGRAY));
                put_str_at(ix + 2, iy + ih/2 + 1, "Supported: .png .wav", color(DGRAY, LGRAY));
            }
            ViewerMode::Image => {
                let content_win = Window::new(ix, iy + 1, iw, ih - 2, "");
                if let Some(f) = fs::get(self.fname_str()) {
                    let data = f.content_str().as_bytes();
                    let _ = png::render_png_ascii(data, &content_win);
                } else {
                    put_str_at(ix + 2, iy + ih/2, "File not found", color(LRED, LGRAY));
                }
            }
            ViewerMode::Audio => {
                self.draw_audio_player(ix, iy + 1, iw, ih - 2);
            }
        }

        // Статусная строка
        let st_y = iy + ih - 1;
        fill_rect(ix, st_y, iw, 1, b' ', color(WHITE, DGRAY));
        let status = core::str::from_utf8(&self.status[..self.status_len]).unwrap_or("");
        put_str_at(ix, st_y, status, color(WHITE, DGRAY));

        // Диалог открытия файла
        if self.show_open {
            let dlg_y = iy + ih / 2;
            fill_rect(ix + 2, dlg_y, iw - 4, 1, b' ', WHITE);
            put_str_at_bg(ix + 2, dlg_y, "Open: ", BLUE, WHITE);
            let input = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
            put_str_at_bg(ix + 8, dlg_y, input, BLACK, WHITE);
            put_char_at_bg(ix + 8 + self.input_len, dlg_y, b'_', BLACK, WHITE);
        }
    }

    fn draw_audio_player(&self, x: usize, y: usize, w: usize, h: usize) {
        fill_rect(x, y, w, h, b' ', color(BLACK, BLACK));

        // Визуализация — простой "эквалайзер" из символов
        let bars = w.min(40);
        let bar_y = y + h / 2;

        if self.playing {
            // Анимированные бары (псевдо-случайные на основе тика)
            for i in 0..bars {
                let height = ((i * 7 + 3) % 8) + 1;
                for j in 0..height {
                    let c = if j < height / 2 { color(LGREEN, BLACK) }
                            else { color(GREEN, BLACK) };
                    put_char_at(x + i, bar_y - j, b'|', c);
                }
            }
            put_str_at(x + 2, y + 1, ">> PLAYING", color(LGREEN, BLACK));
        } else {
            for i in 0..bars {
                put_char_at(x + i, bar_y, b'_', color(DGRAY, BLACK));
            }
            put_str_at(x + 2, y + 1, "|| STOPPED", color(DGRAY, BLACK));
        }

        // Имя файла
        put_str_at(x + 2, y + 3, self.fname_str(), color(CYAN, BLACK));

        // Кнопки
        draw_button(x + 2, y + h - 2, " PLAY ", self.playing);
        draw_button(x + 12, y + h - 2, " STOP ", !self.playing);
    }

    pub fn handle_key(&mut self, key: Key) {
        if self.show_open {
            match key {
                Key::Esc => { self.show_open = false; self.input_len = 0; }
                Key::Backspace => { if self.input_len > 0 { self.input_len -= 1; } }
                Key::Enter => {
                    let mut name_buf = [0u8; 64];
                    let name_len = self.input_len;
                    name_buf[..name_len].copy_from_slice(&self.input_buf[..name_len]);
                    let name = core::str::from_utf8(&name_buf[..name_len])
                        .unwrap_or("").trim();
                    let mut name_copy = [0u8; 64];
                    let nc_len = name.len().min(64);
                    name_copy[..nc_len].copy_from_slice(&name.as_bytes()[..nc_len]);
                    let name_str = core::str::from_utf8(&name_copy[..nc_len]).unwrap_or("");
                    self.open(name_str);
                    self.show_open = false;
                    self.input_len = 0;
                }
                Key::Char(c) => {
                    if self.input_len < 63 { self.input_buf[self.input_len] = c; self.input_len += 1; }
                }
                _ => {}
            }
            return;
        }

        match key {
            Key::Char(b'o') | Key::Char(b'O') => {
                self.show_open = true;
                self.input_len = 0;
            }
            Key::Char(b'p') | Key::Char(b'P') => {
                if self.mode == ViewerMode::Audio {
                    self.playing = true;
                    self.set_status("Playing... S=Stop");
                    // Воспроизводим в фоне (упрощённо — блокирующий вызов)
                    if let Some(f) = fs::get(self.fname_str()) {
                        let data = f.content_str().as_bytes();
                        let _ = speaker::play_wav(data);
                    }
                    self.playing = false;
                    self.set_status("Done. P=Play again  O=Open  Q=Close");
                }
            }
            Key::Char(b's') | Key::Char(b'S') => {
                speaker::stop();
                self.playing = false;
                self.set_status("Stopped. P=Play  O=Open  Q=Close");
            }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, x: usize, y: usize) {
        if self.mode == ViewerMode::Audio {
            let button_y = 10;
            if y == button_y {
                if x >= 2 && x < 9 {
                    self.playing = true;
                    self.set_status("Playing... S=Stop");
                    if let Some(f) = fs::get(self.fname_str()) {
                        let data = f.content_str().as_bytes();
                        let _ = speaker::play_wav(data);
                    }
                    self.playing = false;
                    self.set_status("Done. P=Play again  O=Open  Q=Close");
                } else if x >= 12 && x < 19 {
                    speaker::stop();
                    self.playing = false;
                    self.set_status("Stopped. P=Play  O=Open  Q=Close");
                }
            }
        }
    }
}
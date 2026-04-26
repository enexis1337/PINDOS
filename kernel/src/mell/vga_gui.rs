// Mell GUI — примитивы поверх VESA framebuffer
// Координаты везде в "символьных единицах" (1 ед = FONT_W x FONT_H пикселей)
// Это позволяет не менять код приложений

use crate::drivers::vesa;

pub use vesa::{FONT_W, FONT_H};

// Размер экрана в символьных единицах
pub const SCR_W: usize = 128;  // 1024 / 8
pub const SCR_H: usize = 48;   // 768 / 16

// Псевдонимы для совместимости с кодом приложений
pub const VGA_WIDTH:  usize = SCR_W;
pub const VGA_HEIGHT: usize = SCR_H;

// ── Цвета (0x00RRGGBB) ────────────────────────────────────────────────────

pub const BLACK:    u32 = 0x000000;
pub const BLUE:     u32 = 0x0055AA;
pub const GREEN:    u32 = 0x008800;
pub const CYAN:     u32 = 0x008888;
pub const RED:      u32 = 0xAA0000;
pub const MAGENTA:  u32 = 0xAA00AA;
pub const BROWN:    u32 = 0xAA5500;
pub const LGRAY:    u32 = 0xC0C0C0;
pub const DGRAY:    u32 = 0x404040;
pub const LBLUE:    u32 = 0x5555FF;
pub const LGREEN:   u32 = 0x55FF55;
pub const LCYAN:    u32 = 0x55FFFF;
pub const LRED:     u32 = 0xFF5555;
pub const LMAGENTA: u32 = 0xFF55FF;
pub const YELLOW:   u32 = 0xFFFF55;
pub const WHITE:    u32 = 0xFFFFFF;

// Mell цвета
pub const MELL_DESKTOP:  u32 = 0x2D6A4F;  // тёмно-зелёный рабочий стол
pub const MELL_TITLEBAR: u32 = 0x3A3A3A;  // заголовок окна
pub const MELL_TITLEBAR_ACTIVE: u32 = 0x1E5799; // активный заголовок
pub const MELL_WINDOW:   u32 = 0xF0F0F0;  // фон окна
pub const MELL_TASKBAR:  u32 = 0x2B2B2B;  // нижняя панель
pub const MELL_TOPBAR:   u32 = 0x1A1A1A;  // верхняя панель
pub const MELL_BORDER:   u32 = 0x888888;  // рамка окна
pub const MELL_TEXT:     u32 = 0x000000;
pub const MELL_TEXT_LIGHT: u32 = 0xFFFFFF;

// Цвета кнопок окна (macOS-стиль)
pub const BTN_CLOSE:  u32 = 0xFF5F57;
pub const BTN_MIN:    u32 = 0xFFBD2E;
pub const BTN_MAX:    u32 = 0x28C840;

// ── Конвертация символьных координат в пиксельные ─────────────────────────

#[inline] pub fn px(cx: usize) -> u32 { (cx * FONT_W as usize) as u32 }
#[inline] pub fn py(cy: usize) -> u32 { (cy * FONT_H as usize) as u32 }
#[inline] pub fn pw(cw: usize) -> u32 { (cw * FONT_W as usize) as u32 }
#[inline] pub fn ph(ch: usize) -> u32 { (ch * FONT_H as usize) as u32 }

// ── Базовые операции (символьные координаты) ──────────────────────────────

pub fn put_char_at(cx: usize, cy: usize, c: u8, fg: u32) {
    vesa::draw_char(px(cx), py(cy), c, fg, 0xFF000000); // прозрачный фон = не рисуем bg
}

pub fn put_char_at_bg(cx: usize, cy: usize, c: u8, fg: u32, bg: u32) {
    vesa::draw_char(px(cx), py(cy), c, fg, bg);
}

pub fn put_str_at(cx: usize, cy: usize, s: &str, attr: u32) {
    let mut x = cx;
    for b in s.bytes() {
        vesa::draw_char(px(x), py(cy), b, attr, 0xFF000000);
        x += 1;
    }
}

pub fn put_str_at_bg(cx: usize, cy: usize, s: &str, fg: u32, bg: u32) {
    let mut x = cx;
    for b in s.bytes() {
        vesa::draw_char(px(x), py(cy), b, fg, bg);
        x += 1;
    }
}

pub fn fill_rect(cx: usize, cy: usize, cw: usize, ch: usize, _c: u8, color: u32) {
    vesa::fill_rect_fast(px(cx), py(cy), pw(cw), ph(ch), color);
}

pub fn fill_rect_px(x: u32, y: u32, w: u32, h: u32, color: u32) {
    vesa::fill_rect_fast(x, y, w, h, color);
}

pub fn draw_hline(cx: usize, cy: usize, cw: usize, color: u32) {
    vesa::fill_rect_fast(px(cx), py(cy), pw(cw), 1, color);
}

pub fn draw_vline(cx: usize, cy: usize, ch: usize, color: u32) {
    vesa::draw_vline(px(cx), py(cy), ph(ch), color);
}

// ── Окно ──────────────────────────────────────────────────────────────────

pub const TITLEBAR_H: usize = 2; // высота заголовка в символах

pub struct Window {
    pub x: usize, pub y: usize,  // символьные координаты
    pub w: usize, pub h: usize,
    pub title: &'static str,
    pub focused: bool,
}

impl Window {
    pub fn new(x: usize, y: usize, w: usize, h: usize, title: &'static str) -> Self {
        Window { x, y, w, h, title, focused: true }
    }

    pub fn draw(&self) {
        let px0 = px(self.x);
        let py0 = py(self.y);
        let pw0 = pw(self.w);
        let ph0 = ph(self.h);

        // Тень
        vesa::fill_rect_fast(px0 + 4, py0 + 4, pw0, ph0, 0x00000066 & 0x1A1A1A);

        // Тело окна
        vesa::fill_rect_fast(px0, py0 + ph(TITLEBAR_H), pw0, ph0 - ph(TITLEBAR_H), MELL_WINDOW);

        // Заголовок
        let tb_color = if self.focused { MELL_TITLEBAR_ACTIVE } else { MELL_TITLEBAR };
        vesa::fill_rect_fast(px0, py0, pw0, ph(TITLEBAR_H), tb_color);

        // Заголовок — градиент (нижняя строка чуть темнее)
        let tb_dark = darken(tb_color, 30);
        vesa::fill_rect_fast(px0, py0 + ph(TITLEBAR_H) - 1, pw0, 1, tb_dark);

        // Текст заголовка
        let title_y = self.y + (TITLEBAR_H - 1) / 2;
        put_str_at_bg(self.x + 4, title_y, self.title, WHITE, tb_color);

        // Кнопки (macOS-стиль): красная=закрыть, жёлтая=развернуть, зелёная=свернуть
        // Расположены справа в заголовке, в пикселях
        let btn_y = py0 + ph(TITLEBAR_H) / 2 - 5;
        let btn_x_base = px0 + pw0 - 14;  // правый край - отступ
        draw_circle_btn(btn_x_base,       btn_y, BTN_CLOSE);   // красная — закрыть
        draw_circle_btn(btn_x_base - 20,  btn_y, BTN_MIN);     // жёлтая — развернуть
        draw_circle_btn(btn_x_base - 40,  btn_y, BTN_MAX);     // зелёная — свернуть

        // Resize handle — правый нижний угол
        let rx = px0 + pw0 - 12;
        let ry = py0 + ph0 - 12;
        vesa::fill_rect_fast(rx,     ry + 8,  10, 2, MELL_BORDER);
        vesa::fill_rect_fast(rx + 4, ry + 4,  2,  6, MELL_BORDER);
        vesa::fill_rect_fast(rx + 8, ry,      2,  10, MELL_BORDER);

        // Рамка окна
        vesa::draw_rect_outline(px0, py0, pw0, ph0, MELL_BORDER);
    }

    pub fn inner_x(&self) -> usize { self.x + 1 }
    pub fn inner_y(&self) -> usize { self.y + TITLEBAR_H }
    pub fn inner_w(&self) -> usize { self.w.saturating_sub(2) }
    pub fn inner_h(&self) -> usize { self.h.saturating_sub(TITLEBAR_H + 1) }

    // Хит-тесты кнопок в пиксельных координатах
    // Кнопки: красная(закрыть) — правая, жёлтая(развернуть) — средняя, зелёная(свернуть) — левая
    fn btn_base_px(&self) -> u32 {
        px(self.x) + pw(self.w) - 14
    }
    fn btn_y_px(&self) -> u32 {
        py(self.y) + ph(TITLEBAR_H) / 2 - 5
    }

    /// Клик по красной кнопке (закрыть)
    pub fn close_btn_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        let bx = self.btn_base_px();
        let by = self.btn_y_px();
        (mpx as u32) >= bx && (mpx as u32) < bx + 12 &&
        (mpy as u32) >= by && (mpy as u32) < by + 12
    }

    /// Клик по жёлтой кнопке (развернуть/восстановить)
    pub fn max_btn_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        let bx = self.btn_base_px() - 20;
        let by = self.btn_y_px();
        (mpx as u32) >= bx && (mpx as u32) < bx + 12 &&
        (mpy as u32) >= by && (mpy as u32) < by + 12
    }

    /// Клик по зелёной кнопке (свернуть)
    pub fn min_btn_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        let bx = self.btn_base_px() - 40;
        let by = self.btn_y_px();
        (mpx as u32) >= bx && (mpx as u32) < bx + 12 &&
        (mpy as u32) >= by && (mpy as u32) < by + 12
    }

    /// Клик по заголовку (для drag), исключая кнопки
    pub fn title_bar_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        let px0 = px(self.x) as usize;
        let py0 = py(self.y) as usize;
        let pw0 = pw(self.w) as usize;
        let ph_tb = ph(TITLEBAR_H) as usize;
        // В пределах заголовка, но левее кнопок
        mpx >= px0 && mpx < px0 + pw0 - 50 &&
        mpy >= py0 && mpy < py0 + ph_tb
    }

    /// Клик по resize handle (правый нижний угол, 16x16 пикселей)
    pub fn resize_handle_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        let px0 = px(self.x) as usize;
        let py0 = py(self.y) as usize;
        let pw0 = pw(self.w) as usize;
        let ph0 = ph(self.h) as usize;
        mpx >= px0 + pw0 - 16 && mpx < px0 + pw0 &&
        mpy >= py0 + ph0 - 16 && mpy < py0 + ph0
    }

    /// Окно содержит пиксельную точку
    pub fn contains_px(&self, mpx: usize, mpy: usize) -> bool {
        let px0 = px(self.x) as usize;
        let py0 = py(self.y) as usize;
        let pw0 = pw(self.w) as usize;
        let ph0 = ph(self.h) as usize;
        mpx >= px0 && mpx < px0 + pw0 &&
        mpy >= py0 && mpy < py0 + ph0
    }

    pub fn close_clicked(&self, mx: usize, my: usize) -> bool {
        let btn_cx = self.x + self.w - 8;
        let btn_cy = self.y;
        mx >= btn_cx && mx < btn_cx + 3 && my >= btn_cy && my < btn_cy + TITLEBAR_H
    }

    pub fn title_clicked(&self, mx: usize, my: usize) -> bool {
        my >= self.y && my < self.y + TITLEBAR_H &&
        mx >= self.x && mx < self.x + self.w - 9
    }
}

fn draw_circle_btn(px: u32, py: u32, color: u32) {
    // Круглая кнопка 10x10
    vesa::fill_rect_fast(px + 1, py,     8, 10, color);
    vesa::fill_rect_fast(px,     py + 1, 10, 8,  color);
    // Блик
    vesa::fill_rect_fast(px + 2, py + 1, 4, 2, lighten(color, 60));
}

pub fn darken(c: u32, amt: u32) -> u32 {
    let r = ((c >> 16) & 0xFF).saturating_sub(amt);
    let g = ((c >> 8)  & 0xFF).saturating_sub(amt);
    let b = (c & 0xFF).saturating_sub(amt);
    (r << 16) | (g << 8) | b
}

pub fn lighten(c: u32, amt: u32) -> u32 {
    let r = (((c >> 16) & 0xFF) + amt).min(255);
    let g = (((c >> 8)  & 0xFF) + amt).min(255);
    let b = ((c & 0xFF) + amt).min(255);
    (r << 16) | (g << 8) | b
}

// ── Кнопка ────────────────────────────────────────────────────────────────

pub fn draw_button(cx: usize, cy: usize, label: &str, active: bool) {
    let bg = if active { 0x1E5799u32 } else { 0x555555u32 };
    let fg = WHITE;
    let cw = label.len() + 2;

    // Фон кнопки
    vesa::fill_rect_fast(px(cx), py(cy), pw(cw), ph(1), bg);
    // Блик сверху
    vesa::fill_rect_fast(px(cx), py(cy), pw(cw), 2, lighten(bg, 40));
    // Текст
    put_str_at_bg(cx + 1, cy, label, fg, bg);
}

// ── Иконки ────────────────────────────────────────────────────────────────

pub fn draw_folder_icon(cx: usize, cy: usize, name: &str, selected: bool) {
    let icon_bg  = if selected { 0x5599FFu32 } else { 0xF0C040u32 };
    let label_fg = if selected { WHITE } else { MELL_TEXT_LIGHT };
    let label_bg = if selected { 0x3377DDu32 } else { MELL_DESKTOP };

    let x = px(cx);
    let y = py(cy);

    // Тело папки (32x24)
    vesa::fill_rect_fast(x, y + 4, 32, 20, icon_bg);
    // Язычок папки
    vesa::fill_rect_fast(x, y, 14, 6, lighten(icon_bg, 30));
    // Блик
    vesa::fill_rect_fast(x + 2, y + 6, 28, 3, lighten(icon_bg, 50));

    // Имя под иконкой
    let name_cx = cx.saturating_sub(name.len() / 2);
    put_str_at_bg(name_cx, cy + 3, name, label_fg, label_bg);
}

pub fn draw_file_icon(cx: usize, cy: usize, name: &str, selected: bool) {
    let icon_bg  = if selected { 0x5599FFu32 } else { 0xFFFFFFu32 };
    let label_fg = if selected { WHITE } else { MELL_TEXT_LIGHT };
    let label_bg = if selected { 0x3377DDu32 } else { MELL_DESKTOP };

    let x = px(cx);
    let y = py(cy);

    // Тело файла (28x32)
    vesa::fill_rect_fast(x, y, 28, 32, icon_bg);
    // Загнутый угол
    vesa::fill_rect_fast(x + 20, y, 8, 8, MELL_DESKTOP);
    vesa::fill_rect_fast(x + 20, y + 8, 8, 1, MELL_BORDER);
    vesa::fill_rect_fast(x + 20, y, 1, 8, MELL_BORDER);
    // Линии текста
    for i in 0..4u32 {
        vesa::fill_rect_fast(x + 4, y + 12 + i * 5, 20, 2, 0xCCCCCCu32);
    }

    let name_cx = cx.saturating_sub(name.len() / 2);
    put_str_at_bg(name_cx, cy + 3, name, label_fg, label_bg);
}

// ── Скроллбар ─────────────────────────────────────────────────────────────

pub fn draw_scrollbar(cx: usize, cy: usize, ch: usize, pos: usize, total: usize) {
    let x = px(cx);
    let y = py(cy);
    let h = ph(ch);

    // Трек
    vesa::fill_rect_fast(x, y, 8, h, 0x333333);

    if total > 0 && ch > 0 {
        let thumb_h = ((ch * ch) / total).max(1).min(ch);
        let thumb_pos = (pos * (ch - thumb_h)) / total;
        vesa::fill_rect_fast(x + 1, y + ph(thumb_pos), 6, ph(thumb_h), LGRAY);
    }
}

// ── Курсор мыши ───────────────────────────────────────────────────────────

// Размер курсора
const CW: u32 = 13;
const CH: u32 = 21;

// Форма курсора: 1=чёрный, 2=белая обводка, 0=прозрачно
// Каждая строка — 2 байта (16 бит, используем 13)
const CURSOR_SHAPE: &[u8] = &[
    // row 0..20, по 2 байта на строку
    0b11000000, 0b00000000, // 1100000000000
    0b11100000, 0b00000000, // 1110000000000
    0b11110000, 0b00000000,
    0b11111000, 0b00000000,
    0b11111100, 0b00000000,
    0b11111110, 0b00000000,
    0b11111111, 0b00000000,
    0b11111111, 0b10000000,
    0b11111100, 0b00000000,
    0b11011100, 0b00000000,
    0b10001110, 0b00000000,
    0b00001110, 0b00000000,
    0b00000111, 0b00000000,
    0b00000111, 0b00000000,
    0b00000011, 0b10000000,
    0b00000001, 0b10000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
];

// Обводка — пиксели вокруг формы курсора
const CURSOR_OUTLINE: &[u8] = &[
    0b11100000, 0b00000000,
    0b11110000, 0b00000000,
    0b11111000, 0b00000000,
    0b11111100, 0b00000000,
    0b11111110, 0b00000000,
    0b11111111, 0b00000000,
    0b11111111, 0b10000000,
    0b11111111, 0b11000000,
    0b11111110, 0b00000000,
    0b11111110, 0b00000000,
    0b11011111, 0b00000000,
    0b10011111, 0b00000000,
    0b00001111, 0b10000000,
    0b00001111, 0b10000000,
    0b00000111, 0b11000000,
    0b00000011, 0b11000000,
    0b00000001, 0b11000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
    0b00000000, 0b00000000,
];

static mut CURSOR_X: u32 = 400;
static mut CURSOR_Y: u32 = 300;

/// Обновляет позицию курсора (вызывается при движении мыши)
pub fn update_cursor_pos(x: u32, y: u32) {
    unsafe { CURSOR_X = x; CURSOR_Y = y; }
}

/// Рисует курсор в текущей позиции — вызывать в каждом кадре redraw
pub fn draw_cursor_at_current() {
    let (mx, my) = unsafe { (CURSOR_X, CURSOR_Y) };
    draw_cursor(mx, my);
}

pub fn draw_cursor(mx: u32, my: u32) {
    let fb = vesa::get();
    if !fb.ready { return; }

    for row in 0..CH {
        let b0 = CURSOR_OUTLINE[(row * 2) as usize];
        let b1 = CURSOR_OUTLINE[(row * 2 + 1) as usize];
        let outline_bits = ((b0 as u16) << 8) | (b1 as u16);

        let b0 = CURSOR_SHAPE[(row * 2) as usize];
        let b1 = CURSOR_SHAPE[(row * 2 + 1) as usize];
        let shape_bits = ((b0 as u16) << 8) | (b1 as u16);

        for col in 0..CW {
            let mask = 0x8000u16 >> col;
            let px = mx + col;
            let py = my + row;
            if px >= fb.width || py >= fb.height { continue; }

            if shape_bits & mask != 0 {
                vesa::put_pixel(px, py, 0x000000); // чёрный
            } else if outline_bits & mask != 0 {
                vesa::put_pixel(px, py, 0xFFFFFF); // белая обводка
            }
        }
    }
}

// ── Ввод с клавиатуры и мыши ─────────────────────────────────────────────

pub fn read_input() -> Input {
    loop {
        // Проверяем мышь
        if crate::drivers::ps2::check_mouse() {
            let mouse = crate::drivers::ps2::get_mouse();
            update_cursor_pos(mouse.x as u32, mouse.y as u32);
            return Input::Mouse(MouseEvent {
                x: (mouse.x as usize) / FONT_W as usize,
                y: (mouse.y as usize) / FONT_H as usize,
                px: mouse.x as usize,
                py: mouse.y as usize,
                buttons: mouse.buttons,
                dx: mouse.dx,
                dy: mouse.dy,
            });
        }

        // Проверяем клавиатуру
        if let Some(c) = crate::drivers::ps2::poll_char() {
            let key = match c {
                b'\n'   => Key::Enter,
                b'\x08' => Key::Backspace,
                0x1B => {
                    if let Some(b1) = crate::drivers::ps2::poll_char() {
                        if b1 == b'[' {
                            if let Some(b2) = crate::drivers::ps2::poll_char() {
                                match b2 {
                                    b'A' => Key::Up,
                                    b'B' => Key::Down,
                                    b'C' => Key::Right,
                                    b'D' => Key::Left,
                                    b'H' => Key::Home,
                                    b'F' => Key::End,
                                    b'5' => { crate::drivers::ps2::poll_char(); Key::PageUp }
                                    b'6' => { crate::drivers::ps2::poll_char(); Key::PageDown }
                                    _ => Key::Esc,
                                }
                            } else { Key::Esc }
                        } else { Key::Esc }
                    } else { Key::Esc }
                }
                0x09 => Key::Tab,
                c    => Key::Char(c),
            };
            return Input::Key(key);
        }

        for _ in 0..1000 { unsafe { core::arch::asm!("nop"); } }
    }
}

pub fn read_key() -> Key {
    loop {
        if let Input::Key(key) = read_input() {
            return key;
        }
    }
}

// ── Типы ──────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq)]
pub enum Key {
    Char(u8),
    Enter, Backspace, Esc, Tab,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
}

#[derive(Copy, Clone)]
pub struct MouseEvent {
    pub x: usize,   // символьные координаты
    pub y: usize,
    pub px: usize,  // пиксельные координаты
    pub py: usize,
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
}

#[derive(Copy, Clone)]
pub enum Input {
    Key(Key),
    Mouse(MouseEvent),
}

// ── Заглушки для совместимости (старый VGA API) ───────────────────────────

// color() теперь возвращает u32 напрямую — используется как fg цвет
pub const fn color(fg: u32, _bg: u32) -> u32 { fg }

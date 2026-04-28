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

#[inline]
pub fn screen_cols() -> usize {
    let fb = vesa::get();
    if fb.ready && fb.width >= FONT_W {
        (fb.width / FONT_W) as usize
    } else {
        SCR_W
    }
}

#[inline]
pub fn screen_rows() -> usize {
    let fb = vesa::get();
    if fb.ready && fb.height >= FONT_H {
        (fb.height / FONT_H) as usize
    } else {
        SCR_H
    }
}

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
pub const MELL_DESKTOP:  u32 = 0x008080;  // classic Windows 95 teal
pub const MELL_TITLEBAR: u32 = 0x808080;  // inactive titlebar
pub const MELL_TITLEBAR_ACTIVE: u32 = 0x000080; // active titlebar
pub const MELL_WINDOW:   u32 = 0xC0C0C0;  // classic 3D gray
pub const MELL_TASKBAR:  u32 = 0xC0C0C0;  // taskbar gray
pub const MELL_TOPBAR:   u32 = 0xC0C0C0;  // helper panel gray
pub const MELL_BORDER:   u32 = 0x000000;  // black outer edge
pub const MELL_TEXT:     u32 = 0x000000;
pub const MELL_TEXT_LIGHT: u32 = 0xFFFFFF;

// Цвета кнопок окна
pub const BTN_CLOSE:  u32 = 0xC0C0C0;
pub const BTN_MIN:    u32 = 0xC0C0C0;
pub const BTN_MAX:    u32 = 0xC0C0C0;

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
const CONTROL_BTN_SIZE: u32 = 14;
const CONTROL_BTN_RIGHT_PAD: u32 = 18;

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

        let tb_color = if self.focused { MELL_TITLEBAR_ACTIVE } else { MELL_TITLEBAR };
        let title_h = ph(TITLEBAR_H);

        // Тень
        vesa::fill_rect_fast(px0 + 3, py0 + 3, pw0, ph0, 0x202020);

        // Основа окна
        vesa::fill_rect_fast(px0, py0, pw0, ph0, MELL_WINDOW);
        draw_frame_3d(px0, py0, pw0, ph0, true);

        // Заголовок
        let inner_title_x = px0 + 3;
        let inner_title_y = py0 + 3;
        let inner_title_w = pw0.saturating_sub(6);
        let inner_title_h = title_h.saturating_sub(4).max(12);
        vesa::fill_rect_fast(inner_title_x, inner_title_y, inner_title_w, inner_title_h, tb_color);

        // Текст заголовка
        let title_y = self.y + (TITLEBAR_H - 1) / 2;
        put_str_at_bg(self.x + 2, title_y, self.title, WHITE, tb_color);

        // Кнопки управления окном в стиле Win95
        let btn_y = self.btn_y_px();
        let btn_close_x = self.close_btn_x_px();
        let btn_max_x = self.max_btn_x_px();
        let btn_min_x = self.min_btn_x_px();
        draw_win95_button_px(btn_min_x, btn_y, CONTROL_BTN_SIZE, CONTROL_BTN_SIZE, BTN_MIN, false);
        draw_win95_button_px(btn_max_x, btn_y, CONTROL_BTN_SIZE, CONTROL_BTN_SIZE, BTN_MAX, false);
        draw_win95_button_px(btn_close_x, btn_y, CONTROL_BTN_SIZE, CONTROL_BTN_SIZE, BTN_CLOSE, false);
        draw_min_glyph(btn_min_x + 3, btn_y + 8);
        draw_max_glyph(btn_max_x + 3, btn_y + 3);
        draw_close_glyph(btn_close_x + 4, btn_y + 4);

        // Клиентская область
        let client_y = py0 + title_h;
        let client_h = ph0.saturating_sub(title_h + 2);
        vesa::fill_rect_fast(px0 + 2, client_y, pw0.saturating_sub(4), client_h, MELL_WINDOW);
        draw_frame_3d(px0 + 2, client_y, pw0.saturating_sub(4), client_h, false);

        // Уголок для resize, чтобы было видно что окно можно тянуть
        let grip_x = px0 + pw0.saturating_sub(16);
        let grip_y = py0 + ph0.saturating_sub(16);
        for i in 0..4u32 {
            vesa::draw_hline(grip_x + i * 4, grip_y + 14, 2, DGRAY);
            vesa::draw_hline(grip_x + i * 4 + 1, grip_y + 12, 2, WHITE);
        }
    }

    pub fn inner_x(&self) -> usize { self.x + 1 }
    pub fn inner_y(&self) -> usize { self.y + TITLEBAR_H }
    pub fn inner_w(&self) -> usize { self.w.saturating_sub(2) }
    pub fn inner_h(&self) -> usize { self.h.saturating_sub(TITLEBAR_H + 1) }

    // Хит-тесты кнопок в пиксельных координатах
    fn close_btn_x_px(&self) -> u32 {
        px(self.x) + pw(self.w).saturating_sub(CONTROL_BTN_RIGHT_PAD)
    }
    fn max_btn_x_px(&self) -> u32 {
        self.close_btn_x_px().saturating_sub(CONTROL_BTN_SIZE)
    }
    fn min_btn_x_px(&self) -> u32 {
        self.max_btn_x_px().saturating_sub(CONTROL_BTN_SIZE)
    }
    fn btn_y_px(&self) -> u32 {
        py(self.y) + 4
    }
    fn point_in_rect_px(&self, mpx: usize, mpy: usize, x: u32, y: u32, w: u32, h: u32) -> bool {
        (mpx as u32) >= x && (mpx as u32) < x + w &&
        (mpy as u32) >= y && (mpy as u32) < y + h
    }

    /// Клик по красной кнопке (закрыть)
    pub fn close_btn_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        self.point_in_rect_px(
            mpx,
            mpy,
            self.close_btn_x_px(),
            self.btn_y_px(),
            CONTROL_BTN_SIZE,
            CONTROL_BTN_SIZE,
        )
    }

    /// Клик по жёлтой кнопке (развернуть/восстановить)
    pub fn max_btn_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        self.point_in_rect_px(
            mpx,
            mpy,
            self.max_btn_x_px(),
            self.btn_y_px(),
            CONTROL_BTN_SIZE,
            CONTROL_BTN_SIZE,
        )
    }

    /// Клик по зелёной кнопке (свернуть)
    pub fn min_btn_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        self.point_in_rect_px(
            mpx,
            mpy,
            self.min_btn_x_px(),
            self.btn_y_px(),
            CONTROL_BTN_SIZE,
            CONTROL_BTN_SIZE,
        )
    }

    /// Клик по заголовку (для drag), исключая кнопки
    pub fn title_bar_clicked_px(&self, mpx: usize, mpy: usize) -> bool {
        let px0 = px(self.x) as usize;
        let py0 = py(self.y) as usize;
        let ph_tb = ph(TITLEBAR_H) as usize;
        let title_right = self.min_btn_x_px().saturating_sub(4) as usize;
        // В пределах заголовка, но левее кнопок
        mpx >= px0 && mpx < title_right &&
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

fn draw_frame_3d(x: u32, y: u32, w: u32, h: u32, raised: bool) {
    if w < 2 || h < 2 {
        return;
    }

    let light = if raised { WHITE } else { DGRAY };
    let shadow = if raised { DGRAY } else { WHITE };
    let dark = BLACK;

    vesa::draw_hline(x, y, w, light);
    vesa::draw_vline(x, y, h, light);
    vesa::draw_hline(x, y + h - 1, w, dark);
    vesa::draw_vline(x + w - 1, y, h, dark);

    if w > 4 && h > 4 {
        vesa::draw_hline(x + 1, y + 1, w - 2, shadow);
        vesa::draw_vline(x + 1, y + 1, h - 2, shadow);
        vesa::draw_hline(x + 1, y + h - 2, w - 2, dark);
        vesa::draw_vline(x + w - 2, y + 1, h - 2, dark);
    }
}

fn draw_win95_button_px(x: u32, y: u32, w: u32, h: u32, color: u32, pressed: bool) {
    vesa::fill_rect_fast(x, y, w, h, color);
    draw_frame_3d(x, y, w, h, !pressed);
}

fn draw_close_glyph(x: u32, y: u32) {
    for i in 0..6 {
        vesa::put_pixel(x + i, y + i, BLACK);
        vesa::put_pixel(x + 5 - i, y + i, BLACK);
    }
}

fn draw_min_glyph(x: u32, y: u32) {
    vesa::fill_rect_fast(x, y, 7, 2, BLACK);
}

fn draw_max_glyph(x: u32, y: u32) {
    vesa::draw_rect_outline(x, y, 8, 7, BLACK);
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
    let bg = if active { 0xB8CCE4u32 } else { MELL_WINDOW };
    let fg = BLACK;
    let cw = label.len() + 2;
    let x = px(cx);
    let y = py(cy);
    let w = pw(cw);
    let h = ph(1).max(18);
    draw_win95_button_px(x, y, w, h, bg, active);
    put_str_at_bg(cx + 1, cy, label, fg, bg);
}

// ── Иконки ────────────────────────────────────────────────────────────────

pub fn draw_folder_icon(cx: usize, cy: usize, name: &str, selected: bool) {
    let icon_bg  = 0xD6B04A;
    let label_fg = WHITE;
    let label_bg = if selected { 0x000080 } else { MELL_DESKTOP };

    let x = px(cx);
    let y = py(cy);

    vesa::fill_rect_fast(x + 2, y + 6, 28, 18, icon_bg);
    draw_frame_3d(x + 2, y + 6, 28, 18, true);
    vesa::fill_rect_fast(x + 4, y + 2, 12, 8, lighten(icon_bg, 12));
    draw_frame_3d(x + 4, y + 2, 12, 8, true);
    vesa::fill_rect_fast(x + 4, y + 11, 24, 2, lighten(icon_bg, 24));

    let name_cx = cx.saturating_sub(name.len() / 2);
    put_str_at_bg(name_cx, cy + 3, name, label_fg, label_bg);
}

pub fn draw_file_icon(cx: usize, cy: usize, name: &str, selected: bool) {
    let icon_bg  = WHITE;
    let label_fg = WHITE;
    let label_bg = if selected { 0x000080 } else { MELL_DESKTOP };

    let x = px(cx);
    let y = py(cy);

    vesa::fill_rect_fast(x, y, 28, 32, icon_bg);
    draw_frame_3d(x, y, 28, 32, true);
    vesa::fill_rect_fast(x + 18, y + 2, 7, 7, 0xE0E0E0);
    vesa::draw_hline(x + 18, y + 9, 7, DGRAY);
    vesa::draw_vline(x + 18, y + 2, 7, DGRAY);
    for i in 0..4u32 {
        vesa::fill_rect_fast(x + 4, y + 12 + i * 5, 18, 2, 0x808080);
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
            let px = mx.saturating_add(col);
            let py = my.saturating_add(row);
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

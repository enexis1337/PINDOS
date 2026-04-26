// Mell Window Manager — стек окон, фокус, taskbar (VESA)

use super::vga_gui::*;
use crate::drivers::vesa;

pub const MAX_WINDOWS: usize = 16;

#[derive(Copy, Clone, PartialEq)]
pub enum AppId {
    None,
    Mocha,
    Qinn,
    Burmalda,
    Settings,
    Viewer,
}

pub struct WmWindow {
    pub win:       Window,
    pub app:       AppId,
    pub open:      bool,
    pub minimized: bool,   // свёрнуто в таскбар (зелёная кнопка)
    pub zorder:    u8,
}

impl WmWindow {
    pub fn new(x: usize, y: usize, w: usize, h: usize,
               title: &'static str, app: AppId) -> Self {
        WmWindow { win: Window::new(x, y, w, h, title), app, open: false, minimized: false, zorder: 0 }
    }
    pub fn empty() -> Self {
        WmWindow::new(0, 0, 0, 0, "", AppId::None)
    }
}

pub struct WindowManager {
    pub windows: [WmWindow; MAX_WINDOWS],
    pub count:   usize,
    pub focused: usize,
    pub tick:    u32,
}

// Дефолтные параметры окон по типу приложения
fn default_win_params(app: AppId) -> (usize, usize, usize, usize, &'static str) {
    match app {
        AppId::Burmalda => (4,  4,  80, 28, "Burmalda"),
        AppId::Mocha    => (30, 6,  80, 26, "Mocha"),
        AppId::Qinn     => (8,  4,  90, 32, "Qinn"),
        AppId::Settings => (20, 5,  75, 30, "Settings"),
        AppId::Viewer   => (8,  3, 100, 38, "Viewer"),
        AppId::None     => (0,  0,   0,  0, ""),
    }
}

impl WindowManager {
    pub fn new() -> Self {
        let mut wm = WindowManager {
            windows: core::array::from_fn(|_| WmWindow::empty()),
            count:   0,
            focused: 0,
            tick:    0,
        };
        // Предзаполняем слоты для каждого типа приложения
        let apps = [AppId::Burmalda, AppId::Mocha, AppId::Qinn, AppId::Settings, AppId::Viewer];
        for app in apps {
            let (x, y, w, h, title) = default_win_params(app);
            wm.windows[wm.count] = WmWindow::new(x, y, w, h, title, app);
            wm.count += 1;
        }
        wm
    }

    /// Открыть приложение. Если уже открыто — поднять на передний план.
    /// Если свёрнуто — развернуть. Иначе открыть новый экземпляр.
    pub fn open_app(&mut self, app: AppId) {
        // Ищем уже открытое (не свёрнутое) окно этого типа
        for i in 0..self.count {
            if self.windows[i].app == app && self.windows[i].open && !self.windows[i].minimized {
                self.raise(i);
                return;
            }
        }
        // Ищем свёрнутое — разворачиваем
        for i in 0..self.count {
            if self.windows[i].app == app && self.windows[i].open && self.windows[i].minimized {
                self.windows[i].minimized = false;
                self.raise(i);
                return;
            }
        }
        // Ищем закрытый слот этого типа
        for i in 0..self.count {
            if self.windows[i].app == app && !self.windows[i].open {
                self.windows[i].open = true;
                self.windows[i].minimized = false;
                self.raise(i);
                return;
            }
        }
        // Создаём новый слот если есть место
        if self.count < MAX_WINDOWS {
            let (x, y, w, h, title) = default_win_params(app);
            // Смещаем новое окно чтобы не перекрывало предыдущее
            let offset = (self.count * 3) % 20;
            let mut win = WmWindow::new(x + offset, y + offset, w, h, title, app);
            win.open = true;
            self.windows[self.count] = win;
            let idx = self.count;
            self.count += 1;
            self.raise(idx);
        }
    }

    pub fn close_window(&mut self, idx: usize) {
        self.windows[idx].open = false;
        self.windows[idx].minimized = false;
        // Передаём фокус следующему открытому окну
        let mut new_focus = None;
        let mut max_z = 0u8;
        for i in 0..self.count {
            if i != idx && self.windows[i].open && !self.windows[i].minimized {
                if self.windows[i].zorder >= max_z {
                    max_z = self.windows[i].zorder;
                    new_focus = Some(i);
                }
            }
        }
        if let Some(f) = new_focus {
            self.raise(f);
        } else {
            self.focused = 0;
            for i in 0..self.count {
                self.windows[i].win.focused = false;
            }
        }
    }

    pub fn minimize_window(&mut self, idx: usize) {
        self.windows[idx].minimized = true;
        // Передаём фокус
        let mut new_focus = None;
        let mut max_z = 0u8;
        for i in 0..self.count {
            if i != idx && self.windows[i].open && !self.windows[i].minimized {
                if self.windows[i].zorder >= max_z {
                    max_z = self.windows[i].zorder;
                    new_focus = Some(i);
                }
            }
        }
        if let Some(f) = new_focus {
            self.raise(f);
        } else {
            for i in 0..self.count {
                self.windows[i].win.focused = false;
            }
        }
    }

    pub fn close_focused(&mut self) {
        self.close_window(self.focused);
    }

    pub fn raise(&mut self, idx: usize) {
        let max_z = self.windows[..self.count]
            .iter().map(|w| w.zorder).max().unwrap_or(0);
        self.windows[idx].zorder = max_z.saturating_add(1);
        self.focused = idx;
        for i in 0..self.count {
            self.windows[i].win.focused = i == idx;
        }
    }

    pub fn cycle_focus(&mut self) {
        let mut open_list = [0usize; MAX_WINDOWS];
        let mut n = 0;
        for i in 0..self.count {
            if self.windows[i].open && !self.windows[i].minimized {
                open_list[n] = i; n += 1;
            }
        }
        if n == 0 { return; }
        let cur_pos = open_list[..n].iter().position(|&i| i == self.focused).unwrap_or(0);
        let next = open_list[(cur_pos + 1) % n];
        self.raise(next);
    }

    pub fn draw_desktop(&self) {
        let fb = vesa::get();
        if !fb.ready { return; }
        vesa::fill_rect_fast(0, py(1), fb.width, fb.height - py(3), MELL_DESKTOP);
        let mut grid_y = py(1);
        while grid_y < fb.height - py(2) {
            vesa::fill_rect_fast(0, grid_y, fb.width, 1, darken_color(MELL_DESKTOP, 15));
            grid_y += 32;
        }
    }

    pub fn draw_topbar(&self) {
        let fb = vesa::get();
        if !fb.ready { return; }
        let bar_h = py(2);
        vesa::fill_rect_fast(0, 0, fb.width, bar_h, MELL_TOPBAR);
        vesa::fill_rect_fast(0, bar_h - 1, fb.width, 1, 0x444444);
        put_str_at_bg(1, 0, " Mell ", 0xAADDFFu32, MELL_TOPBAR);
        let dt = crate::drivers::rtc::read();
        let mut tbuf = [0u8; 5];
        crate::drivers::rtc::format_time_short(&dt, &mut tbuf);
        let time_str = core::str::from_utf8(&tbuf).unwrap_or("00:00");
        let tx = VGA_WIDTH - time_str.len() - 2;
        put_str_at_bg(tx, 0, time_str, WHITE, MELL_TOPBAR);
        let mut dbuf = [0u8; 10];
        crate::drivers::rtc::format_date(&dt, &mut dbuf);
        let date_str = core::str::from_utf8(&dbuf).unwrap_or("01.01.2025");
        let dx = VGA_WIDTH - date_str.len() - time_str.len() - 4;
        put_str_at_bg(dx, 0, date_str, 0xAAAAAAu32, MELL_TOPBAR);
    }

    pub fn draw_taskbar(&self) {
        let fb = vesa::get();
        if !fb.ready { return; }
        let bar_y = fb.height - py(2);
        let bar_h = py(2);
        vesa::fill_rect_fast(0, bar_y, fb.width, bar_h, MELL_TASKBAR);
        vesa::fill_rect_fast(0, bar_y, fb.width, 1, 0x555555);
        let mut tx = 1usize;
        for i in 0..self.count {
            if !self.windows[i].open { continue; }
            let active = i == self.focused && !self.windows[i].minimized;
            let title = self.windows[i].win.title;
            let cy = VGA_HEIGHT - 2;
            // Свёрнутые окна показываем с отступом/курсивом (просто другой цвет)
            let btn_bg = if active { 0x1E5799u32 }
                         else if self.windows[i].minimized { 0x444444u32 }
                         else { 0x555555u32 };
            let cw = title.len() + 2;
            vesa::fill_rect_fast(px(tx), py(cy), pw(cw), ph(1), btn_bg);
            vesa::fill_rect_fast(px(tx), py(cy), pw(cw), 2, lighten(btn_bg, 40));
            put_str_at_bg(tx + 1, cy, title, WHITE, btn_bg);
            tx += title.len() + 3;
        }
    }
}

fn darken_color(c: u32, amt: u32) -> u32 {
    let r = ((c >> 16) & 0xFF).saturating_sub(amt);
    let g = ((c >> 8)  & 0xFF).saturating_sub(amt);
    let b = (c & 0xFF).saturating_sub(amt);
    (r << 16) | (g << 8) | b
}

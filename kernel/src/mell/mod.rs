// Mell — оконный менеджер PINDOS

pub mod vga_gui;
pub mod wm;
pub mod apps;

use vga_gui::*;
use wm::{WindowManager, AppId};
use apps::{mocha::MochaApp, qinn::QinnApp, burmalda::BurmaldaApp, settings::SettingsApp, viewer::ViewerApp};

// Mellstroy — ярлыки/ссылки на приложения Mell
pub struct Mellstroy {
    pub app:    AppId,
    pub label:  &'static str,
    pub icon_x: usize,
    pub icon_y: usize,
}

pub const MELLSTROY_ITEMS: &[Mellstroy] = &[
    Mellstroy { app: AppId::Mocha,    label: "Mocha",    icon_x: 2, icon_y: 3  },
    Mellstroy { app: AppId::Qinn,     label: "Qinn",     icon_x: 2, icon_y: 7  },
    Mellstroy { app: AppId::Burmalda, label: "Burmalda", icon_x: 2, icon_y: 11 },
    Mellstroy { app: AppId::Settings, label: "Settings", icon_x: 2, icon_y: 15 },
    Mellstroy { app: AppId::Viewer,   label: "Viewer",   icon_x: 2, icon_y: 19 },
];

// Состояние drag/resize
#[derive(Copy, Clone, PartialEq)]
enum DragMode {
    None,
    Move,
    Resize,
}

// Хранилище состояний приложений — по одному на слот окна
// Максимум MAX_WINDOWS экземпляров каждого типа
const MAX_APP_INSTANCES: usize = wm::MAX_WINDOWS;

pub struct MellDesktop {
    pub wm:       WindowManager,
    // Пулы экземпляров приложений
    burmaldas: [BurmaldaApp; 4],
    mochas:    [MochaApp;    4],
    qinns:     [QinnApp;     4],
    settings:  SettingsApp,
    viewer:    ViewerApp,
    // Маппинг: window index -> app instance index
    app_inst:  [usize; MAX_APP_INSTANCES],

    pub desktop_sel: usize,

    // Drag/resize state
    drag_mode:   DragMode,
    drag_win:    usize,
    drag_off_x:  usize,
    drag_off_y:  usize,
    resize_orig_w: usize,
    resize_orig_h: usize,
    resize_orig_px: usize,
    resize_orig_py: usize,

    // Maximize state
    maximized: [bool;  MAX_APP_INSTANCES],
    saved_x:   [usize; MAX_APP_INSTANCES],
    saved_y:   [usize; MAX_APP_INSTANCES],
    saved_w:   [usize; MAX_APP_INSTANCES],
    saved_h:   [usize; MAX_APP_INSTANCES],
}

impl MellDesktop {
    pub fn new() -> Self {
        MellDesktop {
            wm:          WindowManager::new(),
            burmaldas:   core::array::from_fn(|_| BurmaldaApp::new()),
            mochas:      core::array::from_fn(|_| MochaApp::new()),
            qinns:       core::array::from_fn(|_| QinnApp::new()),
            settings:    SettingsApp::new(),
            viewer:      ViewerApp::new(),
            app_inst:    [0; MAX_APP_INSTANCES],
            desktop_sel: 0,
            drag_mode:   DragMode::None,
            drag_win:    0,
            drag_off_x:  0,
            drag_off_y:  0,
            resize_orig_w:  0,
            resize_orig_h:  0,
            resize_orig_px: 0,
            resize_orig_py: 0,
            maximized: [false; MAX_APP_INSTANCES],
            saved_x:   [0; MAX_APP_INSTANCES],
            saved_y:   [0; MAX_APP_INSTANCES],
            saved_w:   [0; MAX_APP_INSTANCES],
            saved_h:   [0; MAX_APP_INSTANCES],
        }
    }

    pub fn run(&mut self) -> ! {
        self.wm.open_app(AppId::Burmalda);
        self.redraw();
        loop {
            let input = read_input();
            self.handle_input(input);
            self.redraw();
        }
    }

    fn redraw(&self) {
        self.wm.draw_topbar();
        self.wm.draw_desktop();
        for (i, ms) in MELLSTROY_ITEMS.iter().enumerate() {
            let selected = i == self.desktop_sel && !self.any_window_open();
            draw_folder_icon(ms.icon_x, ms.icon_y, ms.label, selected);
        }
        self.draw_all_windows();
        self.wm.draw_taskbar();
        draw_cursor_at_current();
    }

    fn draw_all_windows(&self) {
        let mut order = [0usize; wm::MAX_WINDOWS];
        for i in 0..self.wm.count { order[i] = i; }
        for i in 0..self.wm.count {
            for j in 0..self.wm.count.saturating_sub(1).saturating_sub(i) {
                if self.wm.windows[order[j]].zorder > self.wm.windows[order[j+1]].zorder {
                    order.swap(j, j+1);
                }
            }
        }
        for &i in &order[..self.wm.count] {
            let ww = &self.wm.windows[i];
            if !ww.open || ww.minimized { continue; }
            ww.win.draw();
            let inst = self.app_inst[i];
            match ww.app {
                AppId::Burmalda => self.burmaldas[inst % 4].draw(&ww.win),
                AppId::Mocha    => self.mochas[inst % 4].draw(&ww.win),
                AppId::Qinn     => self.qinns[inst % 4].draw(&ww.win),
                AppId::Settings => self.settings.draw(&ww.win),
                AppId::Viewer   => self.viewer.draw(&ww.win),
                AppId::None     => {}
            }
        }
    }

    fn any_window_open(&self) -> bool {
        self.wm.windows[..self.wm.count].iter().any(|w| w.open && !w.minimized)
    }

    fn handle_input(&mut self, input: Input) {
        match input {
            Input::Key(key)     => self.handle_key(key),
            Input::Mouse(mouse) => self.handle_mouse(mouse),
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        let btn_left = mouse.buttons & 0x01 != 0;

        // ── Продолжение drag/resize ───────────────────────────────────────
        if btn_left && self.drag_mode != DragMode::None {
            let i = self.drag_win;
            match self.drag_mode {
                DragMode::Move => {
                    let new_px = (mouse.px as isize - self.drag_off_x as isize).max(0) as usize;
                    let new_py = (mouse.py as isize - self.drag_off_y as isize).max(py(2) as isize) as usize;
                    // Конвертируем пиксели обратно в символьные единицы
                    let new_cx = new_px / FONT_W as usize;
                    let new_cy = new_py / FONT_H as usize;
                    let w = self.wm.windows[i].win.w;
                    let h = self.wm.windows[i].win.h;
                    self.wm.windows[i].win.x = new_cx.min(VGA_WIDTH.saturating_sub(w));
                    self.wm.windows[i].win.y = new_cy.min(VGA_HEIGHT.saturating_sub(h + 2));
                }
                DragMode::Resize => {
                    let dx = mouse.px as isize - self.resize_orig_px as isize;
                    let dy = mouse.py as isize - self.resize_orig_py as isize;
                    let new_w_px = (self.resize_orig_w as isize * FONT_W as isize + dx).max(FONT_W as isize * 20) as usize;
                    let new_h_px = (self.resize_orig_h as isize * FONT_H as isize + dy).max(FONT_H as isize * 8) as usize;
                    self.wm.windows[i].win.w = new_w_px / FONT_W as usize;
                    self.wm.windows[i].win.h = new_h_px / FONT_H as usize;
                }
                DragMode::None => {}
            }
            return;
        }

        if !btn_left {
            self.drag_mode = DragMode::None;
            return;
        }

        // ── Клик по таскбару — разворачиваем свёрнутые окна ──────────────
        let taskbar_y_px = py(VGA_HEIGHT - 2) as usize;
        if mouse.py >= taskbar_y_px {
            let mut tx = 1usize;
            for i in 0..self.wm.count {
                if !self.wm.windows[i].open { continue; }
                let title = self.wm.windows[i].win.title;
                let btn_px_start = px(tx) as usize;
                let btn_px_end   = px(tx + title.len() + 2) as usize;
                if mouse.px >= btn_px_start && mouse.px < btn_px_end {
                    if self.wm.windows[i].minimized {
                        self.wm.windows[i].minimized = false;
                    }
                    self.wm.raise(i);
                    return;
                }
                tx += title.len() + 3;
            }
            return;
        }

        // ── Ищем окно под курсором (по пиксельным координатам) ────────────
        let mut clicked: Option<usize> = None;
        let mut max_z = 0u8;
        for i in 0..self.wm.count {
            let ww = &self.wm.windows[i];
            if !ww.open || ww.minimized { continue; }
            if ww.win.contains_px(mouse.px, mouse.py) {
                if ww.zorder >= max_z {
                    max_z = ww.zorder;
                    clicked = Some(i);
                }
            }
        }

        if let Some(i) = clicked {
            let win = &self.wm.windows[i].win;

            // Кнопка закрытия (красная)
            if win.close_btn_clicked_px(mouse.px, mouse.py) {
                self.wm.close_window(i);
                return;
            }
            // Кнопка развернуть/восстановить (жёлтая)
            if win.max_btn_clicked_px(mouse.px, mouse.py) {
                if self.maximized[i] {
                    self.wm.windows[i].win.x = self.saved_x[i];
                    self.wm.windows[i].win.y = self.saved_y[i];
                    self.wm.windows[i].win.w = self.saved_w[i];
                    self.wm.windows[i].win.h = self.saved_h[i];
                    self.maximized[i] = false;
                } else {
                    self.saved_x[i] = self.wm.windows[i].win.x;
                    self.saved_y[i] = self.wm.windows[i].win.y;
                    self.saved_w[i] = self.wm.windows[i].win.w;
                    self.saved_h[i] = self.wm.windows[i].win.h;
                    self.wm.windows[i].win.x = 0;
                    self.wm.windows[i].win.y = 2;
                    self.wm.windows[i].win.w = VGA_WIDTH;
                    self.wm.windows[i].win.h = VGA_HEIGHT - 4;
                    self.maximized[i] = true;
                }
                self.wm.raise(i);
                return;
            }
            // Кнопка свернуть (зелёная)
            if win.min_btn_clicked_px(mouse.px, mouse.py) {
                self.wm.minimize_window(i);
                return;
            }

            // Resize handle (правый нижний угол)
            if win.resize_handle_clicked_px(mouse.px, mouse.py) && !self.maximized[i] {
                let orig_w = win.w;
                let orig_h = win.h;
                self.wm.raise(i);
                self.drag_mode = DragMode::Resize;
                self.drag_win  = i;
                self.resize_orig_w  = orig_w;
                self.resize_orig_h  = orig_h;
                self.resize_orig_px = mouse.px;
                self.resize_orig_py = mouse.py;
                return;
            }

            // Заголовок — drag
            if win.title_bar_clicked_px(mouse.px, mouse.py) {
                self.wm.raise(i);
                if self.maximized[i] {
                    // Снимаем максимизацию при drag
                    self.wm.windows[i].win.x = self.saved_x[i];
                    self.wm.windows[i].win.y = self.saved_y[i];
                    self.wm.windows[i].win.w = self.saved_w[i];
                    self.wm.windows[i].win.h = self.saved_h[i];
                    self.maximized[i] = false;
                }
                self.drag_mode  = DragMode::Move;
                self.drag_win   = i;
                self.drag_off_x = mouse.px - px(self.wm.windows[i].win.x) as usize;
                self.drag_off_y = mouse.py - py(self.wm.windows[i].win.y) as usize;
                return;
            }

            // Клик по содержимому
            self.wm.raise(i);
            let win = &self.wm.windows[i].win;
            let inner_x_px = px(win.inner_x()) as usize;
            let inner_y_px = py(win.inner_y()) as usize;
            let rel_x = mouse.px.saturating_sub(inner_x_px) / FONT_W as usize;
            let rel_y = mouse.py.saturating_sub(inner_y_px) / FONT_H as usize;
            let inst = self.app_inst[i];
            match self.wm.windows[i].app {
                AppId::Burmalda => self.burmaldas[inst % 4].handle_mouse(rel_x, rel_y),
                AppId::Mocha    => self.mochas[inst % 4].handle_mouse(rel_x, rel_y),
                AppId::Qinn     => self.qinns[inst % 4].handle_mouse(rel_x, rel_y),
                AppId::Settings => self.settings.handle_mouse(rel_x, rel_y),
                AppId::Viewer   => self.viewer.handle_mouse(rel_x, rel_y),
                AppId::None     => {}
            }
        } else {
            // Клик по рабочему столу
            for (idx, ms) in MELLSTROY_ITEMS.iter().enumerate() {
                if mouse.x >= ms.icon_x && mouse.x < ms.icon_x + 5 &&
                   mouse.y >= ms.icon_y && mouse.y < ms.icon_y + 4 {
                    self.desktop_sel = idx;
                    self.open_app(ms.app);
                    break;
                }
            }
        }
    }

    fn handle_key(&mut self, key: Key) {
        match key {
            Key::Tab => self.wm.cycle_focus(),

            Key::Esc => {
                if self.any_window_open() {
                    self.wm.close_focused();
                }
            }

            Key::Char(b'1') if !self.any_window_open() => self.open_app(AppId::Mocha),
            Key::Char(b'2') if !self.any_window_open() => self.open_app(AppId::Qinn),
            Key::Char(b'3') if !self.any_window_open() => self.open_app(AppId::Burmalda),
            Key::Char(b'4') if !self.any_window_open() => self.open_app(AppId::Settings),

            Key::Enter if !self.any_window_open() => {
                let app = MELLSTROY_ITEMS[self.desktop_sel].app;
                self.open_app(app);
            }

            Key::Down if !self.any_window_open() => {
                if self.desktop_sel + 1 < MELLSTROY_ITEMS.len() {
                    self.desktop_sel += 1;
                }
            }
            Key::Up if !self.any_window_open() => {
                if self.desktop_sel > 0 { self.desktop_sel -= 1; }
            }

            _ => {
                if self.any_window_open() {
                    self.dispatch_key_to_focused(key);
                }
            }
        }
    }

    fn open_app(&mut self, app: AppId) {
        // Считаем сколько уже открытых экземпляров этого типа
        let mut inst_count = 0usize;
        for i in 0..self.wm.count {
            if self.wm.windows[i].app == app && self.wm.windows[i].open {
                inst_count += 1;
            }
        }
        // Запоминаем текущий count до open_app
        let prev_count = self.wm.count;
        self.wm.open_app(app);
        // Если создан новый слот — назначаем ему instance index
        if self.wm.count > prev_count {
            let new_idx = self.wm.count - 1;
            self.app_inst[new_idx] = inst_count;
        }
        // Инициализируем если нужно
        if app == AppId::Mocha {
            let focused = self.wm.focused;
            let inst = self.app_inst[focused];
            self.mochas[inst % 4].refresh();
        }
    }

    fn dispatch_key_to_focused(&mut self, key: Key) {
        let focused = self.wm.focused;
        if focused >= self.wm.count { return; }
        let inst = self.app_inst[focused];
        match self.wm.windows[focused].app {
            AppId::Mocha    => self.mochas[inst % 4].handle_key(key),
            AppId::Qinn     => {
                let close = self.qinns[inst % 4].handle_key(key);
                if close { self.wm.close_focused(); }
            }
            AppId::Burmalda => self.burmaldas[inst % 4].handle_key(key),
            AppId::Settings => self.settings.handle_key(key),
            AppId::Viewer   => self.viewer.handle_key(key),
            AppId::None     => {}
        }
    }
}

/// Точка входа — запускается из шелла командой `mell`
pub fn run() -> ! {
    use core::mem::MaybeUninit;

    let fb = crate::drivers::vesa::get();
    if !fb.ready {
        crate::vga::print_colored("mell: VESA framebuffer not available.\n", 0x0C);
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    crate::drivers::vesa::enable_lfb();
    crate::drivers::vesa::clear(0x000000);

    static mut DESKTOP_BUF: MaybeUninit<MellDesktop> = MaybeUninit::uninit();
    unsafe {
        DESKTOP_BUF.write(MellDesktop::new());
        DESKTOP_BUF.assume_init_mut().run()
    }
}

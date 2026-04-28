// Mell — Windows 95 style desktop runtime

pub mod vga_gui;
pub mod wm;
pub mod apps;

use apps::{
    burmalda::BurmaldaApp,
    mocha::MochaApp,
    qinn::QinnApp,
    settings::SettingsApp,
    viewer::ViewerApp,
};
use vga_gui::*;

const TASKBAR_ROWS: usize = 2;
const START_BTN_W: usize = 8;
const START_MENU_W: usize = 28;
const START_MENU_H: usize = 16;
const APP_WINDOW_COUNT: usize = 5;
const MIN_WINDOW_W: usize = 34;
const MIN_WINDOW_H: usize = 12;
const BURMALDA_MIN_W: usize = 82;
const BURMALDA_MIN_H: usize = 28;

#[derive(Copy, Clone, PartialEq, Eq)]
enum AppKind {
    Mocha,
    Qinn,
    Burmalda,
    Settings,
    Viewer,
}

const APP_ORDER: [AppKind; APP_WINDOW_COUNT] = [
    AppKind::Mocha,
    AppKind::Qinn,
    AppKind::Burmalda,
    AppKind::Settings,
    AppKind::Viewer,
];

#[derive(Copy, Clone)]
enum Action {
    OpenApp(AppKind),
    OpenRun,
    OpenAbout,
    OpenHelp,
    CloseFocused,
}

#[derive(Copy, Clone)]
enum PointerGrab {
    None,
    Drag {
        idx: usize,
        grab_dx: i32,
        grab_dy: i32,
    },
    Resize {
        idx: usize,
        start_px: usize,
        start_py: usize,
        start_w: usize,
        start_h: usize,
    },
}

struct DesktopIcon {
    label: &'static str,
    action: Action,
    x: usize,
    y: usize,
    file_icon: bool,
}

const DESKTOP_ICONS: &[DesktopIcon] = &[
    DesktopIcon { label: "Mocha",        action: Action::OpenApp(AppKind::Mocha),    x: 3, y: 4,  file_icon: false },
    DesktopIcon { label: "Qinn",         action: Action::OpenApp(AppKind::Qinn),     x: 3, y: 9,  file_icon: false },
    DesktopIcon { label: "Burmalda",     action: Action::OpenApp(AppKind::Burmalda), x: 3, y: 14, file_icon: true  },
    DesktopIcon { label: "Settings",     action: Action::OpenApp(AppKind::Settings), x: 3, y: 19, file_icon: false },
    DesktopIcon { label: "Viewer",       action: Action::OpenApp(AppKind::Viewer),   x: 3, y: 24, file_icon: true  },
    DesktopIcon { label: "Recycle Bin",  action: Action::OpenAbout,                  x: 3, y: 29, file_icon: false },
];

struct StartEntry {
    label: &'static str,
    action: Action,
}

const START_MENU: &[StartEntry] = &[
    StartEntry { label: "Programs",     action: Action::OpenHelp },
    StartEntry { label: "Mocha",        action: Action::OpenApp(AppKind::Mocha) },
    StartEntry { label: "Qinn",         action: Action::OpenApp(AppKind::Qinn) },
    StartEntry { label: "Burmalda",     action: Action::OpenApp(AppKind::Burmalda) },
    StartEntry { label: "Settings",     action: Action::OpenApp(AppKind::Settings) },
    StartEntry { label: "Viewer",       action: Action::OpenApp(AppKind::Viewer) },
    StartEntry { label: "Run...",       action: Action::OpenRun },
    StartEntry { label: "About Mell95", action: Action::OpenAbout },
    StartEntry { label: "Close Window", action: Action::CloseFocused },
];

struct AppWindow {
    app: AppKind,
    win: Window,
    open: bool,
    minimized: bool,
    maximized: bool,
    z: u16,
    restore_x: usize,
    restore_y: usize,
    restore_w: usize,
    restore_h: usize,
}

impl AppWindow {
    fn new(app: AppKind, cascade: usize) -> Self {
        let win = build_window_for_app(app, cascade);
        AppWindow {
            app,
            restore_x: win.x,
            restore_y: win.y,
            restore_w: win.w,
            restore_h: win.h,
            win,
            open: false,
            minimized: false,
            maximized: false,
            z: 0,
        }
    }
}

pub struct MellRuntime {
    mocha: MochaApp,
    qinn: QinnApp,
    burmalda: BurmaldaApp,
    settings: SettingsApp,
    viewer: ViewerApp,
    windows: [AppWindow; APP_WINDOW_COUNT],
    focused: Option<usize>,
    next_z: u16,
    pointer_grab: PointerGrab,
    selected_icon: usize,
    selected_start: usize,
    start_open: bool,
    run_open: bool,
    about_open: bool,
    help_open: bool,
    run_input: [u8; 64],
    run_len: usize,
    status: [u8; 80],
    status_len: usize,
    last_left_down: bool,
}

impl MellRuntime {
    pub fn new() -> Self {
        let mut rt = MellRuntime {
            mocha: MochaApp::new(),
            qinn: QinnApp::new(),
            burmalda: BurmaldaApp::new(),
            settings: SettingsApp::new(),
            viewer: ViewerApp::new(),
            windows: core::array::from_fn(|i| AppWindow::new(APP_ORDER[i], i)),
            focused: None,
            next_z: 1,
            pointer_grab: PointerGrab::None,
            selected_icon: 0,
            selected_start: 0,
            start_open: false,
            run_open: false,
            about_open: false,
            help_open: false,
            run_input: [0; 64],
            run_len: 0,
            status: [0; 80],
            status_len: 0,
            last_left_down: false,
        };
        rt.set_status("Desktop ready");
        rt
    }

    pub fn run(&mut self) -> ! {
        crate::vga::serial_print("mell95: runtime start\n");
        crate::drivers::vesa::enable_backbuffer();
        self.redraw();
        loop {
            let input = read_input();
            self.handle_input(input);
            self.redraw();
        }
    }

    fn set_status(&mut self, msg: &str) {
        let b = msg.as_bytes();
        let len = b.len().min(self.status.len());
        self.status[..len].copy_from_slice(&b[..len]);
        self.status_len = len;
    }

    fn redraw(&self) {
        self.draw_background();
        self.draw_desktop_icons();
        self.draw_windows();
        self.draw_taskbar();
        if self.start_open {
            self.draw_start_menu();
        }
        if self.run_open {
            self.draw_run_dialog();
        }
        if self.about_open {
            self.draw_about_dialog();
        }
        if self.help_open {
            self.draw_help_dialog();
        }
        draw_cursor_at_current();
        crate::drivers::vesa::present();
    }

    fn draw_background(&self) {
        crate::drivers::vesa::clear(MELL_DESKTOP);
    }

    fn draw_desktop_icons(&self) {
        let desktop_selected = self.focused.is_none() && !self.start_open && !self.run_open
            && !self.about_open && !self.help_open;
        for (i, icon) in DESKTOP_ICONS.iter().enumerate() {
            let selected = desktop_selected && i == self.selected_icon;
            if icon.file_icon {
                draw_file_icon(icon.x, icon.y, icon.label, selected);
            } else {
                draw_folder_icon(icon.x, icon.y, icon.label, selected);
            }
        }
    }

    fn draw_windows(&self) {
        let mut drawn = [false; APP_WINDOW_COUNT];
        for _ in 0..APP_WINDOW_COUNT {
            let mut next_idx = None;
            let mut next_z = u16::MAX;
            for i in 0..APP_WINDOW_COUNT {
                let slot = &self.windows[i];
                if !slot.open || slot.minimized || drawn[i] {
                    continue;
                }
                if slot.z < next_z {
                    next_z = slot.z;
                    next_idx = Some(i);
                }
            }
            let Some(idx) = next_idx else { break };
            let win = &self.windows[idx].win;
            win.draw();
            self.draw_app(idx);
            drawn[idx] = true;
        }
    }

    fn draw_app(&self, idx: usize) {
        let slot = &self.windows[idx];
        match slot.app {
            AppKind::Mocha => self.mocha.draw(&slot.win),
            AppKind::Qinn => self.qinn.draw(&slot.win),
            AppKind::Burmalda => self.burmalda.draw(&slot.win),
            AppKind::Settings => self.settings.draw(&slot.win),
            AppKind::Viewer => self.viewer.draw(&slot.win),
        }
    }

    fn draw_taskbar(&self) {
        let cols = screen_cols();
        let row = screen_rows().saturating_sub(TASKBAR_ROWS);

        fill_rect(0, row, cols, TASKBAR_ROWS, b' ', MELL_TASKBAR);
        fill_rect(0, row, cols, 1, b' ', WHITE);
        draw_button(0, row, "Start", self.start_open);

        let mut btn_x = START_BTN_W + 1;
        for idx in 0..APP_WINDOW_COUNT {
            let slot = &self.windows[idx];
            if !slot.open {
                continue;
            }
            let active = self.focused == Some(idx) && !slot.minimized;
            let label = app_title(slot.app);
            draw_button(btn_x, row, label, active);
            btn_x += label.len() + 4;
        }

        let status = core::str::from_utf8(&self.status[..self.status_len]).unwrap_or("");
        if btn_x < cols.saturating_sub(18) {
            put_str_at_bg(btn_x, row, status, BLACK, MELL_TASKBAR);
        }

        let dt = crate::drivers::rtc::read();
        let mut tbuf = [0u8; 5];
        crate::drivers::rtc::format_time_short(&dt, &mut tbuf);
        let time_str = core::str::from_utf8(&tbuf).unwrap_or("00:00");
        let clock_w = time_str.len() + 2;
        let clock_x = cols.saturating_sub(clock_w + 1);
        fill_rect(clock_x, row, clock_w, 1, b' ', MELL_WINDOW);
        put_str_at_bg(clock_x + 1, row, time_str, BLACK, MELL_WINDOW);
    }

    fn draw_start_menu(&self) {
        let menu_x = 0usize;
        let menu_y = screen_rows().saturating_sub(TASKBAR_ROWS + START_MENU_H);

        fill_rect(menu_x, menu_y, START_MENU_W, START_MENU_H, b' ', MELL_WINDOW);
        draw_outer_panel(menu_x, menu_y, START_MENU_W, START_MENU_H, true);

        fill_rect(menu_x, menu_y, 4, START_MENU_H, b' ', MELL_TITLEBAR_ACTIVE);
        put_str_at_bg(menu_x + 1, menu_y + 1, "95", WHITE, MELL_TITLEBAR_ACTIVE);

        for (i, entry) in START_MENU.iter().enumerate() {
            let row = menu_y + 1 + i;
            let selected = i == self.selected_start;
            let bg = if selected { MELL_TITLEBAR_ACTIVE } else { MELL_WINDOW };
            let fg = if selected { WHITE } else { BLACK };
            fill_rect(4, row, START_MENU_W.saturating_sub(5), 1, b' ', bg);
            put_str_at_bg(6, row, entry.label, fg, bg);
        }
    }

    fn draw_run_dialog(&self) {
        let win = Window::new(center_x(44), center_y(9), 44, 9, "Run");
        win.draw();
        let ix = win.inner_x();
        let iy = win.inner_y();
        let iw = win.inner_w();

        put_str_at(ix + 1, iy + 1, "Type the name of a program, folder, or command.", BLACK);
        put_str_at(ix + 1, iy + 3, "Open:", BLACK);
        fill_rect(ix + 8, iy + 3, iw.saturating_sub(11), 1, b' ', WHITE);
        let input = core::str::from_utf8(&self.run_input[..self.run_len]).unwrap_or("");
        put_str_at_bg(ix + 9, iy + 3, input, BLACK, WHITE);
        put_char_at_bg(ix + 9 + self.run_len, iy + 3, b'_', BLACK, WHITE);
        draw_button(ix + iw.saturating_sub(18), iy + 5, "OK", false);
        draw_button(ix + iw.saturating_sub(11), iy + 5, "Cancel", false);
    }

    fn draw_about_dialog(&self) {
        let win = Window::new(center_x(48), center_y(12), 48, 12, "About Mell95");
        win.draw();
        let ix = win.inner_x();
        let iy = win.inner_y();
        fill_rect(ix, iy, win.inner_w(), win.inner_h(), b' ', MELL_WINDOW);
        put_str_at(ix + 2, iy + 1, "Mell95 Desktop", MELL_TITLEBAR_ACTIVE);
        put_str_at(ix + 2, iy + 3, "Classic shell inspired by Windows 95.", BLACK);
        put_str_at(ix + 2, iy + 4, "Built on PINDOS framebuffer GUI.", BLACK);
        put_str_at(ix + 2, iy + 6, "Apps available:", BLACK);
        put_str_at(ix + 4, iy + 7, "Mocha, Qinn, Burmalda,", BLACK);
        put_str_at(ix + 4, iy + 8, "Settings, Viewer, Run...", BLACK);
        put_str_at(ix + 2, iy + 10, "Press Enter or Esc to close.", DGRAY);
    }

    fn draw_help_dialog(&self) {
        let win = Window::new(center_x(52), center_y(14), 52, 14, "Programs");
        win.draw();
        let ix = win.inner_x();
        let iy = win.inner_y();
        fill_rect(ix, iy, win.inner_w(), win.inner_h(), b' ', MELL_WINDOW);
        put_str_at(ix + 1, iy + 1, "Accessories", MELL_TITLEBAR_ACTIVE);
        put_str_at(ix + 2, iy + 3, "Mocha     - file manager", BLACK);
        put_str_at(ix + 2, iy + 4, "Qinn      - text editor", BLACK);
        put_str_at(ix + 2, iy + 5, "Burmalda  - terminal", BLACK);
        put_str_at(ix + 2, iy + 6, "Settings  - system settings", BLACK);
        put_str_at(ix + 2, iy + 7, "Viewer    - image/audio viewer", BLACK);
        put_str_at(ix + 1, iy + 9, "Run commands: mocha, qinn, burmalda,", BLACK);
        put_str_at(ix + 1, iy + 10, "settings, viewer, about, help", BLACK);
        put_str_at(ix + 1, iy + 12, "Press Enter or Esc to close.", DGRAY);
    }

    fn handle_input(&mut self, input: Input) {
        match input {
            Input::Key(key) => self.handle_key(key),
            Input::Mouse(mouse) => self.handle_mouse(mouse),
        }
    }

    fn handle_key(&mut self, key: Key) {
        if self.about_open || self.help_open {
            if matches!(key, Key::Esc | Key::Enter) {
                self.about_open = false;
                self.help_open = false;
                self.set_status("Ready");
            }
            return;
        }

        if self.run_open {
            self.handle_run_key(key);
            return;
        }

        if self.start_open {
            self.handle_start_key(key);
            return;
        }

        if key == Key::Char(0x11) {
            self.close_focused_window();
            return;
        }

        if let Some(idx) = self.focused_window_idx() {
            self.handle_app_key(idx, key);
        } else {
            self.handle_desktop_key(key);
        }
    }

    fn handle_run_key(&mut self, key: Key) {
        match key {
            Key::Esc => {
                self.run_open = false;
                self.run_len = 0;
                self.set_status("Run canceled");
            }
            Key::Backspace => {
                if self.run_len > 0 {
                    self.run_len -= 1;
                }
            }
            Key::Enter => self.execute_run_command(),
            Key::Char(c) => {
                if self.run_len < self.run_input.len().saturating_sub(1) {
                    self.run_input[self.run_len] = c;
                    self.run_len += 1;
                }
            }
            _ => {}
        }
    }

    fn execute_run_command(&mut self) {
        let cmd = core::str::from_utf8(&self.run_input[..self.run_len]).unwrap_or("").trim();
        self.run_open = false;
        self.run_len = 0;

        match cmd {
            "mocha" | "computer" | "explorer" => self.open_app(AppKind::Mocha),
            "qinn" | "notepad" | "documents" => self.open_app(AppKind::Qinn),
            "burmalda" | "cmd" | "terminal" => self.open_app(AppKind::Burmalda),
            "settings" | "control" | "panel" => self.open_app(AppKind::Settings),
            "viewer" | "media" | "mplayer" => self.open_app(AppKind::Viewer),
            "about" => {
                self.about_open = true;
                self.help_open = false;
                self.set_status("About Mell95");
            }
            "help" | "programs" => {
                self.help_open = true;
                self.about_open = false;
                self.set_status("Programs");
            }
            "" => self.set_status("Run canceled"),
            _ => self.set_status("Unknown command"),
        }
    }

    fn handle_start_key(&mut self, key: Key) {
        match key {
            Key::Esc => self.start_open = false,
            Key::Up => {
                if self.selected_start == 0 {
                    self.selected_start = START_MENU.len().saturating_sub(1);
                } else {
                    self.selected_start -= 1;
                }
            }
            Key::Down | Key::Tab => {
                self.selected_start = (self.selected_start + 1) % START_MENU.len();
            }
            Key::Enter => {
                let action = START_MENU[self.selected_start].action;
                self.start_open = false;
                self.execute_action(action);
            }
            _ => {}
        }
    }

    fn handle_desktop_key(&mut self, key: Key) {
        match key {
            Key::Left | Key::Up => {
                if self.selected_icon == 0 {
                    self.selected_icon = DESKTOP_ICONS.len().saturating_sub(1);
                } else {
                    self.selected_icon -= 1;
                }
            }
            Key::Right | Key::Down | Key::Tab => {
                self.selected_icon = (self.selected_icon + 1) % DESKTOP_ICONS.len();
            }
            Key::Enter => {
                let action = DESKTOP_ICONS[self.selected_icon].action;
                self.execute_action(action);
            }
            Key::Char(b's') | Key::Char(b'S') => {
                self.start_open = true;
                self.selected_start = 0;
            }
            Key::Char(b'1') => self.open_app(AppKind::Mocha),
            Key::Char(b'2') => self.open_app(AppKind::Qinn),
            Key::Char(b'3') => self.open_app(AppKind::Burmalda),
            Key::Char(b'4') => self.open_app(AppKind::Settings),
            Key::Char(b'5') => self.open_app(AppKind::Viewer),
            _ => {}
        }
    }

    fn handle_app_key(&mut self, idx: usize, key: Key) {
        match self.windows[idx].app {
            AppKind::Mocha => self.mocha.handle_key(key),
            AppKind::Qinn => {
                if self.qinn.handle_key(key) {
                    self.close_window(idx);
                }
            }
            AppKind::Burmalda => self.burmalda.handle_key(key),
            AppKind::Settings => self.settings.handle_key(key),
            AppKind::Viewer => self.viewer.handle_key(key),
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        let left_down = mouse.buttons & 0x01 != 0;
        let pressed = left_down && !self.last_left_down;
        let released = !left_down && self.last_left_down;
        self.last_left_down = left_down;

        if left_down {
            match self.pointer_grab {
                PointerGrab::Drag { idx, grab_dx, grab_dy } => {
                    self.update_drag(idx, mouse.px, mouse.py, grab_dx, grab_dy);
                    return;
                }
                PointerGrab::Resize { idx, start_px, start_py, start_w, start_h } => {
                    self.update_resize(idx, mouse.px, mouse.py, start_px, start_py, start_w, start_h);
                    return;
                }
                PointerGrab::None => {}
            }
        }

        if released {
            self.pointer_grab = PointerGrab::None;
        }

        if !pressed {
            return;
        }

        let taskbar_row = screen_rows().saturating_sub(TASKBAR_ROWS);
        if mouse.y >= taskbar_row {
            if mouse.x < START_BTN_W {
                self.start_open = !self.start_open;
                self.selected_start = 0;
                return;
            }
            if let Some(idx) = self.taskbar_window_hit_test(mouse.x, mouse.y) {
                self.toggle_taskbar_window(idx);
                return;
            }
        }

        if self.about_open || self.help_open {
            self.about_open = false;
            self.help_open = false;
            self.set_status("Ready");
            return;
        }

        if self.run_open {
            self.handle_run_dialog_mouse(mouse);
            return;
        }

        if self.start_open {
            if let Some(entry) = self.start_hit_test(mouse.x, mouse.y) {
                self.selected_start = entry;
                let action = START_MENU[entry].action;
                self.start_open = false;
                self.execute_action(action);
                return;
            }
            self.start_open = false;
        }

        if let Some(idx) = self.topmost_window_at(mouse.px, mouse.py) {
            self.focus_window(idx);
            if self.windows[idx].win.close_btn_clicked_px(mouse.px, mouse.py) {
                self.close_window(idx);
                return;
            }
            if self.windows[idx].win.min_btn_clicked_px(mouse.px, mouse.py) {
                self.minimize_window(idx);
                return;
            }
            if self.windows[idx].win.max_btn_clicked_px(mouse.px, mouse.py) {
                self.toggle_maximize_window(idx);
                return;
            }
            if !self.windows[idx].maximized && self.windows[idx].win.resize_handle_clicked_px(mouse.px, mouse.py) {
                self.pointer_grab = PointerGrab::Resize {
                    idx,
                    start_px: mouse.px,
                    start_py: mouse.py,
                    start_w: self.windows[idx].win.w,
                    start_h: self.windows[idx].win.h,
                };
                self.set_status("Resize window");
                return;
            }
            if !self.windows[idx].maximized && self.windows[idx].win.title_bar_clicked_px(mouse.px, mouse.py) {
                self.pointer_grab = PointerGrab::Drag {
                    idx,
                    grab_dx: mouse.px as i32 - px(self.windows[idx].win.x) as i32,
                    grab_dy: mouse.py as i32 - py(self.windows[idx].win.y) as i32,
                };
                self.set_status("Move window");
                return;
            }
            self.dispatch_mouse_to_app(idx, mouse.px, mouse.py);
            return;
        }

        self.focused = None;
        self.sync_focus_flags();
        if let Some(icon) = self.icon_hit_test(mouse.x, mouse.y) {
            self.selected_icon = icon;
            self.execute_action(DESKTOP_ICONS[icon].action);
        } else {
            self.set_status("Desktop ready");
        }
    }

    fn handle_run_dialog_mouse(&mut self, mouse: MouseEvent) {
        let run = Window::new(center_x(44), center_y(9), 44, 9, "Run");
        if run.close_btn_clicked_px(mouse.px, mouse.py) {
            self.run_open = false;
            self.set_status("Run canceled");
            return;
        }

        let ix = run.inner_x();
        let iy = run.inner_y();
        let iw = run.inner_w();
        let ok_x = ix + iw.saturating_sub(18);
        let cancel_x = ix + iw.saturating_sub(11);
        let btn_y = iy + 5;

        if mouse.y == btn_y && mouse.x >= ok_x && mouse.x < ok_x + 4 {
            self.execute_run_command();
        } else if mouse.y == btn_y && mouse.x >= cancel_x && mouse.x < cancel_x + 8 {
            self.run_open = false;
            self.run_len = 0;
            self.set_status("Run canceled");
        }
    }

    fn dispatch_mouse_to_app(&mut self, idx: usize, mouse_px: usize, mouse_py: usize) {
        let win = &self.windows[idx].win;
        let inner_x = px(win.inner_x()) as usize;
        let inner_y = py(win.inner_y()) as usize;
        let inner_w = pw(win.inner_w()) as usize;
        let inner_h = ph(win.inner_h()) as usize;
        if mouse_px < inner_x || mouse_px >= inner_x + inner_w || mouse_py < inner_y || mouse_py >= inner_y + inner_h {
            return;
        }

        let rel_x = (mouse_px - inner_x) / FONT_W as usize;
        let rel_y = (mouse_py - inner_y) / FONT_H as usize;
        match self.windows[idx].app {
            AppKind::Mocha => self.mocha.handle_mouse(rel_x, rel_y),
            AppKind::Qinn => self.qinn.handle_mouse(rel_x, rel_y),
            AppKind::Burmalda => self.burmalda.handle_mouse(rel_x, rel_y),
            AppKind::Settings => self.settings.handle_mouse(rel_x, rel_y),
            AppKind::Viewer => self.viewer.handle_mouse(rel_x, rel_y),
        }
    }

    fn update_drag(&mut self, idx: usize, mouse_px: usize, mouse_py: usize, grab_dx: i32, grab_dy: i32) {
        let cols = screen_cols();
        let rows = desktop_rows();
        let new_x = (mouse_px as i32 - grab_dx).max(0);
        let new_y = (mouse_py as i32 - grab_dy).max(0);
        let cell_x = (new_x as usize) / FONT_W as usize;
        let cell_y = (new_y as usize) / FONT_H as usize;
        let win = &mut self.windows[idx].win;
        win.x = cell_x.min(cols.saturating_sub(win.w));
        win.y = cell_y.min(rows.saturating_sub(win.h));
    }

    fn update_resize(
        &mut self,
        idx: usize,
        mouse_px: usize,
        mouse_py: usize,
        start_px: usize,
        start_py: usize,
        start_w: usize,
        start_h: usize,
    ) {
        let app = self.windows[idx].app;
        let (min_w, min_h) = app_min_window_size(app);
        let delta_cols = pixel_delta_to_cells(mouse_px as i32 - start_px as i32, FONT_W as usize);
        let delta_rows = pixel_delta_to_cells(mouse_py as i32 - start_py as i32, FONT_H as usize);
        let new_w = (start_w as i32 + delta_cols).max(min_w as i32) as usize;
        let new_h = (start_h as i32 + delta_rows).max(min_h as i32) as usize;
        let win = &mut self.windows[idx].win;
        win.w = new_w;
        win.h = new_h;
        clamp_window_to_desktop_for_app(win, app);
    }

    fn icon_hit_test(&self, x: usize, y: usize) -> Option<usize> {
        for (i, icon) in DESKTOP_ICONS.iter().enumerate() {
            if x >= icon.x && x < icon.x + 10 && y >= icon.y && y < icon.y + 4 {
                return Some(i);
            }
        }
        None
    }

    fn start_hit_test(&self, x: usize, y: usize) -> Option<usize> {
        let menu_y = screen_rows().saturating_sub(TASKBAR_ROWS + START_MENU_H);
        if x >= 4 && x < START_MENU_W && y > menu_y && y <= menu_y + START_MENU.len() {
            Some(y - menu_y - 1)
        } else {
            None
        }
    }

    fn taskbar_window_hit_test(&self, x: usize, y: usize) -> Option<usize> {
        let row = screen_rows().saturating_sub(TASKBAR_ROWS);
        if y != row {
            return None;
        }

        let mut btn_x = START_BTN_W + 1;
        for idx in 0..APP_WINDOW_COUNT {
            let slot = &self.windows[idx];
            if !slot.open {
                continue;
            }
            let label = app_title(slot.app);
            let btn_w = label.len() + 2;
            if x >= btn_x && x < btn_x + btn_w {
                return Some(idx);
            }
            btn_x += label.len() + 4;
        }
        None
    }

    fn topmost_window_at(&self, mouse_px: usize, mouse_py: usize) -> Option<usize> {
        let mut best = None;
        let mut best_z = 0u16;
        for idx in 0..APP_WINDOW_COUNT {
            let slot = &self.windows[idx];
            if !slot.open || slot.minimized || !slot.win.contains_px(mouse_px, mouse_py) {
                continue;
            }
            if best.is_none() || slot.z >= best_z {
                best = Some(idx);
                best_z = slot.z;
            }
        }
        best
    }

    fn execute_action(&mut self, action: Action) {
        match action {
            Action::OpenApp(app) => self.open_app(app),
            Action::OpenRun => {
                self.run_open = true;
                self.run_len = 0;
                self.about_open = false;
                self.help_open = false;
                self.set_status("Run");
            }
            Action::OpenAbout => {
                self.about_open = true;
                self.help_open = false;
                self.run_open = false;
                self.set_status("About Mell95");
            }
            Action::OpenHelp => {
                self.help_open = true;
                self.about_open = false;
                self.run_open = false;
                self.set_status("Programs");
            }
            Action::CloseFocused => self.close_focused_window(),
        }
    }

    fn open_app(&mut self, app: AppKind) {
        let idx = app_slot(app);
        self.start_open = false;
        self.run_open = false;
        self.about_open = false;
        self.help_open = false;
        self.windows[idx].open = true;
        self.windows[idx].minimized = false;
        clamp_window_to_desktop_for_app(&mut self.windows[idx].win, app);
        self.focus_window(idx);
        self.set_status(app_title(app));

        if app == AppKind::Mocha {
            self.mocha.refresh();
        }
    }

    fn close_focused_window(&mut self) {
        if let Some(idx) = self.focused_window_idx() {
            self.close_window(idx);
        }
    }

    fn close_window(&mut self, idx: usize) {
        self.windows[idx].open = false;
        self.windows[idx].minimized = false;
        self.windows[idx].maximized = false;
        self.pointer_grab = PointerGrab::None;
        if self.focused == Some(idx) {
            self.focused = self.pick_next_focus(Some(idx));
        }
        self.sync_focus_flags();
        self.set_status("Window closed");
    }

    fn minimize_window(&mut self, idx: usize) {
        self.windows[idx].minimized = true;
        self.pointer_grab = PointerGrab::None;
        if self.focused == Some(idx) {
            self.focused = self.pick_next_focus(Some(idx));
        }
        self.sync_focus_flags();
        self.set_status("Window minimized");
    }

    fn toggle_maximize_window(&mut self, idx: usize) {
        if self.windows[idx].maximized {
            self.windows[idx].maximized = false;
            self.windows[idx].win.x = self.windows[idx].restore_x;
            self.windows[idx].win.y = self.windows[idx].restore_y;
            self.windows[idx].win.w = self.windows[idx].restore_w;
            self.windows[idx].win.h = self.windows[idx].restore_h;
            let app = self.windows[idx].app;
            clamp_window_to_desktop_for_app(&mut self.windows[idx].win, app);
            self.set_status("Window restored");
        } else {
            self.windows[idx].restore_x = self.windows[idx].win.x;
            self.windows[idx].restore_y = self.windows[idx].win.y;
            self.windows[idx].restore_w = self.windows[idx].win.w;
            self.windows[idx].restore_h = self.windows[idx].win.h;
            self.windows[idx].maximized = true;
            self.windows[idx].minimized = false;
            self.windows[idx].win.x = 0;
            self.windows[idx].win.y = 0;
            self.windows[idx].win.w = screen_cols();
            self.windows[idx].win.h = desktop_rows();
            self.set_status("Window maximized");
        }
        self.focus_window(idx);
    }

    fn toggle_taskbar_window(&mut self, idx: usize) {
        if self.windows[idx].minimized {
            self.windows[idx].minimized = false;
            self.focus_window(idx);
            self.set_status(app_title(self.windows[idx].app));
        } else if self.focused == Some(idx) {
            self.minimize_window(idx);
        } else {
            self.focus_window(idx);
            self.set_status(app_title(self.windows[idx].app));
        }
    }

    fn focus_window(&mut self, idx: usize) {
        self.focused = Some(idx);
        self.windows[idx].open = true;
        self.windows[idx].minimized = false;
        self.windows[idx].z = self.next_z;
        self.next_z = self.next_z.saturating_add(1);
        self.sync_focus_flags();
    }

    fn pick_next_focus(&self, skip: Option<usize>) -> Option<usize> {
        let mut best = None;
        let mut best_z = 0u16;
        for idx in 0..APP_WINDOW_COUNT {
            if Some(idx) == skip {
                continue;
            }
            let slot = &self.windows[idx];
            if !slot.open || slot.minimized {
                continue;
            }
            if best.is_none() || slot.z >= best_z {
                best = Some(idx);
                best_z = slot.z;
            }
        }
        best
    }

    fn focused_window_idx(&self) -> Option<usize> {
        self.focused.filter(|&idx| self.windows[idx].open && !self.windows[idx].minimized)
    }

    fn sync_focus_flags(&mut self) {
        for idx in 0..APP_WINDOW_COUNT {
            self.windows[idx].win.focused = self.focused == Some(idx)
                && self.windows[idx].open
                && !self.windows[idx].minimized;
        }
    }
}

fn app_slot(app: AppKind) -> usize {
    match app {
        AppKind::Mocha => 0,
        AppKind::Qinn => 1,
        AppKind::Burmalda => 2,
        AppKind::Settings => 3,
        AppKind::Viewer => 4,
    }
}

fn app_title(app: AppKind) -> &'static str {
    match app {
        AppKind::Mocha => "Mocha",
        AppKind::Qinn => "Qinn",
        AppKind::Burmalda => "Burmalda",
        AppKind::Settings => "Settings",
        AppKind::Viewer => "Viewer",
    }
}

fn build_window_for_app(app: AppKind, cascade: usize) -> Window {
    let (w, h) = match app {
        AppKind::Mocha => (56, 20),
        AppKind::Qinn => (60, 22),
        AppKind::Burmalda => (86, 28),
        AppKind::Settings => (52, 18),
        AppKind::Viewer => (58, 20),
    };

    let cols = screen_cols();
    let rows = desktop_rows();
    let mut win = Window::new(
        (4 + cascade * 4).min(cols.saturating_sub(w)),
        (2 + cascade * 2).min(rows.saturating_sub(h)),
        w,
        h,
        app_title(app),
    );
    clamp_window_to_desktop_for_app(&mut win, app);
    win
}

fn app_min_window_size(app: AppKind) -> (usize, usize) {
    match app {
        AppKind::Burmalda => (BURMALDA_MIN_W, BURMALDA_MIN_H),
        _ => (MIN_WINDOW_W, MIN_WINDOW_H),
    }
}

fn clamp_window_to_desktop_for_app(win: &mut Window, app: AppKind) {
    let cols = screen_cols();
    let rows = desktop_rows();
    let (min_w, min_h) = app_min_window_size(app);
    let max_w = cols.max(min_w);
    let max_h = rows.max(min_h);

    win.w = win.w.min(max_w).max(min_w.min(cols.max(1)));
    win.h = win.h.min(max_h).max(min_h.min(rows.max(1)));
    win.x = win.x.min(cols.saturating_sub(win.w));
    win.y = win.y.min(rows.saturating_sub(win.h));
}

fn pixel_delta_to_cells(delta_px: i32, cell_px: usize) -> i32 {
    if delta_px >= 0 {
        (delta_px as usize / cell_px) as i32
    } else {
        -(((-delta_px) as usize / cell_px) as i32)
    }
}

fn desktop_rows() -> usize {
    screen_rows().saturating_sub(TASKBAR_ROWS)
}

fn center_x(w: usize) -> usize {
    screen_cols().saturating_sub(w) / 2
}

fn center_y(h: usize) -> usize {
    desktop_rows().saturating_sub(h) / 2
}

fn draw_outer_panel(x: usize, y: usize, w: usize, h: usize, raised: bool) {
    let light = if raised { WHITE } else { DGRAY };
    let shadow = if raised { DGRAY } else { WHITE };
    draw_hline(x, y, w, light);
    draw_vline(x, y, h, light);
    draw_hline(x, y + h - 1, w, BLACK);
    draw_vline(x + w - 1, y, h, BLACK);
    if w > 2 && h > 2 {
        draw_hline(x + 1, y + 1, w - 2, shadow);
        draw_vline(x + 1, y + 1, h - 2, shadow);
    }
}

/// Точка входа — запускается из шелла командой `mell`
pub fn run() -> ! {
    use core::mem::MaybeUninit;

    if !crate::drivers::vesa::get().ready {
        crate::drivers::vesa::probe_only();
    }
    crate::drivers::vesa::try_qemu_vga_std();

    let fb = crate::drivers::vesa::get();
    crate::vga::serial_print("mell95 fb addr=");
    crate::vga::serial_print_hex_u32(fb.addr);
    crate::vga::serial_print(" w=");
    crate::vga::serial_print_u32(fb.width);
    crate::vga::serial_print(" h=");
    crate::vga::serial_print_u32(fb.height);
    crate::vga::serial_print(" pitch=");
    crate::vga::serial_print_u32(fb.pitch);
    crate::vga::serial_print(" bpp=");
    crate::vga::serial_print_u32(fb.bpp as u32);
    crate::vga::serial_print("\n");

    if !fb.ready || !fb.active {
        crate::vga::print_colored("mell95: VESA framebuffer not available.\n", 0x0C);
        loop {
            unsafe { core::arch::asm!("hlt"); }
        }
    }

    static mut RUNTIME_BUF: MaybeUninit<MellRuntime> = MaybeUninit::uninit();
    unsafe {
        RUNTIME_BUF.write(MellRuntime::new());
        RUNTIME_BUF.assume_init_mut().run()
    }
}

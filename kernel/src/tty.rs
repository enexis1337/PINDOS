// TTY менеджер — 6 независимых сессий шелла
// Переключение: Ctrl+Alt+F1..F6

pub const TTY_COUNT: usize = 6;

#[derive(Copy, Clone, PartialEq)]
pub enum TtyState {
    Login,   // ожидает логина
    Shell,   // активный шелл
}

pub struct Tty {
    pub state:    TtyState,
    pub username: [u8; 32],
    pub uname_len: usize,
    pub is_su:    bool,
}

impl Tty {
    const fn new() -> Self {
        Tty {
            state:     TtyState::Login,
            username:  [0u8; 32],
            uname_len: 0,
            is_su:     false,
        }
    }

    pub fn username_str(&self) -> &str {
        core::str::from_utf8(&self.username[..self.uname_len]).unwrap_or("")
    }

    pub fn set_user(&mut self, name: &str, is_su: bool) {
        let b = name.as_bytes();
        let l = b.len().min(32);
        self.username[..l].copy_from_slice(&b[..l]);
        self.uname_len = l;
        self.is_su = is_su;
        self.state = TtyState::Shell;
    }

    pub fn logout(&mut self) {
        self.state = TtyState::Login;
        self.uname_len = 0;
        self.is_su = false;
    }
}

static mut TTYS: [Tty; TTY_COUNT] = [
    Tty::new(), Tty::new(), Tty::new(),
    Tty::new(), Tty::new(), Tty::new(),
];
static mut ACTIVE_TTY: usize = 0;

pub fn active() -> usize {
    unsafe { ACTIVE_TTY }
}

pub fn switch_to(n: usize) {
    if n == 0 || n > TTY_COUNT { return; }
    let idx = n - 1;
    unsafe {
        if idx == ACTIVE_TTY { return; }
        ACTIVE_TTY = idx;
    }
    crate::vga::clear_screen();
    crate::vga::print_colored("Switched to TTY", 0x0B);
    crate::vga::put_char(b'0' + n as u8);
    crate::vga::put_char(b'\n');
}

pub fn current_tty() -> &'static mut Tty {
    unsafe { &mut TTYS[ACTIVE_TTY] }
}

/// Главный цикл TTY — запускается из kernel_main вместо прямого вызова шелла
pub fn run() -> ! {
    loop {
        // Проверяем запрос переключения TTY
        if let Some(n) = crate::drivers::ps2::take_tty_switch() {
            switch_to(n as usize);
            continue;
        }

        let tty = current_tty();
        match tty.state {
            TtyState::Login => {
                if crate::auth::login() {
                    let name = crate::auth::current_name();
                    let is_su = crate::auth::is_su();
                    current_tty().set_user(name, is_su);
                }
                // После login тоже проверяем TTY switch
                if let Some(n) = crate::drivers::ps2::take_tty_switch() {
                    switch_to(n as usize);
                }
            }
            TtyState::Shell => {
                let name = current_tty().username_str();
                let is_su = current_tty().is_su;
                let mut name_buf = [0u8; 32];
                let name_len = name.len().min(32);
                name_buf[..name_len].copy_from_slice(&name.as_bytes()[..name_len]);
                let name_str = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("user");

                crate::uglyshell::run_as(name_str, is_su);

                // Если вернулись из-за TTY switch — не делаем logout
                if crate::drivers::ps2::take_tty_switch().is_none() {
                    current_tty().logout();
                }
            }
        }
    }
}

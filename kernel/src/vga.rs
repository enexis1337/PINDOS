// VGA text mode 80x25, буфер по адресу 0xB8000

const VGA_BUFFER: *mut u8 = 0xB8000 as *mut u8;
const VGA_CELL_W: u32 = 8;
const VGA_CELL_H: u32 = 16;

// ── Capture буфер — перехват вывода для Burmalda ──────────────────────────
const CAPTURE_BUF_SIZE: usize = 8192;
static mut CAPTURE_BUF: [u8; CAPTURE_BUF_SIZE] = [0u8; CAPTURE_BUF_SIZE];
static mut CAPTURE_LEN: usize = 0;
static mut CAPTURE_ACTIVE: bool = false;

pub fn capture_start() {
    unsafe { CAPTURE_LEN = 0; CAPTURE_ACTIVE = true; }
}

pub fn capture_end() -> &'static str {
    unsafe {
        CAPTURE_ACTIVE = false;
        core::str::from_utf8(&CAPTURE_BUF[..CAPTURE_LEN]).unwrap_or("")
    }
}

fn capture_push(b: u8) {
    unsafe {
        if CAPTURE_ACTIVE && CAPTURE_LEN < CAPTURE_BUF_SIZE - 1 {
            CAPTURE_BUF[CAPTURE_LEN] = b;
            CAPTURE_LEN += 1;
        }
    }
}

// COM1 serial port для отладки
const COM1: u16 = 0x3F8;

pub fn serial_init() {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") COM1 + 1, in("al") 0u8); // disable interrupts
        core::arch::asm!("out dx, al", in("dx") COM1 + 3, in("al") 0x80u8); // DLAB on
        core::arch::asm!("out dx, al", in("dx") COM1 + 0, in("al") 1u8);    // 115200 baud lo
        core::arch::asm!("out dx, al", in("dx") COM1 + 1, in("al") 0u8);    // baud hi
        core::arch::asm!("out dx, al", in("dx") COM1 + 3, in("al") 0x03u8); // 8N1, DLAB off
        core::arch::asm!("out dx, al", in("dx") COM1 + 2, in("al") 0xC7u8); // FIFO
        core::arch::asm!("out dx, al", in("dx") COM1 + 4, in("al") 0x0Bu8); // RTS/DTR
    }
}

pub fn serial_putc(c: u8) {
    unsafe {
        // Ждём пока TX buffer пуст
        let mut timeout = 10000u32;
        loop {
            let lsr: u8;
            core::arch::asm!("in al, dx", out("al") lsr, in("dx") COM1 + 5);
            if lsr & 0x20 != 0 { break; }
            timeout -= 1;
            if timeout == 0 { return; }
        }
        core::arch::asm!("out dx, al", in("dx") COM1, in("al") c);
    }
}

pub fn serial_print(s: &str) {
    for b in s.bytes() {
        if b == b'\n' { serial_putc(b'\r'); }
        serial_putc(b);
    }
}

pub fn serial_print_u32(mut n: u32) {
    if n == 0 {
        serial_putc(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0usize;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        serial_putc(buf[i]);
    }
}

pub fn serial_print_hex_u32(n: u32) {
    serial_print("0x");
    for i in (0..8).rev() {
        let nibble = ((n >> (i * 4)) & 0xF) as u8;
        serial_putc(if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 });
    }
}
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const DEFAULT_COLOR: u8 = 0x0F; // белый на чёрном

static mut CURSOR_X: usize = 0;
static mut CURSOR_Y: usize = 0;

fn framebuffer_console_active() -> bool {
    let fb = crate::drivers::vesa::get();
    fb.ready && fb.active
}

fn vga_color_to_rgb(color: u8) -> u32 {
    match color {
        0x0 => 0x000000,
        0x1 => 0x0000AA,
        0x2 => 0x00AA00,
        0x3 => 0x00AAAA,
        0x4 => 0xAA0000,
        0x5 => 0xAA00AA,
        0x6 => 0xAA5500,
        0x7 => 0xAAAAAA,
        0x8 => 0x555555,
        0x9 => 0x5555FF,
        0xA => 0x55FF55,
        0xB => 0x55FFFF,
        0xC => 0xFF5555,
        0xD => 0xFF55FF,
        0xE => 0xFFFF55,
        _ => 0xFFFFFF,
    }
}

fn draw_fb_cell(x: usize, y: usize, c: u8, color: u8) {
    let fg = vga_color_to_rgb(color & 0x0F);
    let bg = vga_color_to_rgb((color >> 4) & 0x0F);
    crate::drivers::vesa::draw_char(
        (x as u32) * VGA_CELL_W,
        (y as u32) * VGA_CELL_H,
        c,
        fg,
        bg,
    );
}

fn clear_fb_screen() {
    let bg = vga_color_to_rgb((DEFAULT_COLOR >> 4) & 0x0F);
    crate::drivers::vesa::clear(bg);
}

fn scroll_fb() {
    let bg = vga_color_to_rgb((DEFAULT_COLOR >> 4) & 0x0F);
    for row in 1..VGA_HEIGHT {
        for col in 0..VGA_WIDTH {
            let src = (row * VGA_WIDTH + col) * 2;
            let ch = unsafe { *VGA_BUFFER.add(src) };
            let color = unsafe { *VGA_BUFFER.add(src + 1) };
            draw_fb_cell(col, row - 1, ch, color);
        }
    }
    for col in 0..VGA_WIDTH {
        draw_fb_cell(col, VGA_HEIGHT - 1, b' ', DEFAULT_COLOR);
    }
    crate::drivers::vesa::fill_rect_fast(
        0,
        ((VGA_HEIGHT - 1) as u32) * VGA_CELL_H,
        (VGA_WIDTH as u32) * VGA_CELL_W,
        VGA_CELL_H,
        bg,
    );
}

fn write_cell(x: usize, y: usize, c: u8, color: u8) {
    let offset = (y * VGA_WIDTH + x) * 2;
    unsafe {
        *VGA_BUFFER.add(offset) = c;
        *VGA_BUFFER.add(offset + 1) = color;
    }
    if framebuffer_console_active() {
        draw_fb_cell(x, y, c, color);
    }
}

pub fn clear_screen() {
    unsafe {
        if CAPTURE_ACTIVE {
            capture_push(0x1B);
            capture_push(b'[');
            capture_push(b'2');
            capture_push(b'J');
            capture_push(0x1B);
            capture_push(b'[');
            capture_push(b'H');
        }
    }
    for y in 0..VGA_HEIGHT {
        for x in 0..VGA_WIDTH {
            write_cell(x, y, b' ', DEFAULT_COLOR);
        }
    }
    if framebuffer_console_active() {
        clear_fb_screen();
    }
    unsafe {
        CURSOR_X = 0;
        CURSOR_Y = 0;
    }
}

pub fn put_char(c: u8) {
    capture_push(c);
    unsafe {
        match c {
            b'\n' => {
                CURSOR_X = 0;
                CURSOR_Y += 1;
            }
            b'\r' => {
                CURSOR_X = 0;
            }
            b'\x08' => {
                if CURSOR_X > 0 {
                    CURSOR_X -= 1;
                    write_cell(CURSOR_X, CURSOR_Y, b' ', DEFAULT_COLOR);
                }
            }
            _ => {
                write_cell(CURSOR_X, CURSOR_Y, c, DEFAULT_COLOR);
                CURSOR_X += 1;
            }
        }

        if CURSOR_X >= VGA_WIDTH {
            CURSOR_X = 0;
            CURSOR_Y += 1;
        }

        if CURSOR_Y >= VGA_HEIGHT {
            scroll();
            CURSOR_Y = VGA_HEIGHT - 1;
        }
        // НЕ вызываем update_hw_cursor здесь — только через set_cursor_pos
    }
}

/// Обновить аппаратный VGA курсор
fn update_hw_cursor(x: usize, y: usize) {
    let pos = y * VGA_WIDTH + x;
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0x3D4u16, in("al") 0x0Fu8);
        core::arch::asm!("out dx, al", in("dx") 0x3D5u16, in("al") (pos & 0xFF) as u8);
        core::arch::asm!("out dx, al", in("dx") 0x3D4u16, in("al") 0x0Eu8);
        core::arch::asm!("out dx, al", in("dx") 0x3D5u16, in("al") ((pos >> 8) & 0xFF) as u8);
    }
}

pub fn print(s: &str) {
    serial_print(s);
    for b in s.bytes() {
        put_char(b);
    }
}

pub fn print_colored(s: &str, color: u8) {
    serial_print(s);
    unsafe {
        for b in s.bytes() {
            capture_push(b);
            match b {
                b'\n' => {
                    CURSOR_X = 0;
                    CURSOR_Y += 1;
                }
                b'\r' => {
                    CURSOR_X = 0;
                }
                b'\x08' => {
                // backspace
                if CURSOR_X > 0 {
                    CURSOR_X -= 1;
                    let erase_color = (color & 0xF0) | (DEFAULT_COLOR & 0x0F);
                    write_cell(CURSOR_X, CURSOR_Y, b' ', erase_color);
                }
            }
                _ => {
                    write_cell(CURSOR_X, CURSOR_Y, b, color);
                    CURSOR_X += 1;
                }
            }
            
            if CURSOR_X >= VGA_WIDTH {
                CURSOR_X = 0;
                CURSOR_Y += 1;
            }
            
            if CURSOR_Y >= VGA_HEIGHT {
                scroll();
                CURSOR_Y = VGA_HEIGHT - 1;
            }
        }
    }
}

fn scroll() {
    unsafe {
        for row in 1..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                let src = (row * VGA_WIDTH + col) * 2;
                let dst = ((row - 1) * VGA_WIDTH + col) * 2;
                *VGA_BUFFER.add(dst) = *VGA_BUFFER.add(src);
                *VGA_BUFFER.add(dst + 1) = *VGA_BUFFER.add(src + 1);
            }
        }
        for col in 0..VGA_WIDTH {
            let offset = ((VGA_HEIGHT - 1) * VGA_WIDTH + col) * 2;
            *VGA_BUFFER.add(offset) = b' ';
            *VGA_BUFFER.add(offset + 1) = DEFAULT_COLOR;
        }
    }
    if framebuffer_console_active() {
        scroll_fb();
    }
}

pub fn get_cursor_pos() -> (usize, usize) {
    unsafe { (CURSOR_X, CURSOR_Y) }
}

pub fn set_cursor_pos(x: usize, y: usize) {
    unsafe {
        CURSOR_X = if x >= VGA_WIDTH { VGA_WIDTH - 1 } else { x };
        CURSOR_Y = if y >= VGA_HEIGHT { VGA_HEIGHT - 1 } else { y };
        update_hw_cursor(CURSOR_X, CURSOR_Y);
    }
}

/// Синхронизировать аппаратный курсор с текущей программной позицией
pub fn sync_hw_cursor() {
    unsafe { update_hw_cursor(CURSOR_X, CURSOR_Y); }
}

// Читаем символ с клавиатуры через PS/2 драйвер
pub fn read_char() -> u8 {
    crate::drivers::ps2::read_char()
}

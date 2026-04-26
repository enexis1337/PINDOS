// VGA text mode 80x25, буфер по адресу 0xB8000

const VGA_BUFFER: *mut u8 = 0xB8000 as *mut u8;

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
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const DEFAULT_COLOR: u8 = 0x0F; // белый на чёрном

static mut CURSOR_X: usize = 0;
static mut CURSOR_Y: usize = 0;

pub fn clear_screen() {
    for i in 0..VGA_WIDTH * VGA_HEIGHT {
        unsafe {
            *VGA_BUFFER.add(i * 2) = b' ';
            *VGA_BUFFER.add(i * 2 + 1) = DEFAULT_COLOR;
        }
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
                // backspace
                if CURSOR_X > 0 {
                    CURSOR_X -= 1;
                    let offset = (CURSOR_Y * VGA_WIDTH + CURSOR_X) * 2;
                    *VGA_BUFFER.add(offset) = b' ';
                    *VGA_BUFFER.add(offset + 1) = DEFAULT_COLOR;
                }
            }
            _ => {
                let offset = (CURSOR_Y * VGA_WIDTH + CURSOR_X) * 2;
                *VGA_BUFFER.add(offset) = c;
                *VGA_BUFFER.add(offset + 1) = DEFAULT_COLOR;
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
                        let offset = (CURSOR_Y * VGA_WIDTH + CURSOR_X) * 2;
                        *VGA_BUFFER.add(offset) = b' ';
                        *VGA_BUFFER.add(offset + 1) = color;
                    }
                }
                _ => {
                    let offset = (CURSOR_Y * VGA_WIDTH + CURSOR_X) * 2;
                    *VGA_BUFFER.add(offset) = b;
                    *VGA_BUFFER.add(offset + 1) = color;
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
        // Сдвигаем все строки вверх
        for row in 1..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                let src = (row * VGA_WIDTH + col) * 2;
                let dst = ((row - 1) * VGA_WIDTH + col) * 2;
                *VGA_BUFFER.add(dst) = *VGA_BUFFER.add(src);
                *VGA_BUFFER.add(dst + 1) = *VGA_BUFFER.add(src + 1);
            }
        }
        // Очищаем последнюю строку
        for col in 0..VGA_WIDTH {
            let offset = ((VGA_HEIGHT - 1) * VGA_WIDTH + col) * 2;
            *VGA_BUFFER.add(offset) = b' ';
            *VGA_BUFFER.add(offset + 1) = DEFAULT_COLOR;
        }
    }
}

pub fn get_cursor_pos() -> (usize, usize) {
    unsafe { (CURSOR_X, CURSOR_Y) }
}

pub fn set_cursor_pos(x: usize, y: usize) {
    unsafe {
        CURSOR_X = if x >= VGA_WIDTH { VGA_WIDTH - 1 } else { x };
        CURSOR_Y = if y >= VGA_HEIGHT { VGA_HEIGHT - 1 } else { y };
    }
}

// Читаем символ с клавиатуры через PS/2 драйвер
pub fn read_char() -> u8 {
    crate::drivers::ps2::read_char()
}



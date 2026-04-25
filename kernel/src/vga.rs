// VGA text mode 80x25, буфер по адресу 0xB8000

const VGA_BUFFER: *mut u8 = 0xB8000 as *mut u8;
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
    for b in s.bytes() {
        put_char(b);
    }
}

pub fn print_colored(s: &str, color: u8) {
    unsafe {
        for b in s.bytes() {
            if b == b'\n' {
                CURSOR_X = 0;
                CURSOR_Y += 1;
                continue;
            }
            let offset = (CURSOR_Y * VGA_WIDTH + CURSOR_X) * 2;
            *VGA_BUFFER.add(offset) = b;
            *VGA_BUFFER.add(offset + 1) = color;
            CURSOR_X += 1;
            if CURSOR_X >= VGA_WIDTH {
                CURSOR_X = 0;
                CURSOR_Y += 1;
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

// Читаем символ с клавиатуры через порт 0x60
pub fn read_char() -> u8 {
    loop {
        let status: u8 = unsafe { x86_in(0x64) };
        if status & 1 != 0 {
            let scancode: u8 = unsafe { x86_in(0x60) };
            if let Some(c) = scancode_to_ascii(scancode) {
                return c;
            }
        }
    }
}

unsafe fn x86_in(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port);
    val
}

fn scancode_to_ascii(sc: u8) -> Option<u8> {
    // Простая таблица scancodes (без shift)
    const MAP: [u8; 58] = [
        0, 0, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
        b'-', b'=', b'\x08', b'\t', b'q', b'w', b'e', b'r', b't', b'y',
        b'u', b'i', b'o', b'p', b'[', b']', b'\n', 0, b'a', b's', b'd',
        b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\',
        b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0,
        b'*', 0, b' ',
    ];
    if sc < 58 && MAP[sc as usize] != 0 {
        Some(MAP[sc as usize])
    } else {
        None
    }
}

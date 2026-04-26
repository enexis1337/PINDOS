// PS/2 контроллер — клавиатура и мышь
// Порты: 0x60 (данные), 0x64 (статус/команды)

const DATA_PORT:   u16 = 0x60;
const STATUS_PORT: u16 = 0x64;
const CMD_PORT:    u16 = 0x64;

// Команды контроллера
const CMD_READ_CONFIG:    u8 = 0x20;
const CMD_WRITE_CONFIG:   u8 = 0x60;
const CMD_DISABLE_PORT2:  u8 = 0xA7;
const CMD_ENABLE_PORT2:   u8 = 0xA8;
const CMD_TEST_PORT2:     u8 = 0xA9;
const CMD_SELF_TEST:      u8 = 0xAA;
const CMD_TEST_PORT1:     u8 = 0xAB;
const CMD_DISABLE_PORT1:  u8 = 0xAD;
const CMD_ENABLE_PORT1:   u8 = 0xAE;
const CMD_WRITE_PORT2:    u8 = 0xD4;

unsafe fn out8(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

unsafe fn in8(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

fn wait_write() {
    let mut timeout = 100000u32;
    while timeout > 0 {
        let status = unsafe { in8(STATUS_PORT) };
        if status & 0x02 == 0 { return; }
        timeout -= 1;
    }
}

fn wait_read() -> bool {
    let mut timeout = 100000u32;
    while timeout > 0 {
        let status = unsafe { in8(STATUS_PORT) };
        if status & 0x01 != 0 { return true; }
        timeout -= 1;
    }
    false
}

fn send_cmd(cmd: u8) {
    wait_write();
    unsafe { out8(CMD_PORT, cmd); }
}

fn send_data(data: u8) {
    wait_write();
    unsafe { out8(DATA_PORT, data); }
}

fn read_data() -> Option<u8> {
    if wait_read() {
        Some(unsafe { in8(DATA_PORT) })
    } else {
        None
    }
}

// ── Инициализация ─────────────────────────────────────────────────────────

pub fn init() {
    // Отключаем оба порта
    send_cmd(CMD_DISABLE_PORT1);
    send_cmd(CMD_DISABLE_PORT2);

    // Очищаем буфер
    unsafe { let _ = in8(DATA_PORT); }

    // Читаем конфигурацию
    send_cmd(CMD_READ_CONFIG);
    let config = read_data().unwrap_or(0);

    // Включаем прерывания для обоих портов, отключаем трансляцию
    let new_config = (config | 0x03) & !0x40;
    send_cmd(CMD_WRITE_CONFIG);
    send_data(new_config);

    // Self-test
    send_cmd(CMD_SELF_TEST);
    let _ = read_data(); // 0x55 = OK

    // Включаем порты
    send_cmd(CMD_ENABLE_PORT1);
    send_cmd(CMD_ENABLE_PORT2);

    // Инициализируем клавиатуру
    init_keyboard();

    // Инициализируем мышь
    init_mouse();
}

// ── Клавиатура ────────────────────────────────────────────────────────────

fn init_keyboard() {
    // Reset
    send_data(0xFF);
    let _ = read_data(); // ACK
    let _ = read_data(); // 0xAA = self-test OK

    // Включаем сканкоды set 1 (по умолчанию)
    send_data(0xF0);
    let _ = read_data();
    send_data(0x01);
    let _ = read_data();

    // Включаем клавиатуру
    send_data(0xF4);
    let _ = read_data();
}

// Состояние модификаторов
static mut SHIFT:   bool = false;
static mut CTRL:    bool = false;
static mut ALT:     bool = false;
static mut CAPS:    bool = false;
static mut EXTENDED: bool = false;

// Буфер для escape sequences
static mut ESC_BUFFER: [u8; 4] = [0; 4];
static mut ESC_POS: usize = 0;
static mut ESC_LEN: usize = 0;

/// Читает символ с клавиатуры (блокирующий)
pub fn read_char() -> u8 {
    unsafe {
        // Если есть символы в escape буфере, возвращаем их
        if ESC_POS < ESC_LEN {
            let c = ESC_BUFFER[ESC_POS];
            ESC_POS += 1;
            if ESC_POS >= ESC_LEN {
                ESC_POS = 0;
                ESC_LEN = 0;
            }
            return c;
        }
    }

    loop {
        if let Some(sc) = poll_scancode() {
            if let Some(c) = scancode_to_char_enhanced(sc) {
                return c;
            }
        }
    }
}

/// Неблокирующий опрос — возвращает символ если есть
pub fn poll_char() -> Option<u8> {
    unsafe {
        // Если есть символы в escape буфере, возвращаем их
        if ESC_POS < ESC_LEN {
            let c = ESC_BUFFER[ESC_POS];
            ESC_POS += 1;
            if ESC_POS >= ESC_LEN {
                ESC_POS = 0;
                ESC_LEN = 0;
            }
            return Some(c);
        }
    }

    poll_scancode().and_then(scancode_to_char_enhanced)
}

/// Читает сырой scancode (только от клавиатуры)
pub fn poll_scancode() -> Option<u8> {
    let status = unsafe { in8(STATUS_PORT) };
    if status & 0x01 == 0 { return None; }
    
    // Бит 5 = данные от мыши - если установлен, обрабатываем как мышь
    if status & 0x20 != 0 { 
        // Данные от мыши - обрабатываем отдельно
        poll_mouse();
        return None; 
    }
    
    // Данные от клавиатуры
    Some(unsafe { in8(DATA_PORT) })
}

fn scancode_to_char_enhanced(sc: u8) -> Option<u8> {
    unsafe {
        // Extended prefix
        if sc == 0xE0 { EXTENDED = true; return None; }

        // Key release (бит 7 = 1)
        if sc & 0x80 != 0 {
            let base = sc & 0x7F;
            match base {
                0x2A | 0x36 => SHIFT = false,
                0x1D => CTRL = false,
                0x38 => ALT = false,
                _ => {}
            }
            EXTENDED = false;
            return None;
        }

        // Key press
        let result = match sc {
            0x2A | 0x36 => { SHIFT = true; None }
            0x1D => { CTRL = true; None }
            0x38 => { ALT = true; None }
            0x3A => { CAPS = !CAPS; None }

            // Специальные клавиши
            0x1C => Some(b'\n'),  // Enter
            0x0E => Some(b'\x08'), // Backspace
            0x0F => Some(b'\t'),   // Tab
            0x01 => Some(0x1B),    // Esc

            // Стрелки (extended) — генерируем полные escape sequences
            _ if EXTENDED => {
                EXTENDED = false;
                
                match sc {
                    0x48 => { // Up
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'A';
                        ESC_LEN = 3; ESC_POS = 1;
                        Some(0x1B)
                    }
                    0x50 => { // Down
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'B';
                        ESC_LEN = 3; ESC_POS = 1;
                        Some(0x1B)
                    }
                    0x4D => { // Right
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'C';
                        ESC_LEN = 3; ESC_POS = 1;
                        Some(0x1B)
                    }
                    0x4B => { // Left
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'D';
                        ESC_LEN = 3; ESC_POS = 1;
                        Some(0x1B)
                    }
                    0x47 => { // Home
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'H';
                        ESC_LEN = 3; ESC_POS = 1;
                        Some(0x1B)
                    }
                    0x4F => { // End
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'F';
                        ESC_LEN = 3; ESC_POS = 1;
                        Some(0x1B)
                    }
                    0x53 => { // Delete
                        ESC_BUFFER[0] = 0x1B; ESC_BUFFER[1] = b'['; ESC_BUFFER[2] = b'3'; ESC_BUFFER[3] = b'~';
                        ESC_LEN = 4; ESC_POS = 1;
                        Some(0x1B)
                    }
                    _ => None,
                }
            }

            // Обычные символы
            _ => {
                if sc as usize >= SCANCODE_NORMAL.len() {
                    None
                } else {
                    let c = if SHIFT || CAPS {
                        SCANCODE_SHIFT[sc as usize]
                    } else {
                        SCANCODE_NORMAL[sc as usize]
                    };
                    if c == 0 { None } else {
                        // Ctrl+буква
                        if CTRL && c >= b'a' && c <= b'z' {
                            Some(c - b'a' + 1)
                        } else {
                            Some(c)
                        }
                    }
                }
            }
        };

        result
    }
}

// Таблицы scancodes (Set 1)
static SCANCODE_NORMAL: &[u8] = &[
//  0     1     2     3     4     5     6     7     8     9     A     B     C     D     E     F
    0,    0x1B, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 0,    b'\t',
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n',0,   b'a', b's',
    b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'',b'`', 0,    b'\\',b'z', b'x', b'c', b'v',
    b'b', b'n', b'm', b',', b'.', b'/', 0,    b'*', 0,    b' ', 0,    0,    0,    0,    0,    0,
    0,    0,    0,    0,    0,    0,    0,    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1',
    b'2', b'3', b'0', b'.', 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,
];

static SCANCODE_SHIFT: &[u8] = &[
//  0     1     2     3     4     5     6     7     8     9     A     B     C     D     E     F
    0,    0x1B, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 0,    b'\t',
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n',0,   b'A', b'S',
    b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~', 0,    b'|', b'Z', b'X', b'C', b'V',
    b'B', b'N', b'M', b'<', b'>', b'?', 0,    b'*', 0,    b' ', 0,    0,    0,    0,    0,    0,
    0,    0,    0,    0,    0,    0,    0,    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1',
    b'2', b'3', b'0', b'.', 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,
];

// ── Мышь ──────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Default)]
pub struct MouseState {
    pub x:       i32,
    pub y:       i32,
    pub buttons: u8,  // бит 0 = левая, бит 1 = правая, бит 2 = средняя
    pub dx:      i8,
    pub dy:      i8,
}

static mut MOUSE: MouseState = MouseState { x: 400, y: 300, buttons: 0, dx: 0, dy: 0 };
static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_PACKET: [u8; 3] = [0u8; 3];

pub const SCREEN_W: i32 = 1024;
pub const SCREEN_H: i32 = 768;

fn init_mouse() {
    // Включаем вспомогательное устройство
    send_cmd(CMD_ENABLE_PORT2);

    // Reset мыши
    send_cmd(CMD_WRITE_PORT2);
    send_data(0xFF);
    let _ = read_data(); // ACK
    let _ = read_data(); // 0xAA
    let _ = read_data(); // 0x00 (device ID)

    // Включаем передачу данных
    send_cmd(CMD_WRITE_PORT2);
    send_data(0xF4);
    let _ = read_data(); // ACK
}

/// Опрашивает мышь, обновляет состояние (вызывается автоматически из poll_scancode)
pub fn poll_mouse() -> bool {
    // Эта функция вызывается только когда мы знаем, что данные от мыши
    let byte = unsafe { in8(DATA_PORT) };

    unsafe {
        MOUSE_PACKET[MOUSE_CYCLE as usize] = byte;
        MOUSE_CYCLE += 1;

        if MOUSE_CYCLE == 3 {
            MOUSE_CYCLE = 0;
            let flags = MOUSE_PACKET[0];
            let dx = MOUSE_PACKET[1] as i8;
            let dy = MOUSE_PACKET[2] as i8;

            // Проверяем overflow биты и валидность пакета
            if flags & 0x08 != 0 && flags & 0xC0 == 0 {
                MOUSE.dx = dx;
                MOUSE.dy = -dy; // Y инвертирован
                MOUSE.x = (MOUSE.x + dx as i32).max(0).min(SCREEN_W - 1);
                MOUSE.y = (MOUSE.y - dy as i32).max(0).min(SCREEN_H - 1);
                MOUSE.buttons = flags & 0x07;
                return true;
            }
        }
    }
    false
}

/// Ручной опрос мыши (если нужно проверить состояние отдельно)
pub fn check_mouse() -> bool {
    let status = unsafe { in8(STATUS_PORT) };
    if status & 0x01 == 0 { return false; }
    if status & 0x20 == 0 { return false; } // не от мыши
    
    poll_mouse()
}

pub fn get_mouse() -> MouseState {
    unsafe { MOUSE }
}

/// Рисует курсор мыши в framebuffer
pub fn draw_cursor(x: i32, y: i32) {
    use crate::drivers::vesa;
    // Простой курсор — стрелка 8x8
    const CURSOR: &[u8] = &[
        0b11000000,
        0b11100000,
        0b11110000,
        0b11111000,
        0b11111100,
        0b11110000,
        0b11011000,
        0b10001100,
    ];
    for row in 0..8i32 {
        let byte = CURSOR[row as usize];
        for col in 0..8i32 {
            if byte & (0x80 >> col) != 0 {
                vesa::put_pixel((x + col) as u32, (y + row) as u32, vesa::WHITE);
            }
        }
    }
}
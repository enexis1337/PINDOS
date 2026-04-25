// Эмуляция DOS INT 21h прерываний
// Вызывается из обработчика исключения #GP (General Protection Fault)
// когда v86 код выполняет INT

use crate::vga;

/// Результат обработки INT 21h
pub enum Int21Result {
    Continue,   // продолжить выполнение
    Terminate,  // завершить программу (AH=4C или INT 20h)
}

/// Обрабатывает INT 21h
/// Принимает регистры из v86 стека
pub fn handle(regs: &mut V86Regs) -> Int21Result {
    let ah = (regs.eax >> 8) as u8;
    let al = regs.eax as u8;

    match ah {
        // AH=00 — завершение программы
        0x00 => Int21Result::Terminate,

        // AH=01 — читать символ с эхом
        0x01 => {
            let c = crate::vga::read_char();
            vga::put_char(c);
            regs.eax = (regs.eax & 0xFFFFFF00) | c as u32;
            Int21Result::Continue
        }

        // AH=02 — вывод символа (DL)
        0x02 => {
            let c = regs.edx as u8;
            vga::put_char(c);
            Int21Result::Continue
        }

        // AH=06 — прямой ввод/вывод консоли
        0x06 => {
            let dl = regs.edx as u8;
            if dl != 0xFF {
                vga::put_char(dl);
            } else {
                // Ввод без ожидания — возвращаем 0 (нет символа)
                regs.eax = regs.eax & 0xFFFFFF00;
            }
            Int21Result::Continue
        }

        // AH=07 — читать символ без эха
        0x07 => {
            let c = crate::vga::read_char();
            regs.eax = (regs.eax & 0xFFFFFF00) | c as u32;
            Int21Result::Continue
        }

        // AH=08 — читать символ без эха (аналог 07)
        0x08 => {
            let c = crate::vga::read_char();
            regs.eax = (regs.eax & 0xFFFFFF00) | c as u32;
            Int21Result::Continue
        }

        // AH=09 — вывод строки (DS:DX, завершается '$')
        0x09 => {
            let seg = regs.ds as u32;
            let off = regs.edx & 0xFFFF;
            let addr = (seg << 4) + off;
            unsafe {
                let mut ptr = addr as *const u8;
                loop {
                    let c = *ptr;
                    if c == b'$' { break; }
                    vga::put_char(c);
                    ptr = ptr.add(1);
                }
            }
            Int21Result::Continue
        }

        // AH=0A — буферизованный ввод строки
        0x0A => {
            let seg = regs.ds as u32;
            let off = regs.edx & 0xFFFF;
            let addr = (seg << 4) + off;
            unsafe {
                let buf = addr as *mut u8;
                let max = *buf as usize; // первый байт — максимум символов
                let mut count = 0usize;
                loop {
                    let c = crate::vga::read_char();
                    if c == b'\n' || c == b'\r' {
                        vga::put_char(b'\n');
                        break;
                    }
                    if c == b'\x08' && count > 0 {
                        count -= 1;
                        vga::put_char(b'\x08');
                        continue;
                    }
                    if count < max - 1 {
                        *buf.add(2 + count) = c;
                        count += 1;
                        vga::put_char(c);
                    }
                }
                *buf.add(1) = count as u8; // второй байт — реальная длина
                *buf.add(2 + count) = b'\r';
            }
            Int21Result::Continue
        }

        // AH=0B — проверить статус ввода (всегда "есть символ")
        0x0B => {
            regs.eax = (regs.eax & 0xFFFFFF00) | 0xFF;
            Int21Result::Continue
        }

        // AH=0E — выбор диска (игнорируем, возвращаем 1 диск)
        0x0E => {
            regs.eax = (regs.eax & 0xFFFFFF00) | 0x01;
            Int21Result::Continue
        }

        // AH=19 — получить текущий диск (всегда C: = 2)
        0x19 => {
            regs.eax = (regs.eax & 0xFFFFFF00) | 0x02;
            Int21Result::Continue
        }

        // AH=25 — установить вектор прерывания (игнорируем)
        0x25 => Int21Result::Continue,

        // AH=2A — получить дату (01.01.2025)
        0x2A => {
            regs.ecx = (regs.ecx & 0xFFFF0000) | 2025; // год
            regs.edx = (regs.edx & 0xFFFF0000) | (1 << 8) | 1; // DH=месяц, DL=день
            regs.eax = regs.eax & 0xFFFFFF00; // AL=0 (понедельник)
            Int21Result::Continue
        }

        // AH=2C — получить время (00:00:00)
        0x2C => {
            regs.ecx = 0;
            regs.edx = 0;
            Int21Result::Continue
        }

        // AH=30 — получить версию DOS (возвращаем 5.0)
        0x30 => {
            regs.eax = (regs.eax & 0xFFFF0000) | 0x0005; // AL=5, AH=0
            regs.ebx = regs.ebx & 0xFFFF0000;
            regs.ecx = regs.ecx & 0xFFFF0000;
            Int21Result::Continue
        }

        // AH=35 — получить вектор прерывания (возвращаем 0)
        0x35 => {
            regs.ebx = 0;
            regs.es = 0;
            Int21Result::Continue
        }

        // AH=3C — создать файл
        0x3C => {
            let seg = regs.ds as u32;
            let off = regs.edx & 0xFFFF;
            let name = read_cstring((seg << 4) + off);
            if fs_create_from_dos(name) {
                regs.eax = (regs.eax & 0xFFFF0000) | 5; // handle = 5
                clear_carry(regs);
            } else {
                regs.eax = (regs.eax & 0xFFFF0000) | 0x0003; // path not found
                set_carry(regs);
            }
            Int21Result::Continue
        }

        // AH=4C — завершить программу с кодом возврата
        0x4C => Int21Result::Terminate,

        // Неизвестная функция — игнорируем
        _ => {
            // Можно раскомментировать для отладки:
            // vga::print("[INT21 unknown AH=");
            // print_hex(ah);
            // vga::print("]\n");
            Int21Result::Continue
        }
    }
}

/// Обрабатывает INT 20h (terminate)
pub fn handle_int20() -> Int21Result {
    Int21Result::Terminate
}

fn read_cstring(addr: u32) -> &'static str {
    unsafe {
        let ptr = addr as *const u8;
        let mut len = 0;
        while *ptr.add(len) != 0 && len < 64 { len += 1; }
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).unwrap_or("")
    }
}

fn fs_create_from_dos(name: &str) -> bool {
    if crate::fs::get(name).is_some() {
        true // уже существует
    } else {
        crate::fs::create(name, "")
    }
}

fn set_carry(regs: &mut V86Regs) {
    regs.eflags |= 1; // CF=1
}

fn clear_carry(regs: &mut V86Regs) {
    regs.eflags &= !1; // CF=0
}

/// Регистры виртуального 8086 процессора
/// Соответствуют структуре на стеке при #GP из v86
#[repr(C)]
pub struct V86Regs {
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    // Сохранённые из стека прерывания
    pub eip: u32,
    pub cs:  u32,
    pub eflags: u32,
    pub user_esp: u32,
    pub ss:  u32,
    pub es:  u32,
    pub ds:  u32,
    pub fs:  u32,
    pub gs:  u32,
}

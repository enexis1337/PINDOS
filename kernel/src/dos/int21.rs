// DOS INT 21h + INT 10h + INT 16h эмуляция — расширенная версия

use crate::vga;
use crate::fs;

pub enum Int21Result { Continue, Terminate }

// ── INT 21h ───────────────────────────────────────────────────────────────

pub fn handle(regs: &mut V86Regs) -> Int21Result {
    let ah = (regs.eax >> 8) as u8;

    match ah {
        0x00 => Int21Result::Terminate,

        // Ввод/вывод символов
        0x01 => { let c = read_char_echo(); regs.set_al(c); Int21Result::Continue }
        0x02 => { vga::put_char(regs.dl()); Int21Result::Continue }
        0x03 => { regs.set_al(0); Int21Result::Continue } // AUX input
        0x04 => { Int21Result::Continue } // AUX output
        0x05 => { Int21Result::Continue } // printer output
        0x06 => {
            let dl = regs.dl();
            if dl == 0xFF {
                // non-blocking input — возвращаем 0 (нет символа), ZF=1
                regs.set_al(0);
                regs.eflags |= 0x40; // ZF
            } else {
                vga::put_char(dl);
                regs.eflags &= !0x40;
            }
            Int21Result::Continue
        }
        0x07 => { regs.set_al(vga::read_char()); Int21Result::Continue }
        0x08 => { regs.set_al(vga::read_char()); Int21Result::Continue }

        // Вывод строки DS:DX до '$'
        0x09 => {
            let addr = seg_off(regs.ds, regs.edx & 0xFFFF);
            unsafe {
                let mut p = addr as *const u8;
                loop {
                    let c = *p;
                    if c == b'$' { break; }
                    vga::put_char(c);
                    p = p.add(1);
                }
            }
            regs.set_al(b'$');
            Int21Result::Continue
        }

        // Буферизованный ввод
        0x0A => {
            let addr = seg_off(regs.ds, regs.edx & 0xFFFF);
            unsafe {
                let buf = addr as *mut u8;
                let max = *buf as usize;
                let mut count = 0usize;
                loop {
                    let c = vga::read_char();
                    if c == b'\r' || c == b'\n' { vga::put_char(b'\r'); vga::put_char(b'\n'); break; }
                    if c == 0x08 && count > 0 { count -= 1; vga::put_char(0x08); vga::put_char(b' '); vga::put_char(0x08); continue; }
                    if count < max.saturating_sub(1) {
                        *buf.add(2 + count) = c;
                        count += 1;
                        vga::put_char(c);
                    }
                }
                *buf.add(1) = count as u8;
                *buf.add(2 + count) = b'\r';
            }
            Int21Result::Continue
        }

        0x0B => { regs.set_al(0xFF); Int21Result::Continue } // input status
        0x0C => { regs.set_al(0); Int21Result::Continue }    // flush + input

        // Диск
        0x0D => Int21Result::Continue, // disk reset
        0x0E => { regs.set_al(26); Int21Result::Continue } // select drive, return count
        0x19 => { regs.set_al(2); Int21Result::Continue }  // current drive = C:
        0x1A => Int21Result::Continue, // set DTA
        0x1B | 0x1C => { regs.set_al(0xFF); Int21Result::Continue } // alloc info

        // Дата/время
        0x2A => {
            regs.ecx = (regs.ecx & 0xFFFF0000) | 2025;
            regs.edx = (regs.edx & 0xFFFF0000) | (1 << 8) | 1;
            regs.set_al(1); // понедельник
            Int21Result::Continue
        }
        0x2B => { regs.set_al(0); Int21Result::Continue } // set date
        0x2C => {
            regs.ecx = (regs.ecx & 0xFFFF0000) | (12 << 8) | 0; // 12:00
            regs.edx = 0;
            Int21Result::Continue
        }
        0x2D => { regs.set_al(0); Int21Result::Continue } // set time

        // Версия DOS
        0x30 => {
            regs.eax = (regs.eax & 0xFFFF0000) | 0x0005; // DOS 5.0
            regs.ebx = regs.ebx & 0xFFFF0000;
            regs.ecx = regs.ecx & 0xFFFF0000;
            Int21Result::Continue
        }

        // Прерывания
        0x25 => Int21Result::Continue, // set interrupt vector
        0x35 => { regs.ebx = 0; regs.es = 0; Int21Result::Continue } // get interrupt vector

        // Память
        0x48 => { // allocate memory
            regs.eax = (regs.eax & 0xFFFF0000) | 0x3000; // возвращаем сегмент
            clear_carry(regs);
            Int21Result::Continue
        }
        0x49 => { clear_carry(regs); Int21Result::Continue } // free memory
        0x4A => { clear_carry(regs); Int21Result::Continue } // resize memory

        // Файловые операции
        0x3C => { // create file
            let name = dos_cstring(regs.ds, regs.edx & 0xFFFF);
            if fs::get(name).is_some() || fs::create(name, "") {
                regs.eax = (regs.eax & 0xFFFF0000) | 5;
                clear_carry(regs);
            } else { regs.eax = (regs.eax & 0xFFFF0000) | 3; set_carry(regs); }
            Int21Result::Continue
        }
        0x3D => { // open file
            let name = dos_cstring(regs.ds, regs.edx & 0xFFFF);
            if fs::get(name).is_some() {
                regs.eax = (regs.eax & 0xFFFF0000) | 5;
                clear_carry(regs);
            } else { regs.eax = (regs.eax & 0xFFFF0000) | 2; set_carry(regs); }
            Int21Result::Continue
        }
        0x3E => { clear_carry(regs); Int21Result::Continue } // close file
        0x3F => { // read file — упрощённо читаем с stdin
            let len = regs.ecx & 0xFFFF;
            let addr = seg_off(regs.ds, regs.edx & 0xFFFF);
            let mut n = 0u32;
            unsafe {
                while n < len {
                    let c = vga::read_char();
                    *(addr as *mut u8).add(n as usize) = c;
                    n += 1;
                    if c == b'\r' { break; }
                }
            }
            regs.eax = (regs.eax & 0xFFFF0000) | n;
            clear_carry(regs);
            Int21Result::Continue
        }
        0x40 => { // write file
            let fd = regs.ebx & 0xFFFF;
            let len = (regs.ecx & 0xFFFF) as usize;
            let addr = seg_off(regs.ds, regs.edx & 0xFFFF);
            if fd <= 2 {
                unsafe {
                    for i in 0..len { vga::put_char(*(addr as *const u8).add(i)); }
                }
            }
            regs.eax = (regs.eax & 0xFFFF0000) | len as u32;
            clear_carry(regs);
            Int21Result::Continue
        }
        0x41 => { // delete file
            let name = dos_cstring(regs.ds, regs.edx & 0xFFFF);
            if fs::delete(name) { clear_carry(regs); }
            else { regs.eax = (regs.eax & 0xFFFF0000) | 2; set_carry(regs); }
            Int21Result::Continue
        }
        0x43 => { // get/set file attributes
            clear_carry(regs);
            regs.ecx = regs.ecx & 0xFFFF0000; // normal file
            Int21Result::Continue
        }
        0x45 | 0x46 => { clear_carry(regs); Int21Result::Continue } // dup handle
        0x47 => { // get current directory
            let addr = seg_off(regs.ds, regs.esi & 0xFFFF);
            let cwd = fs::cwd();
            unsafe {
                let b = cwd.as_bytes();
                for (i, &c) in b.iter().enumerate() { *(addr as *mut u8).add(i) = c; }
                *(addr as *mut u8).add(b.len()) = 0;
            }
            clear_carry(regs);
            Int21Result::Continue
        }

        // Exec / terminate
        0x4B => { // EXEC — не поддерживаем
            regs.eax = (regs.eax & 0xFFFF0000) | 8;
            set_carry(regs);
            Int21Result::Continue
        }
        0x4C => Int21Result::Terminate,
        0x4D => { regs.eax = regs.eax & 0xFFFF0000; Int21Result::Continue } // get exit code

        // Окружение
        0x62 => { // get PSP segment
            regs.ebx = (regs.ebx & 0xFFFF0000) | (crate::dos::loader::DOS_SEGMENT & 0xFFFF);
            Int21Result::Continue
        }

        // Extended open
        0x6C => { clear_carry(regs); regs.eax = (regs.eax & 0xFFFF0000) | 5; Int21Result::Continue }

        _ => Int21Result::Continue,
    }
}

// ── INT 10h — видео BIOS ──────────────────────────────────────────────────

pub fn handle_int10(regs: &mut V86Regs) {
    let ah = (regs.eax >> 8) as u8;
    let al = regs.eax as u8;

    match ah {
        0x00 => { // set video mode
            if al == 0x03 || al == 0x02 { vga::clear_screen(); }
        }
        0x01 => {} // set cursor shape
        0x02 => {} // set cursor position (DH=row, DL=col) — TODO
        0x03 => { // get cursor position
            regs.edx = regs.edx & 0xFFFF0000; // row=0, col=0
            regs.ecx = regs.ecx & 0xFFFF0000;
        }
        0x06 => { // scroll up
            vga::clear_screen();
        }
        0x07 => { // scroll down
            vga::clear_screen();
        }
        0x08 => { // read char/attr at cursor
            regs.eax = (regs.eax & 0xFFFF0000) | 0x0720; // space, white on black
        }
        0x09 | 0x0A => { // write char at cursor
            let c = al;
            let count = regs.ecx & 0xFFFF;
            for _ in 0..count { vga::put_char(c); }
        }
        0x0B => {} // set color palette
        0x0C => {} // write pixel
        0x0D => { regs.eax = regs.eax & 0xFFFF0000; } // read pixel
        0x0E => { // teletype output
            vga::put_char(al);
        }
        0x0F => { // get video mode
            regs.eax = (regs.eax & 0xFFFF0000) | 0x5003; // 80 cols, mode 3
        }
        0x10 => {} // set palette
        0x11 => {} // char generator
        0x12 => { // alternate select
            regs.ebx = (regs.ebx & 0xFFFF0000) | 0x0003; // 256KB, EGA
        }
        0x13 => { // write string
            let len = regs.ecx & 0xFFFF;
            let addr = seg_off(regs.es, regs.ebp & 0xFFFF);
            unsafe {
                for i in 0..len as usize { vga::put_char(*(addr as *const u8).add(i)); }
            }
        }
        0x1A => { // get/set display combination
            regs.eax = (regs.eax & 0xFFFF0000) | 0x001A;
            regs.ebx = (regs.ebx & 0xFFFF0000) | 0x0008; // VGA color
        }
        _ => {}
    }
}

// ── INT 16h — клавиатура BIOS ─────────────────────────────────────────────

pub fn handle_int16(regs: &mut V86Regs) {
    let ah = (regs.eax >> 8) as u8;
    match ah {
        0x00 | 0x10 => { // read key
            let c = vga::read_char();
            regs.eax = (regs.eax & 0xFFFF0000) | (c as u32);
        }
        0x01 | 0x11 => { // check key (non-blocking) — говорим что нет символа
            regs.eflags |= 0x40; // ZF=1 — нет символа
        }
        0x02 | 0x12 => { // get shift status
            regs.eax = regs.eax & 0xFFFF0000;
        }
        _ => {}
    }
}

// ── INT 33h — мышь ────────────────────────────────────────────────────────

pub fn handle_int33(regs: &mut V86Regs) {
    let ax = regs.eax & 0xFFFF;
    match ax {
        0x0000 => { regs.eax = (regs.eax & 0xFFFF0000) | 0xFFFF; regs.ebx = (regs.ebx & 0xFFFF0000) | 2; } // init, 2 buttons
        _ => { regs.eax = regs.eax & 0xFFFF0000; }
    }
}

// ── Вспомогательные ──────────────────────────────────────────────────────

fn seg_off(seg: u32, off: u32) -> u32 {
    ((seg & 0xFFFF) << 4) + (off & 0xFFFF)
}

fn dos_cstring(seg: u32, off: u32) -> &'static str {
    let addr = seg_off(seg, off);
    unsafe {
        let ptr = addr as *const u8;
        let mut len = 0;
        while *ptr.add(len) != 0 && len < 128 { len += 1; }
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).unwrap_or("")
    }
}

fn read_char_echo() -> u8 {
    let c = vga::read_char();
    vga::put_char(c);
    c
}

fn set_carry(regs: &mut V86Regs)   { regs.eflags |= 1; }
fn clear_carry(regs: &mut V86Regs) { regs.eflags &= !1; }

// ── Регистры v86 ─────────────────────────────────────────────────────────

#[repr(C)]
pub struct V86Regs {
    pub edi: u32, pub esi: u32, pub ebp: u32, pub esp: u32,
    pub ebx: u32, pub edx: u32, pub ecx: u32, pub eax: u32,
    pub eip: u32, pub cs:  u32, pub eflags: u32,
    pub user_esp: u32, pub ss: u32, pub es: u32, pub ds: u32,
    pub fs: u32,  pub gs: u32,
}

impl V86Regs {
    pub fn dl(&self) -> u8 { self.edx as u8 }
    pub fn set_al(&mut self, v: u8) { self.eax = (self.eax & 0xFFFFFF00) | v as u32; }
}

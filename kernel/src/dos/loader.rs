// DOS загрузчик — поддержка .COM и .EXE (MZ формат)

use crate::fs;
use crate::vga;

pub const DOS_SEGMENT: u32 = 0x2000;
pub const DOS_LINEAR:  u32 = DOS_SEGMENT << 4; // 0x20000
pub const PSP_SIZE:    u32 = 0x100;
pub const COM_MAX:     usize = 0xFF00;

pub struct DosProgram {
    pub cs:     u32,  // стартовый сегмент кода
    pub ip:     u32,  // стартовый IP
    pub ss:     u32,  // стартовый сегмент стека
    pub sp:     u32,  // стартовый SP
    pub kind:   DosKind,
}

pub enum DosKind { Com, Exe }

// ── .COM ──────────────────────────────────────────────────────────────────

pub fn load_com(filename: &str) -> Option<DosProgram> {
    let file = fs::get(filename)?;
    let content = file.content_str().as_bytes();
    if content.len() > COM_MAX {
        vga::print("Error: .COM too large\n");
        return None;
    }
    build_psp(DOS_LINEAR as *mut u8, content.len(), filename);
    let dst = (DOS_LINEAR + PSP_SIZE) as *mut u8;
    unsafe {
        for (i, &b) in content.iter().enumerate() { *dst.add(i) = b; }
    }
    Some(DosProgram {
        cs: DOS_SEGMENT, ip: PSP_SIZE,
        ss: DOS_SEGMENT, sp: 0xFFFE,
        kind: DosKind::Com,
    })
}

// ── .EXE (MZ) ─────────────────────────────────────────────────────────────

#[repr(C, packed)]
struct MzHeader {
    signature:   u16, // 0x5A4D 'MZ'
    last_page:   u16, // байт в последней странице
    pages:       u16, // страниц в файле (512 байт)
    reloc_count: u16, // количество записей релокации
    header_size: u16, // размер заголовка в параграфах
    min_alloc:   u16, // минимум параграфов после BSS
    max_alloc:   u16, // максимум параграфов
    init_ss:     u16, // начальный SS (относительно load segment)
    init_sp:     u16, // начальный SP
    checksum:    u16,
    init_ip:     u16, // начальный IP
    init_cs:     u16, // начальный CS (относительно load segment)
    reloc_off:   u16, // offset таблицы релокации
    overlay:     u16,
}

pub fn load_exe(filename: &str) -> Option<DosProgram> {
    let file = fs::get(filename)?;
    let data = file.content_str().as_bytes();

    if data.len() < core::mem::size_of::<MzHeader>() {
        vga::print("Error: .EXE too small\n");
        return None;
    }

    let hdr = unsafe { &*(data.as_ptr() as *const MzHeader) };

    if hdr.signature != 0x5A4D {
        vga::print("Error: not an MZ executable\n");
        return None;
    }

    // Размер заголовка в байтах
    let header_bytes = hdr.header_size as usize * 16;
    // Размер образа
    let image_size = if hdr.last_page > 0 {
        (hdr.pages as usize - 1) * 512 + hdr.last_page as usize
    } else {
        hdr.pages as usize * 512
    };
    let load_size = image_size.saturating_sub(header_bytes);

    // Сегмент загрузки (после PSP)
    let load_seg: u32 = DOS_SEGMENT + (PSP_SIZE / 16);
    let load_addr: u32 = load_seg << 4;

    // Строим PSP
    build_psp(DOS_LINEAR as *mut u8, load_size, filename);

    // Копируем образ
    let src = &data[header_bytes..header_bytes + load_size.min(data.len() - header_bytes)];
    unsafe {
        let dst = load_addr as *mut u8;
        for (i, &b) in src.iter().enumerate() { *dst.add(i) = b; }
    }

    // Применяем релокации
    let reloc_count = hdr.reloc_count as usize;
    let reloc_off   = hdr.reloc_off as usize;
    for i in 0..reloc_count {
        let off = reloc_off + i * 4;
        if off + 4 > data.len() { break; }
        let rel_off = u16::from_le_bytes([data[off], data[off+1]]) as u32;
        let rel_seg = u16::from_le_bytes([data[off+2], data[off+3]]) as u32;
        let patch_addr = load_addr + rel_seg * 16 + rel_off;
        unsafe {
            let ptr = patch_addr as *mut u16;
            *ptr = (*ptr).wrapping_add(load_seg as u16);
        }
    }

    Some(DosProgram {
        cs: load_seg + hdr.init_cs as u32,
        ip: hdr.init_ip as u32,
        ss: load_seg + hdr.init_ss as u32,
        sp: hdr.init_sp as u32,
        kind: DosKind::Exe,
    })
}

// ── Автоопределение формата ───────────────────────────────────────────────

pub fn load_auto(filename: &str) -> Option<DosProgram> {
    let file = fs::get(filename)?;
    let data = file.content_str().as_bytes();
    if data.len() >= 2 && data[0] == b'M' && data[1] == b'Z' {
        load_exe(filename)
    } else {
        load_com(filename)
    }
}

// ── PSP ───────────────────────────────────────────────────────────────────

fn build_psp(base: *mut u8, prog_size: usize, filename: &str) {
    unsafe {
        for i in 0..256 { *base.add(i) = 0; }
        // INT 20h по смещению 0x00
        *base.add(0x00) = 0xCD;
        *base.add(0x01) = 0x20;
        // Размер памяти
        *base.add(0x02) = 0xFF;
        *base.add(0x03) = 0x9F;
        // Конец программы
        let end = (PSP_SIZE as usize + prog_size) as u16;
        *base.add(0x06) = (end & 0xFF) as u8;
        *base.add(0x07) = (end >> 8) as u8;
        // INT 21h / RETF по смещению 0x50
        *base.add(0x50) = 0xCD;
        *base.add(0x51) = 0x21;
        *base.add(0x52) = 0xCB; // RETF
        // Имя файла в FCB1 (0x5C)
        let name = filename.as_bytes();
        let n = name.len().min(8);
        for i in 0..n { *base.add(0x5C + i) = name[i].to_ascii_uppercase(); }
    }
}

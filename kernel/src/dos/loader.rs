// Загрузчик DOS .COM файлов
// .COM — flat binary, исполняется с offset 0x100 в сегменте
// Весь сегмент 64KB, PSP занимает первые 256 байт (0x00-0xFF)

use crate::fs;
use crate::vga;

// Адрес в памяти куда грузим DOS программы (1MB зона)
// Сегмент 0x2000 → линейный адрес 0x20000
pub const DOS_SEGMENT: u32 = 0x2000;
pub const DOS_LINEAR:  u32 = DOS_SEGMENT << 4; // 0x20000
pub const PSP_SIZE:    u32 = 0x100;
pub const COM_MAX:     usize = 0xFF00; // максимум .COM

pub struct ComProgram {
    pub segment: u32,
    pub size: usize,
}

/// Загружает .COM файл из FS в память по адресу DOS_LINEAR + PSP_SIZE
/// Возвращает размер программы или None если файл не найден / слишком большой
pub fn load_com(filename: &str) -> Option<ComProgram> {
    let file = fs::get(filename)?;
    let content = file.content_str().as_bytes();

    if content.len() > COM_MAX {
        vga::print("Error: .COM file too large (max 65024 bytes)\n");
        return None;
    }

    // Строим PSP (Program Segment Prefix) — минимальная версия
    build_psp(DOS_LINEAR as *mut u8, content.len());

    // Копируем тело программы после PSP
    let dst = (DOS_LINEAR + PSP_SIZE) as *mut u8;
    unsafe {
        for (i, &b) in content.iter().enumerate() {
            *dst.add(i) = b;
        }
    }

    Some(ComProgram {
        segment: DOS_SEGMENT,
        size: content.len(),
    })
}

/// Минимальный PSP (256 байт)
fn build_psp(base: *mut u8, prog_size: usize) {
    unsafe {
        // Заполняем нулями
        for i in 0..256 {
            *base.add(i) = 0;
        }
        // INT 20h (terminate) по смещению 0x00
        *base.add(0x00) = 0xCD;
        *base.add(0x01) = 0x20;
        // Размер памяти в параграфах по смещению 0x02
        let mem_size: u16 = 0x9FFF;
        *base.add(0x02) = (mem_size & 0xFF) as u8;
        *base.add(0x03) = (mem_size >> 8) as u8;
        // Адрес конца программы (0x06)
        let end = (PSP_SIZE as usize + prog_size) as u16;
        *base.add(0x06) = (end & 0xFF) as u8;
        *base.add(0x07) = (end >> 8) as u8;
    }
}

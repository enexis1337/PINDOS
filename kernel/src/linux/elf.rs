// ELF32 загрузчик для статически слинкованных i386 бинарников

use crate::linux::paging::{PageDir, PAGE_PRESENT, PAGE_WRITE, PAGE_USER, alloc_page};

// ELF магия
const ELF_MAGIC: u32 = 0x464C457F; // "\x7FELF"
const ET_EXEC:   u16 = 2;          // исполняемый файл
const EM_386:    u16 = 3;          // i386
const PT_LOAD:   u32 = 1;          // загружаемый сегмент

#[repr(C, packed)]
struct Elf32Header {
    e_ident:     [u8; 16],
    e_type:      u16,
    e_machine:   u16,
    e_version:   u32,
    e_entry:     u32,   // точка входа
    e_phoff:     u32,   // offset program headers
    e_shoff:     u32,
    e_flags:     u32,
    e_ehsize:    u16,
    e_phentsize: u16,
    e_phnum:     u16,   // количество program headers
    e_shentsize: u16,
    e_shnum:     u16,
    e_shstrndx:  u16,
}

#[repr(C, packed)]
struct Elf32Phdr {
    p_type:   u32,
    p_offset: u32,  // offset в файле
    p_vaddr:  u32,  // виртуальный адрес
    p_paddr:  u32,  // физический адрес (игнорируем)
    p_filesz: u32,  // размер в файле
    p_memsz:  u32,  // размер в памяти (>= filesz, остаток = BSS)
    p_flags:  u32,  // PF_R=4, PF_W=2, PF_X=1
    p_align:  u32,
}

pub struct LoadedElf {
    pub entry:      u32,    // точка входа
    pub stack_top:  u32,    // вершина стека
    pub page_dir:   PageDir,
    pub brk:        u32,    // текущий конец heap
    pub brk_start:  u32,    // начало heap
}

#[derive(Debug)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    NotExecutable,
    NotI386,
    NoPhdrs,
    AllocFail,
    BadSegment,
}

/// Загрузить ELF из байтового среза в новое адресное пространство
pub fn load(data: &[u8]) -> Result<LoadedElf, ElfError> {
    if data.len() < core::mem::size_of::<Elf32Header>() {
        return Err(ElfError::TooSmall);
    }

    let hdr = unsafe { &*(data.as_ptr() as *const Elf32Header) };

    // Проверяем магию
    let magic = u32::from_le_bytes([
        hdr.e_ident[0], hdr.e_ident[1], hdr.e_ident[2], hdr.e_ident[3]
    ]);
    if magic != ELF_MAGIC { return Err(ElfError::BadMagic); }
    if hdr.e_type != ET_EXEC { return Err(ElfError::NotExecutable); }
    if hdr.e_machine != EM_386 { return Err(ElfError::NotI386); }
    if hdr.e_phnum == 0 { return Err(ElfError::NoPhdrs); }

    // Создаём новое адресное пространство
    let pd = PageDir::new().ok_or(ElfError::AllocFail)?;

    let mut max_vaddr: u32 = 0;

    // Обрабатываем program headers
    for i in 0..hdr.e_phnum as usize {
        let ph_off = hdr.e_phoff as usize + i * hdr.e_phentsize as usize;
        if ph_off + core::mem::size_of::<Elf32Phdr>() > data.len() {
            return Err(ElfError::BadSegment);
        }
        let ph = unsafe { &*(data.as_ptr().add(ph_off) as *const Elf32Phdr) };

        if ph.p_type != PT_LOAD { continue; }
        if ph.p_memsz == 0 { continue; }

        let vaddr = ph.p_vaddr;
        let memsz = ph.p_memsz;
        let filesz = ph.p_filesz;

        // Флаги страниц
        let flags = PAGE_USER | PAGE_WRITE; // упрощённо — всё writable

        // Выделяем физические страницы и маппим
        let pages = (memsz + 0xFFF) / 0x1000;
        let vpage_start = vaddr & 0xFFFFF000;

        for p in 0..pages {
            let phys = alloc_page().ok_or(ElfError::AllocFail)?;
            let virt = vpage_start + p * 0x1000;
            if !pd.map(virt, phys, flags) {
                return Err(ElfError::AllocFail);
            }

            // Копируем данные из файла
            let virt_off = (virt as i64 - vaddr as i64) as i64;
            let file_start = ph.p_offset as usize;

            unsafe {
                let dst = phys as *mut u8;
                // Обнуляем страницу
                for j in 0..0x1000usize { *dst.add(j) = 0; }

                // Копируем данные файла
                for byte_off in 0..0x1000usize {
                    let file_byte = virt_off + byte_off as i64;
                    if file_byte >= 0 && (file_byte as usize) < filesz as usize {
                        let src_off = file_start + file_byte as usize;
                        if src_off < data.len() {
                            *dst.add(byte_off) = data[src_off];
                        }
                    }
                    // Остаток (BSS) уже обнулён
                }
            }
        }

        let end = vaddr + memsz;
        if end > max_vaddr { max_vaddr = end; }
    }

    // Heap начинается сразу после последнего сегмента (выровнено по странице)
    let brk_start = (max_vaddr + 0xFFF) & 0xFFFFF000;

    // Стек: 8MB в конце пользовательского пространства
    let stack_top:  u32 = 0xBFFFF000;
    let stack_size: u32 = 0x800000; // 8MB
    let stack_bot:  u32 = stack_top - stack_size;

    let stack_pages = stack_size / 0x1000;
    for p in 0..stack_pages {
        let phys = alloc_page().ok_or(ElfError::AllocFail)?;
        let virt = stack_bot + p * 0x1000;
        if !pd.map(virt, phys, PAGE_USER | PAGE_WRITE) {
            return Err(ElfError::AllocFail);
        }
    }

    Ok(LoadedElf {
        entry: hdr.e_entry,
        stack_top,
        page_dir: pd,
        brk: brk_start,
        brk_start,
    })
}

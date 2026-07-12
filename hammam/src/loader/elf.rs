use xmas_elf::ElfFile;
use xmas_elf::program::Type;
use crate::mm::{map_page, PageFlags, BuddyAllocator, AllocError};

/// Ошибки при загрузке ELF файлов
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    InvalidMagic,
    InvalidElf,
    AllocationError,
    MappingError,
}

impl From<AllocError> for ElfError {
    fn from(_: AllocError) -> Self {
        ElfError::AllocationError
    }
}

/// Загрузчик ELF файлов
pub struct ElfLoader<'a> {
    data: &'a [u8],
    allocator: &'a mut BuddyAllocator,
}

impl<'a> ElfLoader<'a> {
    /// Создает новый ELF загрузчик
    pub fn new(data: &'a [u8], allocator: &'a mut BuddyAllocator) -> Self {
        ElfLoader { data, allocator }
    }

    /// Загрузить ELF в address space.
    /// Возвращает виртуальный адрес точки входа (e_entry).
    pub fn load(&mut self) -> Result<u64, ElfError> {
        let elf = ElfFile::new(self.data).map_err(|_| ElfError::InvalidMagic)?;

        // Detect PIE binaries (first LOAD VA < 0x1000) and relocate to USER_BASE
        let first_load_vaddr = elf.program_iter()
            .filter(|ph| ph.get_type() == Ok(Type::Load))
            .map(|ph| ph.virtual_addr())
            .min()
            .unwrap_or(0);
        const USER_BASE: u64 = 0x10000000;
        let load_offset = if first_load_vaddr < 0x1000 { USER_BASE } else { 0 };

        let entry = elf.header.pt2.entry_point() + load_offset;

        // Итерируем через программные сегменты (segments)
        for ph in elf.program_iter() {
            // Нас интересуют только LOAD сегменты
            if ph.get_type() != Ok(Type::Load) {
                continue;
            }

            let vaddr = ph.virtual_addr() + load_offset;
            let filesz = ph.file_size() as usize;
            let memsz = ph.mem_size() as usize;
            let offset = ph.offset() as usize;
            let flags = ph.flags();

            // Вычисляем количество страниц для отображения
            let start_page = vaddr & !0xFFF;
            let end_page = (vaddr + memsz as u64 + 0xFFF) & !0xFFF;
            let num_pages = ((end_page - start_page) / 0x1000) as usize;

            // Выделяем и отображаем физические фреймы для каждой страницы
            for i in 0..num_pages {
                let frame = self.allocator.allocate(0)?;
                let page_vaddr = start_page + (i * 0x1000) as u64;

                // Преобразуем флаги ELF в флаги пейджинга
                let mut page_flags = PageFlags::PRESENT | PageFlags::USER_ACCESSIBLE;
                
                if flags.is_write() {
                    page_flags |= PageFlags::WRITABLE;
                }
                
                if flags.is_execute() {
                    // NO_EXECUTE не устанавливаем
                } else {
                    page_flags |= PageFlags::NO_EXECUTE;
                }

                // Отображаем страницу
                // SAFETY: только что выделили фрейм, адрес в userspace
                unsafe {
                    map_page(page_vaddr, frame, page_flags, self.allocator).map_err(|_| ElfError::MappingError)?;
                }
            }

            // Копируем данные сегмента из ELF файла в память
            if filesz > 0 {
                let src = &self.data[offset..offset + filesz];
                // SAFETY: только что замапили эти страницы, копируем данные
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        src.as_ptr(),
                        vaddr as *mut u8,
                        filesz,
                    );
                }
            }

            // Обнулить BSS (части памяти, которые должны быть инициализированы нулями)
            if memsz > filesz {
                // SAFETY: меmsz >= filesz, обнулять безопасно
                unsafe {
                    core::ptr::write_bytes(
                        (vaddr as *mut u8).add(filesz),
                        0,
                        memsz - filesz,
                    );
                }
            }
        }

        // Apply PIE relocations (R_X86_64_RELATIVE) for PIE binaries
        self.apply_relocations(load_offset)?;

        // Возвращаем смещённую точку входа
        Ok(entry)
    }

    /// Apply R_X86_64_RELATIVE relocations from .rela.dyn (pointed to by PT_DYNAMIC).
    fn apply_relocations(&self, load_offset: u64) -> Result<(), ElfError> {
        if load_offset == 0 || load_offset > 0x100000000 {
            return Ok(());
        }
        let elf = ElfFile::new(self.data).map_err(|_| ElfError::InvalidMagic)?;

        let mut rela_va = 0u64;
        let mut rela_size = 0u64;
        let mut rela_ent = 24u64;

        for ph in elf.program_iter() {
            if ph.get_type() != Ok(Type::Dynamic) {
                continue;
            }
            let dyn_off = ph.offset() as usize;
            let dyn_sz = ph.file_size() as usize;
            let mut i = 0;
            while i + 16 <= dyn_sz {
                let d_tag = u64::from_ne_bytes(
                    self.data[dyn_off + i..dyn_off + i + 8].try_into().unwrap()
                );
                let d_val = u64::from_ne_bytes(
                    self.data[dyn_off + i + 8..dyn_off + i + 16].try_into().unwrap()
                );
                match d_tag {
                    7 => rela_va = d_val,             // DT_RELA
                    8 => rela_size = d_val,           // DT_RELASZ
                    9 => rela_ent = d_val,            // DT_RELAENT
                    0 => break,
                    _ => {}
                }
                i += 16;
            }
            break;
        }

        if rela_va == 0 || rela_size == 0 {
            return Ok(());
        }

        let delta = load_offset;
        let count = rela_size / rela_ent;
        for i in 0..count {
            let entry_va = rela_va + delta + i * rela_ent;
            let r_offset = unsafe { core::ptr::read_volatile(entry_va as *const u64) };
            let r_info = unsafe { core::ptr::read_volatile((entry_va + 8) as *const u64) };
            let r_addend = unsafe { core::ptr::read_volatile((entry_va + 16) as *const i64) };

            let r_type = (r_info & 0xFFFFFFFF) as u32;
            if r_type == 8 {
                let target_va = r_offset + delta;
                let value = (r_addend as u64).wrapping_add(delta);
                unsafe { core::ptr::write_volatile(target_va as *mut u64, value); }
            }
        }
        Ok(())
    }
}

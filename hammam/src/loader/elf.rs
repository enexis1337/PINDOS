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

        // Итерируем через программные сегменты (segments)
        for ph in elf.program_iter() {
            // Нас интересуют только LOAD сегменты
            if ph.get_type() != Ok(Type::Load) {
                continue;
            }

            let vaddr = ph.virtual_addr();
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

        // Возвращаем точку входа программы
        Ok(elf.header.pt2.entry_point())
    }
}

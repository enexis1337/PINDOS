use crate::boot_info::{MemoryKind, MemoryRegion};
use crate::drivers::serial::SpinMutex;

extern "C" {
    static kernel_phys_start: u8;
    static kernel_phys_end: u8;
}

/// Максимальный порядок (order) блоков в Buddy Allocator.
/// Порядки от 0 до 10 включительно (2^10 * 4 KiB = 4 MiB).
pub const MAX_ORDER: usize = 11;
/// Размер одной физической страницы в байтах.
pub const PAGE_SIZE: usize = 4096;

/// Физический фрейм памяти, представляющий собой выровненную 4 KiB область физических адресов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct PhysFrame {
    pub start_address: u64,
}

impl PhysFrame {
    /// Создает новое представление физического фрейма из указанного адреса.
    pub const fn new(addr: u64) -> Self {
        Self { start_address: addr }
    }
}

/// Ошибки выделения физической памяти.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    /// Физическая память исчерпана.
    OutOfMemory,
    /// Неверный порядок блока (например, превышающий MAX_ORDER - 1).
    InvalidOrder,
}

/// Связный список свободных блоков памяти.
/// Поля хранятся прямо в начале свободной области памяти (инлайн-хранение).
struct FreeBlock {
    next: Option<*mut FreeBlock>,
}

/// Аллокатор физических страниц на базе алгоритма Buddy (Близнецов).
/// Управляет свободными фреймами без использования динамической кучи.
pub struct BuddyAllocator {
    free_lists: [Option<*mut FreeBlock>; MAX_ORDER],
    total_frames: usize,
    free_frames: usize,
}

// SAFETY: BuddyAllocator работает с сырыми указателями, но так как доступ к нему
// защищается посредством SpinMutex во всем ядре, реализация Send безопасна.
unsafe impl Send for BuddyAllocator {}

impl Default for BuddyAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl BuddyAllocator {
    /// Создает пустой экземпляр аллокатора.
    pub const fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER],
            total_frames: 0,
            free_frames: 0,
        }
    }

    /// Инициализирует аллокатор на основе карты памяти UEFI, пропуская занятые регионы.
    ///
    /// # Safety
    /// Функция должна вызываться один раз на этапе старта ядра с валидной картой памяти.
    pub unsafe fn init(&mut self, memory_map: &[MemoryRegion]) {
        for region in memory_map {
            // Используем только Usable оперативную память для аллокатора
            if region.kind != MemoryKind::Usable {
                continue;
            }

            // Выравниваем границы региона по размеру страницы (4 KiB)
            let start = (region.start + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
            let end = region.end & !(PAGE_SIZE as u64 - 1);

            if start < end {
                // SAFETY: Инициализация свободного участка физической памяти.
                unsafe {
                    self.add_memory_region(start, end);
                }
            }
        }
    }

    /// Добавляет свободный физический диапазон адресов в списки свободных страниц аллокатора.
    ///
    /// # Safety
    /// Границы диапазона должны быть строго выровнены по размеру страницы (4 KiB).
    unsafe fn add_memory_region(&mut self, start_addr: u64, end_addr: u64) {
        // Skip the kernel's own loaded range (code + data + bss)
        // SAFETY: kernel_phys_start/end are plain u8 symbols from the linker script.
        let kstart = unsafe { &kernel_phys_start as *const u8 as u64 };
        let kend = unsafe { &kernel_phys_end as *const u8 as u64 };

        // Process up to two sub-ranges (before kernel, after kernel)
        let sub_ranges = [
            (start_addr, core::cmp::min(end_addr, kstart)),
            (core::cmp::max(start_addr, kend), end_addr),
        ];

        for &(range_start, range_end) in &sub_ranges {
            if range_start >= range_end {
                continue;
            }
            let mut current_addr = if range_start == 0 { PAGE_SIZE as u64 } else { range_start };
            while current_addr < range_end {
                let size = range_end - current_addr;
                let max_pages = size / PAGE_SIZE as u64;
                if max_pages == 0 {
                    break;
                }

                // Находим максимальный порядок блока, который помещается в остаток региона
                // и выровнен по границе (2^order * PAGE_SIZE)
                let mut order = (MAX_ORDER - 1) as u8;
                while order > 0 {
                    let block_size = (1u64 << order) * PAGE_SIZE as u64;
                    if max_pages >= (1u64 << order) && current_addr.is_multiple_of(block_size) {
                        break;
                    }
                    order -= 1;
                }

                let block_size = (1u64 << order) * PAGE_SIZE as u64;

                // SAFETY: Мы добавляем чистый и свободный участок памяти.
                unsafe {
                    self.deallocate_internal(PhysFrame::new(current_addr), order);
                }
                self.total_frames += 1 << order;
                current_addr += block_size;
            }
        }
    }

    /// Выделяет физический блок страниц указанного порядка.
    /// Order 0 = 4 KiB, Order N = 2^N * 4 KiB.
    pub fn allocate(&mut self, order: u8) -> Result<PhysFrame, AllocError> {
        if order as usize >= MAX_ORDER {
            return Err(AllocError::InvalidOrder);
        }

        // 1. Ищем свободный блок начиная с запрошенного порядка
        for current_order in (order as usize)..MAX_ORDER {
            if let Some(block_ptr) = self.free_lists[current_order] {
                // Извлекаем блок из списка свободных блоков
                // SAFETY: Указатели во free_lists всегда указывают на валидные свободные блоки.
                unsafe {
                    self.free_lists[current_order] = (*block_ptr).next;
                }

                let block_addr = block_ptr as u64;

                // 2. Если мы нашли блок большего размера, разделяем его (buddy split)
                for split_order in (order as usize..current_order).rev() {
                    let buddy_size = (1u64 << split_order) * PAGE_SIZE as u64;
                    let buddy_addr = block_addr + buddy_size;
                    let buddy_ptr = buddy_addr as *mut FreeBlock;

                    // Помещаем вторую половину в список свободных блоков меньшего порядка
                    // SAFETY: Память по buddy_addr свободна и доступна для записи заголовка списка.
                    unsafe {
                        (*buddy_ptr).next = self.free_lists[split_order];
                        self.free_lists[split_order] = Some(buddy_ptr);
                    }
                }

                self.free_frames -= 1 << order;
                return Ok(PhysFrame::new(block_addr));
            }
        }

        Err(AllocError::OutOfMemory)
    }

    /// Освобождает физический блок страниц указанного порядка.
    pub fn deallocate(&mut self, frame: PhysFrame, order: u8) {
        if order as usize >= MAX_ORDER {
            return;
        }
        // SAFETY: Безопасный вызов внутреннего механизма освобождения с объединением.
        unsafe {
            self.deallocate_internal(frame, order);
        }
    }

    /// Внутренний механизм освобождения с рекурсивным объединением близнецов (buddy merge).
    ///
    /// # Safety
    /// Освобождаемый адрес фрейма должен быть ранее выделен или находиться в Usable-области.
    unsafe fn deallocate_internal(&mut self, frame: PhysFrame, order: u8) {
        let mut current_addr = frame.start_address;
        let mut current_order = order;

        while (current_order as usize) < MAX_ORDER - 1 {
            let block_size = (1u64 << current_order) * PAGE_SIZE as u64;
            // Вычисляем адрес близнеца (buddy) с помощью XOR с размером блока
            let buddy_addr = current_addr ^ block_size;

            // Пытаемся найти близнеца в списке свободных на текущем уровне порядка
            // SAFETY: Поиск и удаление близнеца из списка свободных блоков.
            if unsafe { self.remove_from_list(current_order, buddy_addr) } {
                // Близнец свободен! Объединяем их в один блок удвоенного размера
                current_addr = core::cmp::min(current_addr, buddy_addr);
                current_order += 1;
            } else {
                // Близнец занят, объединение невозможно
                break;
            }
        }

        // Помещаем финальный (возможно, объединенный) блок в список свободных
        let block_ptr = current_addr as *mut FreeBlock;
        // SAFETY: Записываем указатель связного списка по свободному физическому адресу.
        unsafe {
            (*block_ptr).next = self.free_lists[current_order as usize];
            self.free_lists[current_order as usize] = Some(block_ptr);
        }

        self.free_frames += 1 << order;
    }

    /// Удаляет блок с указанным адресом из списка свободных страниц определенного порядка.
    ///
    /// # Safety
    /// Указатели внутри связных списков должны быть валидными или равняться None.
    unsafe fn remove_from_list(&mut self, order: u8, block_addr: u64) -> bool {
        let mut current = &mut self.free_lists[order as usize];
        while let Some(node_ptr) = *current {
            if node_ptr as u64 == block_addr {
                // Нашли нужный блок, удаляем из связного списка
                // SAFETY: Указатели валидны в контексте списков аллокатора.
                unsafe {
                    *current = (*node_ptr).next;
                }
                return true;
            }
            // SAFETY: Переходим по списку к следующему элементу.
            unsafe {
                current = &mut (*node_ptr).next;
            }
        }
        false
    }

    /// Возвращает общее количество физических фреймов под управлением аллокатора.
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// Инициализирует аллокатор известным свободным диапазоном памяти.
    /// Используется, когда нет карты памяти от загрузчика.
    pub fn bootstrap(&mut self, start: u64, end: u64) {
        if start < end {
            unsafe { self.add_memory_region(start, end); }
        }
    }

    /// Возвращает количество доступных (свободных) физических фреймов.
    pub fn free_frames(&self) -> usize {
        self.free_frames
    }
}

/// Глобальный экземпляр физического аллокатора страниц под защитой SpinMutex.
pub static PHYSICAL_ALLOCATOR: SpinMutex<BuddyAllocator> = SpinMutex::new(BuddyAllocator::new());

// PINDOS — x86 страничная адресация (paging)
// 4KB страницы, 32-bit, без PAE
//
// Раскладка виртуальной памяти процесса:
//   0x00000000 - 0xBFFFFFFF  — пользовательское пространство (3GB)
//   0xC0000000 - 0xFFFFFFFF  — ядро (1GB, identity-mapped)
//
// Физическая память:
//   0x00000000 - 0x000FFFFF  — первый мегабайт (BIOS, VGA, ядро)
//   0x00100000 - 0x01FFFFFF  — 31MB для ядра и процессов

// Физический аллокатор — простой bump allocator
static mut PHYS_NEXT: u32 = 0x0200000; // начинаем с 2MB (после ядра)
const PHYS_END: u32 = 0x2000000;       // 32MB

pub fn alloc_page() -> Option<u32> {
    unsafe {
        if PHYS_NEXT + 0x1000 > PHYS_END {
            return None;
        }
        let addr = PHYS_NEXT;
        PHYS_NEXT += 0x1000;
        // Обнуляем страницу
        let ptr = addr as *mut u32;
        for i in 0..1024 {
            *ptr.add(i) = 0;
        }
        Some(addr)
    }
}

pub fn free_page(_addr: u32) {
    // Bump allocator не освобождает — для простоты
    // В реальной ОС здесь был бы free list
}

// ── Page Directory / Table ────────────────────────────────────────────────

// Флаги PDE/PTE
pub const PAGE_PRESENT:  u32 = 1 << 0;
pub const PAGE_WRITE:    u32 = 1 << 1;
pub const PAGE_USER:     u32 = 1 << 2;

pub struct PageDir {
    pub phys_addr: u32,
}

impl PageDir {
    /// Создать новый page directory с identity mapping ядра (0xC0000000+)
    pub fn new() -> Option<Self> {
        let pd_phys = alloc_page()?;
        let pd = pd_phys as *mut u32;

        // Маппим ядро: виртуальные 0xC0000000-0xFFFFFFFF → физические 0x00000000-0x3FFFFFFF
        // Используем 4MB страницы (PSE) для простоты маппинга ядра
        // Индексы PD для 0xC0000000: 768..1023
        unsafe {
            // ИСПРАВЛЕНО: {0:e} для 32-битных значений cr4
            let mut cr4: u32;
            core::arch::asm!("mov {:e}, cr4", out(reg) cr4);
            cr4 |= 1 << 4; // PSE bit
            core::arch::asm!("mov cr4, {:e}", in(reg) cr4);

            // Identity map ядра через 4MB страницы
            for i in 0..256usize {
                let virt_idx = 768 + i;
                let phys = (i as u32) * 0x400000;
                *pd.add(virt_idx) = phys | PAGE_PRESENT | PAGE_WRITE | (1 << 7);
            }

            // Identity map первых 16MB физически (ядро живёт в 0x10000..~0x200000)
            // Используем 4MB страницы: 0x0, 0x400000, 0x800000, 0xC00000
            for i in 0..4usize {
                let phys = (i as u32) * 0x400000;
                *pd.add(i) = phys | PAGE_PRESENT | PAGE_WRITE | (1 << 7);
            }
        }

        Some(PageDir { phys_addr: pd_phys })
    }

    /// Замапить виртуальную страницу на физическую
    // ИСПРАВЛЕНО: ? оператор — меняем возврат на Option<bool>
    pub fn map(&self, virt: u32, phys: u32, flags: u32) -> bool {
        let pd = self.phys_addr as *mut u32;
        let pd_idx = (virt >> 22) as usize;
        let pt_idx = ((virt >> 12) & 0x3FF) as usize;

        unsafe {
            let pde = *pd.add(pd_idx);
            let pt_phys = if pde & PAGE_PRESENT != 0 && pde & (1 << 7) == 0 {
                // PT уже существует
                pde & 0xFFFFF000
            } else if pde & PAGE_PRESENT == 0 {
                // Создаём новый PT
                // ИСПРАВЛЕНО: ? заменён на match (функция возвращает bool)
                let pt = match alloc_page() {
                    Some(p) => p,
                    None => return false,
                };
                *pd.add(pd_idx) = pt | PAGE_PRESENT | PAGE_WRITE | PAGE_USER;
                pt
            } else {
                return false; // 4MB страница — нельзя переопределить
            };

            let pt = pt_phys as *mut u32;
            *pt.add(pt_idx) = (phys & 0xFFFFF000) | flags | PAGE_PRESENT;
            true
        }
    }

    /// Замапить диапазон виртуальных адресов
    pub fn map_range(&self, virt_start: u32, phys_start: u32, size: u32, flags: u32) -> bool {
        let pages = (size + 0xFFF) / 0x1000;
        for i in 0..pages {
            if !self.map(virt_start + i * 0x1000, phys_start + i * 0x1000, flags) {
                return false;
            }
        }
        true
    }

    /// Активировать этот page directory
    pub fn activate(&self) {
        unsafe {
            // ИСПРАВЛЕНО: :e суффикс для 32-битных cr3/cr0
            core::arch::asm!("mov cr3, {:e}", in(reg) self.phys_addr);
            let mut cr0: u32;
            core::arch::asm!("mov {:e}, cr0", out(reg) cr0);
            if cr0 & (1 << 31) == 0 {
                cr0 |= 1 << 31;
                core::arch::asm!("mov cr0, {:e}", in(reg) cr0);
            }
        }
    }

    /// Восстановить page directory ядра
    pub fn restore_kernel() {
        unsafe {
            core::arch::asm!("mov cr3, {:e}", in(reg) KERNEL_PD);
        }
    }
}

// Глобальный page directory ядра
static mut KERNEL_PD: u32 = 0;

pub fn init() {
    let pd = match PageDir::new() {
        Some(p) => p,
        None => panic!("Failed to create kernel page directory"),
    };
    unsafe { KERNEL_PD = pd.phys_addr; }
    pd.activate();
}

/// Ленивая инициализация — вызывается только перед запуском ELF
pub fn ensure_init() {
    unsafe {
        if KERNEL_PD != 0 { return; }
    }
    init();
}

pub fn kernel_pd() -> u32 {
    unsafe { KERNEL_PD }
}

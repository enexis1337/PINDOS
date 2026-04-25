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
            // Включаем PSE (4MB pages) в CR4
            let mut cr4: u32;
            core::arch::asm!("mov {}, cr4", out(reg) cr4);
            cr4 |= 1 << 4; // PSE bit
            core::arch::asm!("mov cr4, {}", in(reg) cr4);

            // Identity map ядра через 4MB страницы
            for i in 0..256usize {
                let virt_idx = 768 + i;
                let phys = (i as u32) * 0x400000;
                // PDE с флагом PS (4MB), Present, Write
                *pd.add(virt_idx) = phys | PAGE_PRESENT | PAGE_WRITE | (1 << 7);
            }

            // Также маппим первые 4MB физически (для VGA и BIOS)
            *pd.add(0) = 0x000000 | PAGE_PRESENT | PAGE_WRITE | (1 << 7);
        }

        Some(PageDir { phys_addr: pd_phys })
    }

    /// Замапить виртуальную страницу на физическую
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
                let pt = alloc_page()?;
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
            core::arch::asm!("mov cr3, {}", in(reg) self.phys_addr);
            // Включаем paging если ещё не включено
            let mut cr0: u32;
            core::arch::asm!("mov {}, cr0", out(reg) cr0);
            if cr0 & (1 << 31) == 0 {
                cr0 |= 1 << 31;
                core::arch::asm!("mov cr0, {}", in(reg) cr0);
            }
        }
    }

    /// Восстановить page directory ядра
    pub fn restore_kernel() {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) KERNEL_PD);
        }
    }
}

// Глобальный page directory ядра
static mut KERNEL_PD: u32 = 0;

pub fn init() {
    // Создаём page directory ядра
    let pd = PageDir::new().expect("Failed to create kernel page directory");
    unsafe { KERNEL_PD = pd.phys_addr; }
    pd.activate();
}

pub fn kernel_pd() -> u32 {
    unsafe { KERNEL_PD }
}

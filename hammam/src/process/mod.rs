use crate::drivers::serial::SpinMutex;
use crate::cap::CapTable;
use crate::loader::elf::ElfLoader;
use crate::mm::PHYSICAL_ALLOCATOR;
use crate::arch::gdt;

/// Структура процесса — контекст выполнения в Ring 3
pub struct Process {
    pub pid: u32,
    pub cap_table: SpinMutex<CapTable>,
    pub kernel_stack_top: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessError {
    ElfError,
    AllocationError,
    StackAllocationError,
}

impl Process {
    /// Создать новый процесс из ELF-бинаря в памяти.
    pub fn from_elf(pid: u32, elf_data: &[u8]) -> Result<Self, ProcessError> {
        // Получаем allocator для выделения памяти
        let mut allocator = PHYSICAL_ALLOCATOR.lock();

        // 1. Загружаем ELF через ElfLoader
        let mut loader = ElfLoader::new(elf_data, &mut allocator);
        let _entry_point = loader.load().map_err(|_| ProcessError::ElfError)?;

        // 2. Выделяем kernel stack для процесса (4 KiB)
        // Этот stack используется при входе из Ring 3 через SYSCALL
        let kernel_stack_frame = allocator
            .allocate(0)
            .map_err(|_| ProcessError::StackAllocationError)?;
        
        let kernel_stack_top = kernel_stack_frame.start_address + 0x1000;

        // 3. Устанавливаем kernel stack в TSS для этого процесса
        // (В реальной системе это должно быть per-CPU, но сейчас глобально)
        gdt::set_kernel_stack(kernel_stack_top);

        // 4. Создаем пустой CapTable
        let cap_table = SpinMutex::new(CapTable::new());

        // Создаем процесс
        Ok(Process {
            pid,
            cap_table,
            kernel_stack_top,
        })
    }

    /// Получить PID процесса
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Получить kernel stack top для этого процесса
    pub fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack_top
    }
}

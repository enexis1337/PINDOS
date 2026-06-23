use crate::drivers::serial::SpinMutex;
use crate::cap::CapTable;
use crate::loader::elf::ElfLoader;
use crate::mm::{PHYSICAL_ALLOCATOR, map_page, PageFlags};
use crate::arch::gdt;
use crate::arch::x86_64::syscall;

pub struct Process {
    pub pid: u32,
    pub cap_table: SpinMutex<CapTable>,
    pub kernel_stack_top: u64,
    pub entry_point: u64,
    pub user_stack_top: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessError {
    ElfError,
    AllocationError,
    StackAllocationError,
}

impl Process {
    pub fn from_elf(pid: u32, elf_data: &[u8]) -> Result<Self, ProcessError> {
        let mut allocator = PHYSICAL_ALLOCATOR.lock();

        let mut loader = ElfLoader::new(elf_data, &mut allocator);
        let entry_point = loader.load().map_err(|_| ProcessError::ElfError)?;

        let kernel_stack_frame = allocator
            .allocate(0)
            .map_err(|_| ProcessError::StackAllocationError)?;
        let kernel_stack_top = kernel_stack_frame.start_address + 0x1000;
        gdt::set_kernel_stack(kernel_stack_top);
        syscall::set_kernel_stack(kernel_stack_top);

        const USER_STACK_PAGES: u64 = 16; // 64 KiB
        let user_stack_vaddr: u64 = 0x08000000;
        let mut user_stack_bottom = user_stack_vaddr;
        let user_stack_vaddr_end = user_stack_vaddr + USER_STACK_PAGES * 0x1000;
        while user_stack_bottom < user_stack_vaddr_end {
            let frame = allocator
                .allocate(0)
                .map_err(|_| ProcessError::StackAllocationError)?;
            unsafe {
                map_page(
                    user_stack_bottom,
                    frame,
                    PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::USER_ACCESSIBLE,
                    &mut allocator,
                )
                .map_err(|_| ProcessError::StackAllocationError)?;
            }
            user_stack_bottom += 0x1000;
        }
        let user_stack_top = user_stack_vaddr_end;

        let cap_table = SpinMutex::new(CapTable::new());

        Ok(Process {
            pid,
            cap_table,
            kernel_stack_top,
            entry_point,
            user_stack_top,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack_top
    }
}

use crate::drivers::serial::SpinMutex;
use crate::cap::CapTable;
use crate::loader::elf::ElfLoader;
use crate::mm::{PHYSICAL_ALLOCATOR, map_page, PageFlags};
use crate::arch::gdt;
use crate::arch::x86_64::syscall;
use crate::sched::task::{Task, AddressSpace, Mutex, TaskState};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicI32, AtomicBool, Ordering};

pub struct Process {
    pub pid:           u32,
    pub address_space: Arc<Mutex<AddressSpace>>,
    pub cap_table:     SpinMutex<CapTable>,
    pub main_task:     Arc<Task>,
    pub entry_point:   u64,
    pub user_stack_top: u64,
    pub exit_code:     AtomicI32,
    pub is_zombie:     AtomicBool,
}

impl Process {
    pub fn is_zombie(&self) -> bool {
        self.is_zombie.load(Ordering::Acquire)
    }
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

        const USER_STACK_PAGES: u64 = 16;
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

        let address_space = Arc::new(Mutex::new(AddressSpace));

        let mut task = Arc::new(Task::new(
            crate::sched::task::TaskId(pid as u64),
            1,
            Arc::clone(&address_space),
        ));

        // Initialize task context for scheduler: when switched to, return to userspace via SYSRET
        unsafe {
            let task_ptr = Arc::as_ptr(&task) as *mut Task;
            let stack_top = (*task_ptr).kernel_stack.top;
            let stack_ptr = (stack_top - core::mem::size_of::<u64>()) as *mut u64;
            *stack_ptr = crate::arch::x86_64::syscall::return_to_userspace_trampoline as u64;
            (*task_ptr).context.rsp = stack_ptr as u64;
            (*task_ptr).state = TaskState::Ready;
        }

        Ok(Process {
            pid,
            address_space,
            cap_table,
            main_task: task,
            entry_point,
            user_stack_top,
            exit_code: AtomicI32::new(0),
            is_zombie: AtomicBool::new(false),
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn kernel_stack_top(&self) -> u64 {
        self.main_task.kernel_stack.top as u64
    }
}

pub static PROCESS_TABLE: SpinMutex<BTreeMap<u32, Arc<Process>>> =
    SpinMutex::new(BTreeMap::new());

pub static CURRENT_PROCESS: SpinMutex<Option<Arc<Process>>> = SpinMutex::new(None);

pub fn next_pid() -> u32 {
    static NEXT_PID: core::sync::atomic::AtomicU32 =
        core::sync::atomic::AtomicU32::new(2);
    NEXT_PID.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}
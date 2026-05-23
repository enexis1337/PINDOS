#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use memory::{VmArea, VmFlags, AddressSpace};

/// Maximum number of processes
const MAX_PROCESSES: usize = 256;

/// Process states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Sleeping(u64), // Wake time in ticks
    Zombie,
    Terminated,
}

/// Process priority levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Thread control block
pub struct Thread {
    pub id: usize,
    pub process_id: usize,
    pub state: ProcessState,
    pub priority: Priority,
    pub stack_top: usize,
    pub stack_bottom: usize,
    pub context: ThreadContext,
    pub time_slice: u64,
    pub wake_time: u64,
}

impl Thread {
    /// Create a new thread
    pub fn new(process_id: usize, entry_point: usize, stack_top: usize) -> Self {
        Thread {
            id: 0,
            process_id,
            state: ProcessState::Ready,
            priority: Priority::Normal,
            stack_top,
            stack_bottom: stack_top - 0x10000, // 64KB stack
            context: ThreadContext::new(entry_point, stack_top),
            time_slice: 10, // Default time slice
            wake_time: 0,
        }
    }
}

/// Thread context saved during context switch
#[repr(C)]
pub struct ThreadContext {
    pub rax: usize,
    pub rbx: usize,
    pub rcx: usize,
    pub rdx: usize,
    pub rsi: usize,
    pub rdi: usize,
    pub rbp: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
    pub rip: usize,
    pub rsp: usize,
    pub rflags: usize,
}

impl ThreadContext {
    /// Create a new context for a thread
    pub fn new(entry_point: usize, stack_top: usize) -> Self {
        ThreadContext {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: entry_point,
            rsp: stack_top,
            rflags: 0x202, // IF flag set
        }
    }
}

/// Process control block
pub struct Process {
    pub id: usize,
    pub name: String,
    pub threads: Vec<Thread>,
    pub state: ProcessState,
    pub address_space: AddressSpace,
    pub parent_id: usize,
    pub exit_code: i32,
    pub capabilities: u64,
}

impl Process {
    /// Create a new process
    pub fn new(name: String) -> Self {
        Process {
            id: 0,
            name,
            threads: Vec::new(),
            state: ProcessState::Ready,
            address_space: AddressSpace::new(),
            parent_id: 0,
            exit_code: 0,
            capabilities: 0,
        }
    }

    /// Add a thread to the process
    pub fn add_thread(&mut self, thread: Thread) {
        self.threads.push(thread);
    }
}

/// Global process table
pub static PROCESS_TABLE: Mutex<[Option<Process>; MAX_PROCESSES]> = 
    Mutex::new([None; MAX_PROCESSES]);

/// Next available process ID
static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

/// Next available thread ID
static NEXT_TID: AtomicUsize = AtomicUsize::new(1);

/// Current running thread
static CURRENT_THREAD: AtomicUsize = AtomicUsize::new(0);

/// Ready queue for each priority level
static READY_QUEUE: Mutex<[Vec<usize>; 5]> = Mutex::new([
    Vec::new(), // Idle
    Vec::new(), // Low
    Vec::new(), // Normal
    Vec::new(), // High
    Vec::new(), // Critical
]);

/// Initialize the scheduler
pub fn init() {
    // Create the idle process
    let idle_process = Process::new("idle".to_string());
    let pid = allocate_pid();
    let mut table = PROCESS_TABLE.lock();
    table[pid].replace(idle_process);
    drop(table);
}

/// Allocate a new process ID
fn allocate_pid() -> usize {
    let mut pid = NEXT_PID.fetch_add(1, Ordering::SeqCst);
    if pid >= MAX_PROCESSES {
        pid = 1;
        NEXT_PID.store(pid, Ordering::SeqCst);
    }
    pid
}

/// Allocate a new thread ID
fn allocate_tid() -> usize {
    let mut tid = NEXT_TID.fetch_add(1, Ordering::SeqCst);
    if tid >= MAX_PROCESSES * 64 {
        tid = 1;
        NEXT_TID.store(tid, Ordering::SeqCst);
    }
    tid
}

/// Create a new process
pub fn create_process(name: String, entry_point: usize) -> usize {
    let pid = allocate_pid();
    let mut table = PROCESS_TABLE.lock();
    
    let mut process = Process::new(name);
    process.id = pid;
    
    // Allocate stack for main thread
    let stack_top = 0x200000; // 2MB stack for kernel space
    
    // Create main thread
    let thread = Thread::new(pid, entry_point, stack_top);
    process.threads.push(thread);
    
    table[pid].replace(process);
    
    // Add to ready queue
    let mut queue = READY_QUEUE.lock();
    queue[Priority::Normal as usize].push(pid);
    
    pid
}

/// Spawn a process (add to ready queue)
pub fn spawn(process: Process) -> usize {
    let pid = allocate_pid();
    let mut table = PROCESS_TABLE.lock();
    table[pid].replace(process);
    pid
}

/// Get current thread ID
pub fn get_current_tid() -> usize {
    CURRENT_THREAD.load(Ordering::SeqCst)
}

/// Get current process ID
pub fn get_current_pid() -> usize {
    let tid = get_current_tid();
    let table = PROCESS_TABLE.lock();
    for entry in table.iter() {
        if let Some(process) = entry {
            for thread in &process.threads {
                if thread.id == tid {
                    return process.id;
                }
            }
        }
    }
    0
}

/// Schedule the next thread to run
pub fn schedule() -> *mut ThreadContext {
    let mut queue = READY_QUEUE.lock();
    
    // Find highest priority queue with ready threads
    for priority in (Priority::Critical as usize..=Priority::Idle as usize).rev() {
        if !queue[priority].is_empty() {
            let pid = queue[priority].remove(0);
            let mut table = PROCESS_TABLE.lock();
            
            if let Some(process) = &mut table[pid] {
                if let Some(thread) = process.threads.first_mut() {
                    thread.state = ProcessState::Running;
                    CURRENT_THREAD.store(thread.id, Ordering::SeqCst);
                    return &mut thread.context as *mut ThreadContext;
                }
            }
        }
    }
    
    // No ready threads, return to idle
    core::ptr::null_mut()
}

/// Yield the CPU to the scheduler
pub fn yield_cpu() {
    // Save current thread state
    let tid = get_current_tid();
    let mut table = PROCESS_TABLE.lock();
    
    for entry in table.iter_mut() {
        if let Some(process) = entry {
            for thread in &mut process.threads {
                if thread.id == tid {
                    thread.state = ProcessState::Ready;
                    let mut queue = READY_QUEUE.lock();
                    queue[thread.priority as usize].push(process.id);
                    break;
                }
            }
        }
    }
    
    // Trigger context switch
    unsafe {
        context_switch();
    }
}

/// Block the current thread
pub fn block_current() {
    let tid = get_current_tid();
    let mut table = PROCESS_TABLE.lock();
    
    for entry in table.iter_mut() {
        if let Some(process) = entry {
            for thread in &mut process.threads {
                if thread.id == tid {
                    thread.state = ProcessState::Blocked;
                    break;
                }
            }
        }
    }
}

/// Wake a specific thread
pub fn wake_thread(tid: usize) {
    let mut table = PROCESS_TABLE.lock();
    
    for entry in table.iter_mut() {
        if let Some(process) = entry {
            for thread in &mut process.threads {
                if thread.id == tid && thread.state == ProcessState::Blocked {
                    thread.state = ProcessState::Ready;
                    let mut queue = READY_QUEUE.lock();
                    queue[thread.priority as usize].push(process.id);
                    break;
                }
            }
        }
    }
}

/// Sleep the current thread for specified ticks
pub fn sleep(ticks: u64) {
    let tid = get_current_tid();
    let mut table = PROCESS_TABLE.lock();
    
    for entry in table.iter_mut() {
        if let Some(process) = entry {
            for thread in &mut process.threads {
                if thread.id == tid {
                    thread.state = ProcessState::Sleeping(ticks);
                    break;
                }
            }
        }
    }
}

/// External context switch implementation
extern "C" fn context_switch();

#[naked]
#[no_mangle]
unsafe extern "C" fn context_switch() {
    core::arch::asm!(
        // Save current thread context
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "pushfq",
        
        // Get current thread pointer
        "mov rax, [rip + CURRENT_THREAD]",
        "mov rax, [rax]",
        
        // Save RSP to current thread context
        "mov [rax + 8*15], rsp", // offsetof(ThreadContext, rsp)
        
        // Call scheduler to get next thread
        "call schedule",
        
        // Load new thread context
        "mov rax, [rip + CURRENT_THREAD]",
        "mov rax, [rax]",
        
        // Load RSP from new thread context
        "mov rsp, [rax + 8*15]",
        
        // Restore registers
        "popfq",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",
        
        "iretq",
    );
}
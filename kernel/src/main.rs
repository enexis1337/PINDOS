#![no_std]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(const_fn_floating_point_arithmetic)]

extern crate alloc;

mod arch;
mod gdt;
mod idt;
mod memory;
mod scheduler;
mod ipc;
mod syscall;

use arch::bootstrap::early_init;
use memory::frame::{FRAME_ALLOCATOR, PhysicalFrameAllocator};
use scheduler::Process;

/// Kernel entry point called by boot assembly
#[no_mangle]
pub extern "C" fn kmain(multiboot_info: usize, boot_type: u32) -> ! {
    // Initialize early architecture-specific components
    early_init(multiboot_info, boot_type);
    
    // Initialize physical memory allocator
    FRAME_ALLOCATOR.lock().initialize();
    
    // Initialize the scheduler
    scheduler::init();
    
    // Initialize IPC subsystem
    ipc::init();
    
    // Initialize system call interface
    syscall::init();
    
    // Start the init process (dealduck)
    start_init_process();
    
    // Idle loop - should never return
    loop {
        arch::halt();
    }
}

fn start_init_process() {
    // Create the init process (PID 1) - dealduck
    let init_process = Process::new("/system/dealduck".to_string());
    scheduler::spawn(init_process);
}
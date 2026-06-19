#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let msg = b"hello from userspace\n";
    
    // Вызов syscall 1 (write)
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 1u64,                      // sys_write
            in("rdi") 1u64,                      // fd = stdout
            in("rsi") msg.as_ptr() as u64,      // buffer pointer
            in("rdx") msg.len() as u64,         // buffer length
        );
    }

    // Вызов syscall 60 (exit)
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 60u64,                     // sys_exit
            in("rdi") 0u64,                      // exit code
        );
    }

    loop {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

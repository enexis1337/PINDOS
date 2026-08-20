#![no_std]
#![no_main]

mod allocator;
mod println;
mod unit;
mod manager;

use manager::ServiceManager;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("[dealduck] PINDOS service manager starting...");
    println!("[dealduck] PID 1");

    let mut manager = ServiceManager::new();

    manager.register("net-server", "/usr/bin/net-server");

    println!("[dealduck] starting services...");
    manager.start_all();

    println!("[dealduck] all services started, entering monitor loop");
    manager.run()
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt", options(nostack)) };
    }
}
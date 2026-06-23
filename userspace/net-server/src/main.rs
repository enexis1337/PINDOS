#![no_std]
#![no_main]

//! TCP/IP стек полностью в userspace
//! Ядро предоставляет только raw доступ к NIC через capability
//!
//! Использует smoltcp для TCP/IP обработки
//! Virtio-net для QEMU/KVM

extern crate alloc;

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n { core::ptr::write_volatile(dest.add(i), core::ptr::read_volatile(src.add(i))); }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    for i in 0..n { core::ptr::write_volatile(s.add(i), c as u8); }
    s
}
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = core::ptr::read_volatile(s1.add(i));
        let b = core::ptr::read_volatile(s2.add(i));
        if a != b { return a as i32 - b as i32; }
    }
    0
}
#[no_mangle]
pub unsafe extern "C" fn bcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    memcmp(s1, s2, n)
}

/// Print via syscall write(1, ...)
macro_rules! println {
    ($($arg:tt)*) => {{
        let _fmt = core::format_args!($($arg)*);
        // Estimate formatted length: use a generous upper bound
        let _cap = 256usize;
        let _layout = core::alloc::Layout::array::<u8>(_cap).unwrap();
        let _ptr = unsafe { alloc::alloc::alloc(_layout) };
        if !_ptr.is_null() {
            struct HeapWriter(*mut u8, usize, usize);
            impl core::fmt::Write for HeapWriter {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    let b = s.as_bytes();
                    let remaining = self.2 - self.1;
                    if b.len() > remaining { return Err(core::fmt::Error); }
                    for i in 0..b.len() {
                        unsafe { core::ptr::write_volatile(self.0.add(self.1 + i), b[i]); }
                    }
                    self.1 += b.len();
                    Ok(())
                }
            }
            let mut _w = HeapWriter(_ptr, 0, _cap);
            let _ = core::fmt::Write::write_fmt(&mut _w, _fmt);
            if _w.1 < _cap {
                unsafe { core::ptr::write_volatile(_ptr.add(_w.1), b'\n'); }
                _w.1 += 1;
                #[allow(unused_unsafe)]
                unsafe {
                    core::arch::asm!(
                        "syscall",
                        in("rax") 1u64,
                        in("rdi") 1u64,
                        in("rsi") _ptr,
                        in("rdx") _w.1,
                    );
                }
            }
        }
    }};
}

mod alloc_impl;
mod device;
mod pci;
mod virtio;

use alloc::vec;
use smoltcp::{
    iface::{Config, Interface, SocketSet},
    time::Instant,
    wire::{EthernetAddress, IpCidr, Ipv4Address},
};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    main();
    loop {}
}

fn main() {
    println!("[net-server] starting virtio-net driver...");

    // 1. Найти virtio-net устройство на PCI шине
    let pci_dev = match pci::find_virtio_net() {
        Some(dev) => {
            println!("[net-server] found virtio-net at PCI bus={} slot={}", dev.bus, dev.slot);
            dev
        }
        None => {
            println!("[net-server] ERROR: virtio-net device not found!");
            return;
        }
    };

    // 2. Инициализировать RX и TX очереди
    let rx_queue = unsafe { virtio::Virtqueue::init(pci_dev.bar0, 0) };
    let tx_queue = unsafe { virtio::Virtqueue::init(pci_dev.bar0, 1) };

    // 3. Создать Device wrapper для smoltcp
    let mut device = device::VirtioNetDevice {
        rx_queue,
        tx_queue,
        rx_buf: [0u8; 1514],
    };


    // 5. Настроить smoltcp интерфейс
    let mac = EthernetAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    let config = Config::new(mac.into());
    let mut iface = Interface::new(config, &mut device, Instant::ZERO);

    // 6. Настроить IP адрес
    iface.update_ip_addrs(|addr_list| {
        addr_list
            .push(IpCidr::new(Ipv4Address::new(10, 0, 0, 2).into(), 24))
            .ok();
    });

    let mut sockets = SocketSet::new(vec![]);

    println!("[net-server] network interface ready:");
    println!("[net-server]   MAC:  52:54:00:12:34:56");
    println!("[net-server]   IPv4: 10.0.0.2/24");
    println!("[net-server]   Gateway: 10.0.0.1");
    println!("[net-server]   DNS: 10.0.2.3");
    println!("[net-server] entering main event loop");

    // 7. Event loop
    let mut poll_count = 0u64;
    loop {
        let timestamp = Instant::from_millis(poll_count as i64);
        iface.poll(timestamp, &mut device, &mut sockets);

        poll_count += 1;

        // Периодическое логирование
        if poll_count % 10_000_000 == 0 {
            println!("[net-server] running (poll cycles: {})", poll_count);
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}
#[no_mangle]
pub unsafe extern "C" fn _Unwind_Resume() { loop {} }
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut i = 0;
    while *s.add(i) != 0 { i += 1; }
    i
}
#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src < dest as *const u8 {
        for i in (0..n).rev() { core::ptr::write_volatile(dest.add(i), core::ptr::read_volatile(src.add(i))); }
    } else {
        for i in 0..n { core::ptr::write_volatile(dest.add(i), core::ptr::read_volatile(src.add(i))); }
    }
    dest
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 60u64, // sys_exit
            in("rdi") 1u64,  // код выхода 1 = ошибка
        );
    }
    loop {}
}

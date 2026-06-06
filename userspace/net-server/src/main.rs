#![no_std]
#![no_main]

//! TCP/IP стек полностью в userspace
//! Ядро предоставляет только raw доступ к NIC через capability
//!
//! Использует smoltcp для TCP/IP обработки
//! Virtio-net для QEMU/KVM

extern crate alloc;

mod device;
mod pci;
mod virtio;

use alloc::vec;
use device::VirtioNetDevice;
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
            println!(
                "[net-server] found virtio-net at PCI bus={} slot={}",
                dev.bus, dev.slot
            );
            dev
        }
        None => {
            println!("[net-server] ERROR: virtio-net device not found!");
            return;
        }
    };

    // 2. Инициализировать устройство
    unsafe {
        virtio::init_device(pci_dev.bar0);
    }
    println!("[net-server] device initialized");

    // 3. Инициализировать RX и TX очереди
    let rx_queue = unsafe { virtio::Virtqueue::init(pci_dev.bar0, 0) };
    let tx_queue = unsafe { virtio::Virtqueue::init(pci_dev.bar0, 1) };
    println!("[net-server] RX and TX queues initialized");

    // 4. Создать Device wrapper для smoltcp
    let mut device = VirtioNetDevice::new(rx_queue, tx_queue);

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

    println!("[net-server] interface up:");
    println!("  MAC: 52:54:00:12:34:56");
    println!("  IPv4: 10.0.0.2/24");
    println!("[net-server] event loop started");

    // 7. Event loop
    let mut poll_count = 0u64;
    loop {
        let timestamp = Instant::from_millis(poll_count as i64);
        iface.poll(timestamp, &mut device, &mut sockets);

        poll_count += 1;

        // Периодическое логирование
        if poll_count % 1_000_000 == 0 {
            println!("[net-server] poll cycles: {}", poll_count);
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("[net-server] PANIC: {}", info);
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nostack));
        }
    }
}

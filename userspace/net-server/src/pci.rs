/// PCI конфигурация для поиска virtio-net устройства
/// Vendor ID = 0x1AF4, Device ID = 0x1000 (virtio-net legacy)

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub bar0: u16,
}

/// Найти virtio-net устройство на PCI шине
pub fn find_virtio_net() -> Option<PciDevice> {
    for bus in 0u8..=255 {
        for slot in 0u8..32 {
            let addr = pci_config_addr(bus, slot, 0, 0);
            let vendor_device = pci_read32(addr);

            // Device ID (upper 16 bits) = 0x1000, Vendor ID (lower 16 bits) = 0x1AF4
            if vendor_device == 0x10001AF4 {
                let bar0_addr = pci_read32(pci_config_addr(bus, slot, 0, 0x10));
                // BAR0 — I/O порт (младшие биты — флаги, реальный адрес в битах 15:0 или выше)
                let bar0 = (bar0_addr & 0xFFFC) as u16;
                return Some(PciDevice { bus, slot, bar0 });
            }
        }
    }
    None
}

/// Конструктор адреса для PCI конфигурационного пространства
/// CONFIG_ADDRESS формат: 31=Enable, 23-16=Bus, 15-11=Slot, 10-8=Function, 7-0=Offset
fn pci_config_addr(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

/// Прочитать 32-битное значение из PCI конфигурации
/// Используется Port I/O через CONFIG_ADDRESS (0xCF8) и CONFIG_DATA (0xCFC)
fn pci_read32(addr: u32) -> u32 {
    unsafe {
        // Записать адрес в CONFIG_ADDRESS (0xCF8)
        core::arch::asm!(
            "out dx, eax",
            in("dx") 0xCF8u16,
            in("eax") addr,
            options(nostack)
        );

        // Прочитать значение из CONFIG_DATA (0xCFC)
        let val: u32;
        core::arch::asm!(
            "in eax, dx",
            in("dx") 0xCFCu16,
            out("eax") val,
            options(nostack)
        );
        val
    }
}

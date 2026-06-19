//! Virtio-net Device реализация для smoltcp
//! Использует DMA через capability system ядра

use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken};
use alloc::vec::Vec;
use alloc::boxed::Box;

/// Virtio-net Device трейт реализация
/// Использует DMA через capability system
pub struct VirtioNetDevice {
    mac_address: [u8; 6],
    mmio_base: *mut u8,
    rx_buffers: Vec<Box<[u8; 2048]>>,
    tx_buffers: Vec<Box<[u8; 2048]>>,
    rx_index: usize,
    tx_index: usize,
}

// Virtio-net MMIO registers (стандартный virtio PCI layout)
const VIRTIO_REG_DEVICE_FEATURES: u32 = 0x00;
const VIRTIO_REG_DRIVER_FEATURES: u32 = 0x04;
const VIRTIO_REG_QUEUE_ADDR: u32 = 0x08;
const VIRTIO_REG_QUEUE_SIZE: u32 = 0x0c;
const VIRTIO_REG_QUEUE_SEL: u32 = 0x14;
const VIRTIO_REG_STATUS: u32 = 0x18;
const VIRTIO_REG_ISR: u32 = 0x20;

// Virtio device status bits
const VIRTIO_STATUS_RESET: u8 = 0;
const VIRTIO_STATUS_ACK: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FAILED: u8 = 128;

impl VirtioNetDevice {
    /// Создать новое virtio-net устройство
    /// # Safety
    /// bar_addr должен быть корректным адресом MMIO
    pub unsafe fn new(bar_addr: u64, mac_address: [u8; 6]) -> Self {
        let mut device = VirtioNetDevice {
            mac_address,
            mmio_base: bar_addr as *mut u8,
            rx_buffers: Vec::with_capacity(32),
            tx_buffers: Vec::with_capacity(32),
            rx_index: 0,
            tx_index: 0,
        };

        // Выделить буферы для RX и TX
        for _ in 0..32 {
            device.rx_buffers.push(Box::new([0u8; 2048]));
            device.tx_buffers.push(Box::new([0u8; 2048]));
        }

        device
    }

    /// Прочитать MMIO регистр (4 байта)
    unsafe fn read_reg(&self, offset: u32) -> u32 {
        let addr = (self.mmio_base as u32 + offset) as *const u32;
        addr.read_volatile()
    }

    /// Записать в MMIO регистр (4 байта)
    unsafe fn write_reg(&mut self, offset: u32, value: u32) {
        let addr = (self.mmio_base as u32 + offset) as *mut u32;
        addr.write_volatile(value);
    }

    /// Инициализировать virtio-net устройство
    pub unsafe fn init(&mut self) -> Result<(), &'static str> {
        // 1. Reset device
        self.write_reg(VIRTIO_REG_STATUS, VIRTIO_STATUS_RESET as u32);

        // 2. Set ACKNOWLEDGE status
        self.write_reg(VIRTIO_REG_STATUS, VIRTIO_STATUS_ACK as u32);

        // 3. Set DRIVER status
        self.write_reg(VIRTIO_REG_STATUS, VIRTIO_STATUS_DRIVER as u32);

        // 4. Negotiate features (simplified - just set basic features)
        // Обычно: читаем DEVICE_FEATURES, выбираем подходящие, пишем в DRIVER_FEATURES
        let _features = self.read_reg(VIRTIO_REG_DEVICE_FEATURES);
        // Установить минимальные features для virtio-net
        self.write_reg(VIRTIO_REG_DRIVER_FEATURES, 0);

        // 5. Set DRIVER_OK status
        self.write_reg(VIRTIO_REG_STATUS, VIRTIO_STATUS_DRIVER_OK as u32);

        Ok(())
    }

    /// Получить MAC адрес
    pub fn mac_address(&self) -> [u8; 6] {
        self.mac_address
    }

    /// Получить следующий RX буфер
    fn next_rx_buffer(&mut self) -> Option<&mut [u8; 2048]> {
        if self.rx_index < self.rx_buffers.len() {
            let idx = self.rx_index;
            self.rx_index = (self.rx_index + 1) % self.rx_buffers.len();
            Some(&mut self.rx_buffers[idx])
        } else {
            None
        }
    }

    /// Получить следующий TX буфер
    fn next_tx_buffer(&mut self) -> Option<&mut [u8; 2048]> {
        if self.tx_index < self.tx_buffers.len() {
            let idx = self.tx_index;
            self.tx_index = (self.tx_index + 1) % self.tx_buffers.len();
            Some(&mut self.tx_buffers[idx])
        } else {
            None
        }
    }
}

/// RX Token для virtio-net
pub struct VirtioRxToken {
    buffer: Box<[u8; 2048]>,
    len: usize,
}

impl RxToken for VirtioRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = *self.buffer;
        let result = f(&mut buffer[..self.len]);
        result
    }
}

/// TX Token для virtio-net
pub struct VirtioTxToken {
    buffer: Box<[u8; 2048]>,
}

impl TxToken for VirtioTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = *self.buffer;
        let result = f(&mut buffer[..len]);
        // TODO: передать пакет в virtio-net TX queue
        result
    }
}

impl Device for VirtioNetDevice {
    type RxToken = VirtioRxToken;
    type TxToken = VirtioTxToken;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken, Self::TxToken)> {
        // TODO: проверить RX virtqueue на наличие пакетов
        // Для теста возвращаем None (нет пакетов)
        None
    }

    fn transmit(&mut self) -> Option<Self::TxToken> {
        // Получить буфер для TX
        self.next_tx_buffer().map(|buf| VirtioTxToken {
            buffer: Box::new(*buf),
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1500;
        caps.max_burst_size = Some(64);
        caps
    }
}

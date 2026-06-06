//! Интеграция virtio-net с smoltcp Device trait

use crate::virtio::Virtqueue;
use alloc::vec::Vec;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

const RX_BUFFER_SIZE: usize = 1514;
const QUEUE_SIZE: usize = 256;

/// Реализация Device trait для virtio-net
pub struct VirtioNetDevice {
    pub rx_queue: Virtqueue,
    pub tx_queue: Virtqueue,
    rx_buffers: Vec<Vec<u8>>,
    rx_buf_idx: usize,
}

impl VirtioNetDevice {
    pub fn new(rx_queue: Virtqueue, tx_queue: Virtqueue) -> Self {
        let mut device = VirtioNetDevice {
            rx_queue,
            tx_queue,
            rx_buffers: Vec::with_capacity(QUEUE_SIZE),
            rx_buf_idx: 0,
        };

        // Выделить RX буферы и добавить их в очередь
        for _ in 0..QUEUE_SIZE {
            device.rx_buffers.push(alloc::vec![0u8; RX_BUFFER_SIZE]);
        }

        // Инициализировать RX очередь — добавить буферы
        unsafe {
            for buf in &mut device.rx_buffers {
                if let Err(e) = device.rx_queue.prepare_rx(buf) {
                    println!("[device] RX buffer preparation failed: {}", e);
                    break;
                }
            }
        }

        device
    }
}

pub struct VirtioRxToken<'a> {
    buf: &'a [u8],
}

pub struct VirtioTxToken<'a> {
    queue: &'a mut Virtqueue,
}

impl<'a> RxToken for VirtioRxToken<'a> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, f: F) -> R {
        let mut buf = alloc::vec![0u8; self.buf.len()];
        buf.copy_from_slice(self.buf);
        f(&mut buf)
    }
}

impl<'a> TxToken for VirtioTxToken<'a> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut buf = alloc::vec![0u8; len];
        let result = f(&mut buf);
        unsafe {
            self.queue.send(&buf);
        }
        result
    }
}

impl Device for VirtioNetDevice {
    type RxToken<'a>
        = VirtioRxToken<'a>
    where
        Self: 'a;

    type TxToken<'a>
        = VirtioTxToken<'a>
    where
        Self: 'a;

    fn receive(&mut self, _: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        // Попробовать получить пакет из RX очереди
        let buf = &mut self.rx_buffers[self.rx_buf_idx];

        unsafe {
            if let Some(len) = self.rx_queue.recv(buf) {
                // Пакет получен
                let rx_token = VirtioRxToken { buf: &buf[..len] };
                let tx_token = VirtioTxToken {
                    queue: &mut self.tx_queue,
                };

                // Перейти к следующему буферу для следующего приема
                self.rx_buf_idx = (self.rx_buf_idx + 1) % self.rx_buffers.len();

                return Some((rx_token, tx_token));
            }
        }

        None
    }

    fn transmit(&mut self, _: Instant) -> Option<Self::TxToken<'_>> {
        Some(VirtioTxToken {
            queue: &mut self.tx_queue,
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = RX_BUFFER_SIZE;
        caps.max_burst_size = Some(1);
        caps
    }
}

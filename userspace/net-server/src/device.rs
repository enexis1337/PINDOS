// userspace/net-server/src/device.rs

use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken, Medium};
use smoltcp::time::Instant;
use crate::virtio::Virtqueue;
use alloc::vec::Vec;

pub struct VirtioNetDevice {
    pub rx_queue: Virtqueue,
    pub tx_queue: Virtqueue,
    pub rx_buf:   [u8; 1514],
}

impl Device for VirtioNetDevice {
    type RxToken<'a> = VirtioRxToken where Self: 'a;
    type TxToken<'a> = VirtioTxToken<'a> where Self: 'a;

    fn receive(&mut self, _: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let len = unsafe { self.rx_queue.recv(&mut self.rx_buf)? };
        let data = self.rx_buf[..len].to_vec();
        Some((
            VirtioRxToken { buf: data },
            VirtioTxToken { queue: &mut self.tx_queue },
        ))
    }

    fn transmit(&mut self, _: Instant) -> Option<Self::TxToken<'_>> {
        Some(VirtioTxToken { queue: &mut self.tx_queue })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

pub struct VirtioRxToken { buf: Vec<u8> }
pub struct VirtioTxToken<'a> { queue: &'a mut Virtqueue }

impl RxToken for VirtioRxToken {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(mut self, f: F) -> R {
        f(&mut self.buf)
    }
}

impl<'a> TxToken for VirtioTxToken<'a> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut buf = alloc::vec![0u8; len];
        let result = f(&mut buf);
        unsafe { self.queue.send(&buf) };
        result
    }
}

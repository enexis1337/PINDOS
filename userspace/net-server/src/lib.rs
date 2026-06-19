//! TCP/IP стек полностью в userspace
//! Ядро предоставляет только raw доступ к NIC через capability
//!
//! Использует smoltcp для TCP/IP обработки
//! Virtio-net для QEMU/KVM

#![no_std]

extern crate alloc;

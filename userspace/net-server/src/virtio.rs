//! Virtio-net драйвер с полной реализацией virtqueue protocol

use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use core::sync::atomic::{fence, Ordering};

const QUEUE_SIZE: usize = 256;

// Регистры virtio-net (legacy, relative to BAR0)
const QUEUE_SEL: u16 = 14;
const QUEUE_PFN: u16 = 8;
const QUEUE_NOTIFY: u16 = 16;
const STATUS: u16 = 18;

// Virtio device status
const DEVICE_RESET: u8 = 0;
const DEVICE_ACKNOWLEDGE: u8 = 1;
const DEVICE_DRIVER: u8 = 2;
const DEVICE_DRIVER_OK: u8 = 4;
const DEVICE_FEATURES_OK: u8 = 8;

/// Descriptor table entry (16 байт)
/// Описывает буфер в памяти, который может быть прочитан или записан устройством
#[repr(C)]
#[derive(Copy, Clone)]
struct VirtqDesc {
    addr: u64,  // физический адрес буфера
    len: u32,   // длина буфера
    flags: u16, // VIRTQ_DESC_F_NEXT=1, VIRTQ_DESC_F_WRITE=2
    next: u16,  // индекс следующего дескриптора в цепи (если NEXT установлен)
}

// Флаги дескриптора
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

/// Available ring — драйвер пишет, устройство читает
/// Информирует устройство о новых буферах в descriptor table
#[repr(C)]
#[derive(Copy, Clone)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE],
}

/// Used ring — устройство пишет, драйвер читает
/// Информирует драйвер о том, какие буферы обработаны
#[repr(C)]
#[derive(Copy, Clone)]
struct VirtqUsedElem {
    id: u32,  // индекс дескриптора
    len: u32, // сколько байт обработано
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; QUEUE_SIZE],
}

/// Виртуальная очередь (virtqueue)
/// Содержит все три структуры: descriptor table, available ring, used ring
pub struct Virtqueue {
    desc: *mut VirtqDesc,
    avail: *mut VirtqAvail,
    used: *mut VirtqUsed,
    free_head: u16, // индекс первого свободного дескриптора
    last_used: u16, // последний обработанный индекс в used ring
    io_base: u16,   // BAR0 адрес (I/O порты)
    queue_idx: u16, // индекс очереди (0=RX, 1=TX для net-server)
    layout: Layout, // для деаллокации
}

impl Virtqueue {
    /// Инициализировать очередь
    /// Выделяет выровненную память, инициализирует структуры и сообщает адрес устройству
    pub unsafe fn init(io_base: u16, queue_idx: u16) -> Self {
        // Размер структур, выровненных до 4096 байт
        let desc_size = core::mem::size_of::<VirtqDesc>() * QUEUE_SIZE;
        let avail_size = core::mem::size_of::<VirtqAvail>();
        let used_size = core::mem::size_of::<VirtqUsed>();
        let total_size = desc_size + avail_size + used_size;

        let layout =
            Layout::from_size_align(total_size, 4096).expect("Invalid layout for virtqueue");

        let ptr = alloc_zeroed(layout);
        if ptr.is_null() {
            panic!("Failed to allocate virtqueue memory");
        }

        let desc = ptr as *mut VirtqDesc;
        let avail = ptr.add(desc_size) as *mut VirtqAvail;
        let used = ptr.add(desc_size + avail_size) as *mut VirtqUsed;

        // Инициализировать free list: каждый дескриптор указывает на следующий
        for i in 0..QUEUE_SIZE - 1 {
            (*desc.add(i)).next = (i + 1) as u16;
            (*desc.add(i)).flags = VIRTQ_DESC_F_NEXT;
        }
        // Последний дескриптор не имеет следующего
        (*desc.add(QUEUE_SIZE - 1)).flags = 0;
        (*desc.add(QUEUE_SIZE - 1)).next = 0;

        // Инициализировать available ring
        (*avail).flags = 0;
        (*avail).idx = 0;

        // Инициализировать used ring
        (*used).flags = 0;
        (*used).idx = 0;

        // Сообщить устройству о физическом адресе очереди
        // Выбрать очередь
        outw(io_base + QUEUE_SEL, queue_idx);
        // Записать адрес (в страницах, разделить на 4096)
        outl(io_base + QUEUE_PFN, ptr as u32 / 4096);

        Virtqueue {
            desc,
            avail,
            used,
            free_head: 0,
            last_used: 0,
            io_base,
            queue_idx,
            layout,
        }
    }

    /// Положить буфер в очередь для отправки (TX)
    /// Добавляет дескриптор в available ring, уведомляет устройство
    pub unsafe fn send(&mut self, data: &[u8]) {
        if self.free_head >= QUEUE_SIZE as u16 {
            return; // Очередь переполнена
        }

        let idx = self.free_head as usize;

        // Получить следующий свободный дескриптор
        self.free_head = (*self.desc.add(idx)).next;

        // Заполнить дескриптор
        (*self.desc.add(idx)).addr = data.as_ptr() as u64;
        (*self.desc.add(idx)).len = data.len() as u32;
        (*self.desc.add(idx)).flags = 0; // TX буфер не требует WRITE флага

        // Добавить индекс в available ring
        let avail_idx = (*self.avail).idx as usize % QUEUE_SIZE;
        (*self.avail).ring[avail_idx] = idx as u16;

        // Memory barrier перед обновлением idx
        fence(Ordering::Release);
        (*self.avail).idx = (*self.avail).idx.wrapping_add(1);
        fence(Ordering::Release);

        // Уведомить устройство
        outw(self.io_base + QUEUE_NOTIFY, self.queue_idx);
    }

    /// Получить буфер из used ring (если есть обработанные пакеты)
    /// Копирует данные в предоставленный буфер
    pub unsafe fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        // Проверить, есть ли новые элементы в used ring
        if (*self.used).idx == self.last_used {
            return None;
        }

        // Получить элемент из used ring
        let elem = &(*self.used).ring[self.last_used as usize % QUEUE_SIZE];
        let desc_idx = elem.id as usize;

        if desc_idx >= QUEUE_SIZE {
            self.last_used = self.last_used.wrapping_add(1);
            return None;
        }

        let desc = &*self.desc.add(desc_idx);
        let len = (elem.len as usize).min(buf.len());

        // Скопировать данные из буфера устройства в предоставленный буфер
        core::ptr::copy_nonoverlapping(desc.addr as *const u8, buf.as_mut_ptr(), len);

        // Обновить last_used
        self.last_used = self.last_used.wrapping_add(1);

        // Вернуть дескриптор в free list
        (*self.desc.add(desc_idx)).next = self.free_head;
        self.free_head = desc_idx as u16;

        Some(len)
    }

    /// Инициализировать RX буфер перед получением данных
    /// Добавляет дескриптор в available ring, готовый для приема
    pub unsafe fn prepare_rx(&mut self, buf: &mut [u8]) -> Result<(), &'static str> {
        if self.free_head >= QUEUE_SIZE as u16 {
            return Err("RX queue full");
        }

        let idx = self.free_head as usize;
        self.free_head = (*self.desc.add(idx)).next;

        // Заполнить дескриптор для RX (устройство будет писать в буфер)
        (*self.desc.add(idx)).addr = buf.as_ptr() as u64;
        (*self.desc.add(idx)).len = buf.len() as u32;
        (*self.desc.add(idx)).flags = VIRTQ_DESC_F_WRITE; // WRITE флаг для RX

        // Добавить в available ring
        let avail_idx = (*self.avail).idx as usize % QUEUE_SIZE;
        (*self.avail).ring[avail_idx] = idx as u16;

        fence(Ordering::Release);
        (*self.avail).idx = (*self.avail).idx.wrapping_add(1);
        fence(Ordering::Release);

        // Уведомить устройство
        outw(self.io_base + QUEUE_NOTIFY, self.queue_idx);

        Ok(())
    }
}

impl Drop for Virtqueue {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.desc as *mut u8, self.layout);
        }
    }
}

/// Инициализировать virtio-net устройство
pub unsafe fn init_device(io_base: u16) {
    // 1. Reset device
    outb(io_base + STATUS, DEVICE_RESET);

    // 2. Set ACKNOWLEDGE
    outb(io_base + STATUS, DEVICE_ACKNOWLEDGE);

    // 3. Set DRIVER
    outb(io_base + STATUS, DEVICE_DRIVER);

    // 4. Set FEATURES_OK (минимальные features для работы)
    outb(io_base + STATUS, DEVICE_FEATURES_OK);

    // 5. Set DRIVER_OK
    outb(io_base + STATUS, DEVICE_DRIVER_OK);
}

/// Написать 8-битное значение в I/O порт
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") val,
        options(nostack)
    );
}

/// Написать 16-битное значение в I/O порт
unsafe fn outw(port: u16, val: u16) {
    core::arch::asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") val,
        options(nostack)
    );
}

/// Написать 32-битное значение в I/O порт
unsafe fn outl(port: u16, val: u32) {
    core::arch::asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") val,
        options(nostack)
    );
}

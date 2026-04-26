// USB UHCI/OHCI/EHCI базовый драйвер
// Обнаружение через PCI, инициализация хост-контроллера, HID устройства

// ── PCI сканирование ──────────────────────────────────────────────────────

const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

unsafe fn pci_out32(port: u16, val: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val);
}

unsafe fn pci_in32(port: u16) -> u32 {
    let v: u32;
    core::arch::asm!("in eax, dx", out("eax") v, in("dx") port);
    v
}

unsafe fn out8(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

unsafe fn in8(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

unsafe fn out16(port: u16, val: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val);
}

unsafe fn in16(port: u16) -> u16 {
    let v: u16;
    core::arch::asm!("in ax, dx", out("ax") v, in("dx") port);
    v
}

pub fn pci_read(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    unsafe {
        pci_out32(PCI_CONFIG_ADDR, addr);
        pci_in32(PCI_CONFIG_DATA)
    }
}

pub fn pci_write(bus: u8, dev: u8, func: u8, offset: u8, val: u32) {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    unsafe {
        pci_out32(PCI_CONFIG_ADDR, addr);
        pci_out32(PCI_CONFIG_DATA, val);
    }
}

#[derive(Copy, Clone)]
pub struct PciDevice {
    pub bus:      u8,
    pub dev:      u8,
    pub func:     u8,
    pub vendor:   u16,
    pub device:   u16,
    pub class:    u8,
    pub subclass: u8,
    pub prog_if:  u8,
    pub bar0:     u32,
}

impl PciDevice {
    pub fn name(&self) -> &'static str {
        match (self.class, self.subclass, self.prog_if) {
            (0x0C, 0x03, 0x00) => "USB UHCI",
            (0x0C, 0x03, 0x10) => "USB OHCI",
            (0x0C, 0x03, 0x20) => "USB EHCI",
            (0x0C, 0x03, 0x30) => "USB xHCI",
            (0x01, 0x01, _)    => "IDE Controller",
            (0x01, 0x06, _)    => "SATA Controller",
            (0x02, 0x00, _)    => "Ethernet",
            (0x03, 0x00, _)    => "VGA Controller",
            (0x04, 0x01, _)    => "Audio (AC97)",
            (0x04, 0x03, _)    => "Audio (HDA)",
            _ => "Unknown",
        }
    }
}

const MAX_PCI_DEVICES: usize = 32;
static mut PCI_DEVICES: [PciDevice; MAX_PCI_DEVICES] = [PciDevice {
    bus: 0, dev: 0, func: 0, vendor: 0, device: 0,
    class: 0, subclass: 0, prog_if: 0, bar0: 0,
}; MAX_PCI_DEVICES];
static mut PCI_COUNT: usize = 0;

pub fn scan_pci() {
    unsafe { PCI_COUNT = 0; }
    for bus in 0u8..=255 {
        for dev in 0u8..32 {
            for func in 0u8..8 {
                let id = pci_read(bus, dev, func, 0x00);
                if id == 0xFFFFFFFF { continue; }

                let vendor = (id & 0xFFFF) as u16;
                let device = (id >> 16) as u16;

                let class_info = pci_read(bus, dev, func, 0x08);
                let class    = (class_info >> 24) as u8;
                let subclass = (class_info >> 16) as u8;
                let prog_if  = (class_info >> 8) as u8;

                let bar0 = pci_read(bus, dev, func, 0x10);

                unsafe {
                    if PCI_COUNT < MAX_PCI_DEVICES {
                        PCI_DEVICES[PCI_COUNT] = PciDevice {
                            bus, dev, func, vendor, device,
                            class, subclass, prog_if, bar0,
                        };
                        PCI_COUNT += 1;
                    }
                }

                // Если не multi-function — пропускаем остальные функции
                let header = pci_read(bus, dev, func, 0x0C);
                if func == 0 && (header >> 16) as u8 & 0x80 == 0 { break; }
            }
        }
    }
}

pub fn pci_device_count() -> usize { unsafe { PCI_COUNT } }

pub fn get_pci_device(idx: usize) -> Option<&'static PciDevice> {
    unsafe { if idx < PCI_COUNT { Some(&PCI_DEVICES[idx]) } else { None } }
}

// ── UHCI (USB 1.1) ────────────────────────────────────────────────────────

// UHCI регистры (I/O порты, base = BAR0 & ~3)
const UHCI_CMD:    u16 = 0x00;
const UHCI_STS:    u16 = 0x02;
const UHCI_INTR:   u16 = 0x04;
const UHCI_FRNUM:  u16 = 0x06;
const UHCI_FLBASE: u16 = 0x08;
const UHCI_SOF:    u16 = 0x0C;
const UHCI_PORTSC1: u16 = 0x10;
const UHCI_PORTSC2: u16 = 0x12;

// Команды
const UHCI_CMD_RS:   u16 = 0x0001; // Run/Stop
const UHCI_CMD_HCRESET: u16 = 0x0002;
const UHCI_CMD_GRESET:  u16 = 0x0004;

pub struct UhciController {
    pub base: u16,
    pub frame_list: u32, // физический адрес frame list
}

impl UhciController {
    pub fn new(base: u16) -> Self {
        UhciController { base, frame_list: 0 }
    }

    pub fn init(&mut self) {
        unsafe {
            // Global reset
            out16(self.base + UHCI_CMD, UHCI_CMD_GRESET);
            // Задержка ~10ms
            for _ in 0..100000u32 { out8(0x80, 0); }
            out16(self.base + UHCI_CMD, 0);

            // Host controller reset
            out16(self.base + UHCI_CMD, UHCI_CMD_HCRESET);
            let mut timeout = 10000u32;
            while timeout > 0 {
                if in16(self.base + UHCI_CMD) & UHCI_CMD_HCRESET == 0 { break; }
                timeout -= 1;
            }

            // Отключаем прерывания
            out16(self.base + UHCI_INTR, 0);

            // Очищаем статус
            out16(self.base + UHCI_STS, 0xFFFF);

            // SOF = 64 (1ms frame)
            out8(self.base + UHCI_SOF, 0x40);

            // Выделяем frame list (1024 * 4 байта = 4KB, выровнено на 4KB)
            // Используем статический буфер
            self.frame_list = UHCI_FRAME_LIST.as_ptr() as u32;

            // Заполняем frame list терминаторами
            for i in 0..1024 {
                UHCI_FRAME_LIST[i] = 0x00000001; // T=1 (terminate)
            }

            out16(self.base + UHCI_FRNUM, 0);
            // Записываем адрес frame list (должен быть выровнен на 4KB)
            let fl_addr = self.frame_list & 0xFFFFF000;
            // Записываем как 32-bit через два 16-bit
            out16(self.base + UHCI_FLBASE,     (fl_addr & 0xFFFF) as u16);
            out16(self.base + UHCI_FLBASE + 2, (fl_addr >> 16) as u16);

            // Запускаем контроллер
            out16(self.base + UHCI_CMD, UHCI_CMD_RS);
        }
    }

    /// Проверяет порты на подключённые устройства
    pub fn probe_ports(&self) -> [bool; 2] {
        let mut connected = [false; 2];
        for i in 0..2 {
            let portsc = unsafe { in16(self.base + UHCI_PORTSC1 + i as u16 * 2) };
            connected[i] = portsc & 0x0001 != 0; // Current Connect Status
        }
        connected
    }

    /// Сброс порта
    pub fn reset_port(&self, port: u8) {
        let portsc_reg = self.base + UHCI_PORTSC1 + port as u16 * 2;
        unsafe {
            // Port Reset
            out16(portsc_reg, in16(portsc_reg) | 0x0200);
            for _ in 0..50000u32 { out8(0x80, 0); } // 50ms
            out16(portsc_reg, in16(portsc_reg) & !0x0200);
            for _ in 0..10000u32 { out8(0x80, 0); } // 10ms
            // Enable port
            out16(portsc_reg, in16(portsc_reg) | 0x0004);
        }
    }
}

static mut UHCI_FRAME_LIST: [u32; 1024] = [0u32; 1024];

// ── USB HID (клавиатура/мышь) ─────────────────────────────────────────────

// USB HID keycodes → ASCII (упрощённая таблица)
static HID_TO_ASCII: &[u8] = &[
//  0     1     2     3     4     5     6     7     8     9     A     B     C     D     E     F
    0,    0,    0,    0,    b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l',
    b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z',
    b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
    b'\n', 0x1B, b'\x08', b'\t', b' ',
    b'-', b'=', b'[', b']', b'\\', 0, b';', b'\'', b'`', b',', b'.', b'/',
];

pub fn hid_keycode_to_ascii(keycode: u8, shift: bool) -> Option<u8> {
    if (keycode as usize) < HID_TO_ASCII.len() {
        let c = HID_TO_ASCII[keycode as usize];
        if c == 0 { return None; }
        if shift && c >= b'a' && c <= b'z' {
            return Some(c - 32);
        }
        Some(c)
    } else {
        None
    }
}

// ── Инициализация USB ─────────────────────────────────────────────────────

static mut USB_CONTROLLER: Option<UhciController> = None;

pub fn init() {
    scan_pci();

    // Ищем UHCI контроллер
    for i in 0..pci_device_count() {
        if let Some(dev) = get_pci_device(i) {
            if dev.class == 0x0C && dev.subclass == 0x03 && dev.prog_if == 0x00 {
                // UHCI — BAR4 это I/O base
                let bar4 = pci_read(dev.bus, dev.dev, dev.func, 0x20);
                let io_base = (bar4 & 0xFFFC) as u16;

                if io_base > 0 {
                    let mut ctrl = UhciController::new(io_base);
                    ctrl.init();

                    let ports = ctrl.probe_ports();
                    for (i, &connected) in ports.iter().enumerate() {
                        if connected {
                            ctrl.reset_port(i as u8);
                        }
                    }

                    unsafe { USB_CONTROLLER = Some(ctrl); }
                    return;
                }
            }
        }
    }
}

pub fn get_controller() -> Option<&'static UhciController> {
    unsafe { USB_CONTROLLER.as_ref() }
}

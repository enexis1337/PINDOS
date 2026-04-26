// EHCI — USB 2.0 Host Controller
// Спецификация: Intel EHCI spec 1.0
// Работает через MMIO (Memory-Mapped I/O), адрес из PCI BAR0

use super::usb::pci_read;

// ── EHCI Capability Registers (относительно base) ─────────────────────────
const CAP_CAPLENGTH:  u32 = 0x00; // длина capability регистров (1 байт)
const CAP_HCIVERSION: u32 = 0x02; // версия (2 байта)
const CAP_HCSPARAMS:  u32 = 0x04; // structural params
const CAP_HCCPARAMS:  u32 = 0x08; // capability params

// ── EHCI Operational Registers (base + CAPLENGTH) ─────────────────────────
const OP_USBCMD:    u32 = 0x00;
const OP_USBSTS:    u32 = 0x04;
const OP_USBINTR:   u32 = 0x08;
const OP_FRINDEX:   u32 = 0x0C;
const OP_CTRLDSSEG: u32 = 0x10;
const OP_PERIODICLISTBASE: u32 = 0x14;
const OP_ASYNCLISTADDR:    u32 = 0x18;
const OP_CONFIGFLAG: u32 = 0x40;
const OP_PORTSC_BASE: u32 = 0x44; // PORTSC[0..N]

// USBCMD биты
const CMD_RUN:       u32 = 1 << 0;
const CMD_HCRESET:   u32 = 1 << 1;
const CMD_FLS_1024:  u32 = 0 << 2; // frame list size 1024
const CMD_PSE:       u32 = 1 << 4; // periodic schedule enable
const CMD_ASE:       u32 = 1 << 5; // async schedule enable
const CMD_IAAD:      u32 = 1 << 6; // interrupt on async advance doorbell

// USBSTS биты
const STS_HALTED:    u32 = 1 << 12;

// PORTSC биты
const PORT_CONNECT:  u32 = 1 << 0;
const PORT_ENABLE:   u32 = 1 << 2;
const PORT_RESET:    u32 = 1 << 8;
const PORT_POWER:    u32 = 1 << 12;
const PORT_OWNER:    u32 = 1 << 13; // 0=EHCI, 1=companion (UHCI/OHCI)
const PORT_SPEED:    u32 = 3 << 26; // 00=FS, 01=LS, 10=HS

unsafe fn mmio_read32(addr: u32) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

unsafe fn mmio_write32(addr: u32, val: u32) {
    core::ptr::write_volatile(addr as *mut u32, val);
}

unsafe fn mmio_read8(addr: u32) -> u8 {
    core::ptr::read_volatile(addr as *const u8)
}

fn delay_ms(ms: u32) {
    for _ in 0..ms * 10000 {
        unsafe { core::arch::asm!("nop"); }
    }
}

// ── QH (Queue Head) и qTD (Queue element Transfer Descriptor) ─────────────

#[repr(C, align(32))]
struct Qtd {
    next:       u32,  // следующий qTD (бит 0 = terminate)
    alt_next:   u32,  // alternate next
    token:      u32,  // статус, PID, длина
    buf:        [u32; 5], // буферы данных
}

#[repr(C, align(32))]
struct Qh {
    next:       u32,  // следующий QH (тип = 01 QH)
    endpoint:   u32,  // endpoint характеристики
    caps:       u32,  // endpoint capabilities
    cur_qtd:    u32,  // текущий qTD
    // overlay (копия qTD)
    next_qtd:   u32,
    alt_qtd:    u32,
    token:      u32,
    buf:        [u32; 5],
}

// Статические буферы (выровнены на 32 байта)
static mut EHCI_QH:  Qh  = Qh  { next: 1, endpoint: 0, caps: 0, cur_qtd: 0, next_qtd: 1, alt_qtd: 1, token: 0, buf: [0;5] };
static mut EHCI_QTD: Qtd = Qtd { next: 1, alt_next: 1, token: 0, buf: [0;5] };
static mut EHCI_BUF: [u8; 4096] = [0u8; 4096];
static mut PERIODIC_LIST: [u32; 1024] = [0u32; 1024];

// ── Контроллер ────────────────────────────────────────────────────────────

pub struct EhciController {
    pub cap_base: u32,  // база capability регистров
    pub op_base:  u32,  // база operational регистров
    pub port_count: u8,
    pub initialized: bool,
}

impl EhciController {
    pub fn new(mmio_base: u32) -> Self {
        let cap_len = unsafe { mmio_read8(mmio_base) } as u32;
        EhciController {
            cap_base: mmio_base,
            op_base:  mmio_base + cap_len,
            port_count: 0,
            initialized: false,
        }
    }

    pub fn init(&mut self) -> bool {
        unsafe {
            // Читаем количество портов
            let hcsparams = mmio_read32(self.cap_base + CAP_HCSPARAMS);
            self.port_count = (hcsparams & 0x0F) as u8;

            // Сброс контроллера
            mmio_write32(self.op_base + OP_USBCMD, CMD_HCRESET);
            let mut timeout = 100000u32;
            while timeout > 0 {
                if mmio_read32(self.op_base + OP_USBCMD) & CMD_HCRESET == 0 { break; }
                timeout -= 1;
            }
            if timeout == 0 { return false; }

            // Отключаем прерывания
            mmio_write32(self.op_base + OP_USBINTR, 0);

            // Очищаем статус
            mmio_write32(self.op_base + OP_USBSTS, 0x3F);

            // Настраиваем periodic list (заполняем терминаторами)
            for i in 0..1024 {
                PERIODIC_LIST[i] = 0x00000001; // T=1
            }
            mmio_write32(self.op_base + OP_PERIODICLISTBASE,
                PERIODIC_LIST.as_ptr() as u32);

            // Настраиваем async list — пустой QH указывает на себя
            EHCI_QH.next = ((&EHCI_QH as *const Qh as u32) & !0x1F) | 0x02; // тип QH
            EHCI_QH.endpoint = (1 << 15); // H bit — head of reclamation list
            mmio_write32(self.op_base + OP_ASYNCLISTADDR,
                &EHCI_QH as *const Qh as u32);

            // Segment = 0 (32-bit адресация)
            mmio_write32(self.op_base + OP_CTRLDSSEG, 0);

            // Запускаем контроллер
            mmio_write32(self.op_base + OP_USBCMD,
                CMD_RUN | CMD_FLS_1024 | CMD_PSE | CMD_ASE);

            delay_ms(10);

            // Проверяем что запустился
            if mmio_read32(self.op_base + OP_USBSTS) & STS_HALTED != 0 {
                return false;
            }

            // Передаём управление EHCI (ConfigFlag = 1)
            mmio_write32(self.op_base + OP_CONFIGFLAG, 1);
            delay_ms(5);

            self.initialized = true;
            true
        }
    }

    /// Включает питание и сбрасывает порт
    pub fn reset_port(&self, port: u8) -> PortSpeed {
        let portsc_addr = self.op_base + OP_PORTSC_BASE + port as u32 * 4;
        unsafe {
            // Включаем питание
            let val = mmio_read32(portsc_addr);
            if val & PORT_POWER == 0 {
                mmio_write32(portsc_addr, val | PORT_POWER);
                delay_ms(20);
            }

            // Проверяем подключение
            if mmio_read32(portsc_addr) & PORT_CONNECT == 0 {
                return PortSpeed::None;
            }

            // Port Reset
            let val = mmio_read32(portsc_addr);
            mmio_write32(portsc_addr, (val | PORT_RESET) & !PORT_ENABLE);
            delay_ms(50); // USB spec: минимум 50ms

            // Снимаем reset
            let val = mmio_read32(portsc_addr);
            mmio_write32(portsc_addr, val & !PORT_RESET);
            delay_ms(10);

            // Ждём enable
            let mut timeout = 10000u32;
            while timeout > 0 {
                let s = mmio_read32(portsc_addr);
                if s & PORT_ENABLE != 0 { break; }
                timeout -= 1;
            }

            let portsc = mmio_read32(portsc_addr);

            // Если порт не включился — это Low/Full Speed устройство
            // Передаём companion контроллеру (UHCI/OHCI)
            if portsc & PORT_ENABLE == 0 {
                mmio_write32(portsc_addr, portsc | PORT_OWNER);
                return PortSpeed::FullOrLow;
            }

            // High Speed (USB 2.0)
            PortSpeed::High
        }
    }

    pub fn probe_ports(&self) -> [PortSpeed; 8] {
        let mut speeds = [PortSpeed::None; 8];
        for i in 0..self.port_count.min(8) {
            let portsc = unsafe {
                mmio_read32(self.op_base + OP_PORTSC_BASE + i as u32 * 4)
            };
            speeds[i as usize] = if portsc & PORT_CONNECT == 0 {
                PortSpeed::None
            } else {
                let speed_bits = (portsc >> 26) & 3;
                match speed_bits {
                    0 => PortSpeed::Full,
                    1 => PortSpeed::Low,
                    2 => PortSpeed::High,
                    _ => PortSpeed::None,
                }
            };
        }
        speeds
    }

    /// Выполняет Control Transfer (для инициализации устройств)
    pub fn control_transfer(
        &self,
        addr: u8,
        setup: &[u8; 8],
        data: Option<&mut [u8]>,
        data_in: bool,
    ) -> bool {
        unsafe {
            // Настраиваем SETUP qTD
            let setup_buf_addr = EHCI_BUF.as_ptr() as u32;
            core::ptr::copy_nonoverlapping(setup.as_ptr(), EHCI_BUF.as_mut_ptr(), 8);

            // SETUP token: PID=SETUP(0x02), 8 байт, IOC=0
            EHCI_QTD.token = (8 << 16) | (0x02 << 8) | 0x80; // active
            EHCI_QTD.buf[0] = setup_buf_addr;
            EHCI_QTD.next = 1; // terminate

            // Настраиваем QH
            EHCI_QH.endpoint = (addr as u32)           // device address
                | (0 << 8)                              // endpoint 0
                | (1 << 13)                             // data toggle control
                | (64 << 16)                            // max packet size
                | (2 << 12);                            // speed: HS
            EHCI_QH.next_qtd = &EHCI_QTD as *const Qtd as u32;
            EHCI_QH.token = 0;

            // Ждём завершения (упрощённо — polling)
            let mut timeout = 100000u32;
            while timeout > 0 {
                let tok = core::ptr::read_volatile(&EHCI_QTD.token);
                if tok & 0x80 == 0 { break; } // active бит снят
                timeout -= 1;
            }

            timeout > 0
        }
    }

    /// Читает дескриптор устройства
    pub fn get_device_descriptor(&self, addr: u8, buf: &mut [u8]) -> bool {
        // GET_DESCRIPTOR, Device, index=0, lang=0, length=18
        let setup: [u8; 8] = [0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 18, 0x00];
        self.control_transfer(addr, &setup, Some(buf), true)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PortSpeed { None, Low, Full, FullOrLow, High }

// ── Глобальный EHCI контроллер ────────────────────────────────────────────

static mut EHCI: Option<EhciController> = None;

pub fn init(mmio_base: u32) -> bool {
    let mut ctrl = EhciController::new(mmio_base);
    let ok = ctrl.init();
    if ok {
        unsafe { EHCI = Some(ctrl); }
    }
    ok
}

pub fn get() -> Option<&'static EhciController> {
    unsafe { EHCI.as_ref() }
}

/// Находит EHCI контроллер через PCI и инициализирует
pub fn init_from_pci() -> bool {
    for bus in 0u8..=255 {
        for dev in 0u8..32 {
            for func in 0u8..8 {
                let id = super::usb::pci_read(bus, dev, func, 0x00);
                if id == 0xFFFFFFFF { continue; }

                let class_info = super::usb::pci_read(bus, dev, func, 0x08);
                let class    = (class_info >> 24) as u8;
                let subclass = (class_info >> 16) as u8;
                let prog_if  = (class_info >> 8) as u8;

                if class == 0x0C && subclass == 0x03 && prog_if == 0x20 {
                    // EHCI найден — читаем BAR0 (MMIO)
                    let bar0 = super::usb::pci_read(bus, dev, func, 0x10);
                    let mmio = bar0 & 0xFFFFFFF0;

                    if mmio == 0 { continue; }

                    // Включаем Bus Master и Memory Space в PCI Command
                    let cmd = super::usb::pci_read(bus, dev, func, 0x04);
                    super::usb::pci_write(bus, dev, func, 0x04, cmd | 0x06);

                    if init(mmio) {
                        return true;
                    }
                }

                let header = super::usb::pci_read(bus, dev, func, 0x0C);
                if func == 0 && (header >> 16) as u8 & 0x80 == 0 { break; }
            }
        }
    }
    false
}

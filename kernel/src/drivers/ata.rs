// ATA/IDE драйвер — PIO режим
// Поддерживает: чтение/запись секторов, LBA28, LBA48
// Шины: Primary (0x1F0), Secondary (0x170)

// Primary ATA
const ATA0_DATA:    u16 = 0x1F0;
const ATA0_ERROR:   u16 = 0x1F1;
const ATA0_SECTORS: u16 = 0x1F2;
const ATA0_LBA_LO:  u16 = 0x1F3;
const ATA0_LBA_MID: u16 = 0x1F4;
const ATA0_LBA_HI:  u16 = 0x1F5;
const ATA0_DRIVE:   u16 = 0x1F6;
const ATA0_STATUS:  u16 = 0x1F7;
const ATA0_CMD:     u16 = 0x1F7;
const ATA0_CTRL:    u16 = 0x3F6;

// Secondary ATA
const ATA1_DATA:    u16 = 0x170;
const ATA1_STATUS:  u16 = 0x177;
const ATA1_CMD:     u16 = 0x177;
const ATA1_DRIVE:   u16 = 0x176;
const ATA1_SECTORS: u16 = 0x172;
const ATA1_LBA_LO:  u16 = 0x173;
const ATA1_LBA_MID: u16 = 0x174;
const ATA1_LBA_HI:  u16 = 0x175;
const ATA1_CTRL:    u16 = 0x376;

// Статусные биты
const STATUS_ERR:  u8 = 0x01;
const STATUS_DRQ:  u8 = 0x08; // Data Request
const STATUS_SRV:  u8 = 0x10;
const STATUS_DF:   u8 = 0x20; // Drive Fault
const STATUS_RDY:  u8 = 0x40;
const STATUS_BSY:  u8 = 0x80; // Busy

// Команды
const CMD_READ_PIO:    u8 = 0x20;
const CMD_READ_PIO_EXT: u8 = 0x24; // LBA48
const CMD_WRITE_PIO:   u8 = 0x30;
const CMD_WRITE_PIO_EXT: u8 = 0x34;
const CMD_CACHE_FLUSH: u8 = 0xE7;
const CMD_IDENTIFY:    u8 = 0xEC;

unsafe fn out8(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

unsafe fn out16(port: u16, val: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val);
}

unsafe fn in8(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

unsafe fn in16(port: u16) -> u16 {
    let v: u16;
    core::arch::asm!("in ax, dx", out("ax") v, in("dx") port);
    v
}

#[derive(Copy, Clone, PartialEq)]
pub enum AtaBus { Primary, Secondary }

#[derive(Copy, Clone, PartialEq)]
pub enum AtaDrive { Master, Slave }

#[derive(Copy, Clone)]
pub struct AtaDisk {
    pub bus:     AtaBus,
    pub drive:   AtaDrive,
    pub present: bool,
    pub lba48:   bool,
    pub sectors: u64,
    pub model:   [u8; 40],
}

impl AtaDisk {
    const fn empty() -> Self {
        AtaDisk {
            bus: AtaBus::Primary, drive: AtaDrive::Master,
            present: false, lba48: false, sectors: 0,
            model: [0u8; 40],
        }
    }

    pub fn model_str(&self) -> &str {
        let mut end = 40;
        while end > 0 && (self.model[end-1] == b' ' || self.model[end-1] == 0) { end -= 1; }
        core::str::from_utf8(&self.model[..end]).unwrap_or("Unknown")
    }
}

static mut DISKS: [AtaDisk; 4] = [AtaDisk::empty(); 4];
static mut DISK_COUNT: usize = 0;

fn bus_ports(bus: AtaBus) -> (u16, u16, u16, u16, u16, u16, u16, u16) {
    match bus {
        AtaBus::Primary   => (ATA0_DATA, ATA0_ERROR, ATA0_SECTORS, ATA0_LBA_LO, ATA0_LBA_MID, ATA0_LBA_HI, ATA0_DRIVE, ATA0_STATUS),
        AtaBus::Secondary => (ATA1_DATA, ATA1_STATUS, ATA1_SECTORS, ATA1_LBA_LO, ATA1_LBA_MID, ATA1_LBA_HI, ATA1_DRIVE, ATA1_STATUS),
    }
}

fn ctrl_port(bus: AtaBus) -> u16 {
    match bus { AtaBus::Primary => ATA0_CTRL, AtaBus::Secondary => ATA1_CTRL }
}

fn wait_bsy(bus: AtaBus) {
    let status_port = match bus { AtaBus::Primary => ATA0_STATUS, AtaBus::Secondary => ATA1_STATUS };
    let mut timeout = 1000000u32;
    while timeout > 0 {
        let s = unsafe { in8(status_port) };
        if s & STATUS_BSY == 0 { return; }
        timeout -= 1;
    }
}

fn wait_drq(bus: AtaBus) -> bool {
    let status_port = match bus { AtaBus::Primary => ATA0_STATUS, AtaBus::Secondary => ATA1_STATUS };
    let mut timeout = 1000000u32;
    while timeout > 0 {
        let s = unsafe { in8(status_port) };
        if s & STATUS_ERR != 0 || s & STATUS_DF != 0 { return false; }
        if s & STATUS_DRQ != 0 { return true; }
        timeout -= 1;
    }
    false
}

fn select_drive(bus: AtaBus, drive: AtaDrive, lba_top: u8) {
    let drive_port = match bus { AtaBus::Primary => ATA0_DRIVE, AtaBus::Secondary => ATA1_DRIVE };
    let drive_bit = match drive { AtaDrive::Master => 0u8, AtaDrive::Slave => 0x10 };
    unsafe { out8(drive_port, 0xE0 | drive_bit | (lba_top & 0x0F)); }
    // 400ns задержка (читаем статус 4 раза)
    let ctrl = ctrl_port(bus);
    for _ in 0..4 { unsafe { let _ = in8(ctrl); } }
}

// ── Инициализация ─────────────────────────────────────────────────────────

pub fn init() {
    unsafe { DISK_COUNT = 0; }

    for &bus in &[AtaBus::Primary, AtaBus::Secondary] {
        for &drive in &[AtaDrive::Master, AtaDrive::Slave] {
            if let Some(disk) = identify(bus, drive) {
                unsafe {
                    if DISK_COUNT < 4 {
                        DISKS[DISK_COUNT] = disk;
                        DISK_COUNT += 1;
                    }
                }
            }
        }
    }
}

fn identify(bus: AtaBus, drive: AtaDrive) -> Option<AtaDisk> {
    let status_port = match bus { AtaBus::Primary => ATA0_STATUS, AtaBus::Secondary => ATA1_STATUS };
    let cmd_port    = match bus { AtaBus::Primary => ATA0_CMD,    AtaBus::Secondary => ATA1_CMD };
    let data_port   = match bus { AtaBus::Primary => ATA0_DATA,   AtaBus::Secondary => ATA1_DATA };
    let sec_port    = match bus { AtaBus::Primary => ATA0_SECTORS, AtaBus::Secondary => ATA1_SECTORS };
    let lba_lo      = match bus { AtaBus::Primary => ATA0_LBA_LO, AtaBus::Secondary => ATA1_LBA_LO };
    let lba_mid     = match bus { AtaBus::Primary => ATA0_LBA_MID, AtaBus::Secondary => ATA1_LBA_MID };
    let lba_hi      = match bus { AtaBus::Primary => ATA0_LBA_HI, AtaBus::Secondary => ATA1_LBA_HI };

    select_drive(bus, drive, 0);

    unsafe {
        // Проверяем что диск существует
        let s = in8(status_port);
        if s == 0xFF { return None; } // floating bus

        // Отправляем IDENTIFY
        out8(sec_port, 0);
        out8(lba_lo, 0);
        out8(lba_mid, 0);
        out8(lba_hi, 0);
        out8(cmd_port, CMD_IDENTIFY);

        let s = in8(status_port);
        if s == 0 { return None; } // нет диска

        wait_bsy(bus);

        // Проверяем что это ATA (не ATAPI)
        let mid = in8(lba_mid);
        let hi  = in8(lba_hi);
        if mid != 0 || hi != 0 { return None; } // ATAPI

        if !wait_drq(bus) { return None; }

        // Читаем 256 слов (512 байт) идентификации
        let mut id = [0u16; 256];
        for i in 0..256 {
            id[i] = in16(data_port);
        }

        // Парсим данные
        let lba48 = id[83] & (1 << 10) != 0;
        let sectors = if lba48 {
            (id[100] as u64) | ((id[101] as u64) << 16) |
            ((id[102] as u64) << 32) | ((id[103] as u64) << 48)
        } else {
            (id[60] as u64) | ((id[61] as u64) << 16)
        };

        // Модель (слова 27-46, big-endian байты)
        let mut model = [0u8; 40];
        for i in 0..20 {
            let word = id[27 + i];
            model[i*2]   = (word >> 8) as u8;
            model[i*2+1] = (word & 0xFF) as u8;
        }

        Some(AtaDisk { bus, drive, present: true, lba48, sectors, model })
    }
}

pub fn disk_count() -> usize { unsafe { DISK_COUNT } }

pub fn get_disk(idx: usize) -> Option<&'static AtaDisk> {
    unsafe {
        if idx < DISK_COUNT { Some(&DISKS[idx]) } else { None }
    }
}

// ── Чтение/запись секторов ────────────────────────────────────────────────

pub enum AtaError { Timeout, DriveFault, Error, OutOfRange }

/// Читает `count` секторов начиная с LBA `lba` в буфер `buf`
pub fn read_sectors(disk: &AtaDisk, lba: u64, count: u16, buf: &mut [u8]) -> Result<(), AtaError> {
    if buf.len() < count as usize * 512 { return Err(AtaError::OutOfRange); }

    let bus = disk.bus;
    let cmd_port    = match bus { AtaBus::Primary => ATA0_CMD,    AtaBus::Secondary => ATA1_CMD };
    let data_port   = match bus { AtaBus::Primary => ATA0_DATA,   AtaBus::Secondary => ATA1_DATA };
    let status_port = match bus { AtaBus::Primary => ATA0_STATUS, AtaBus::Secondary => ATA1_STATUS };
    let sec_port    = match bus { AtaBus::Primary => ATA0_SECTORS, AtaBus::Secondary => ATA1_SECTORS };
    let lba_lo      = match bus { AtaBus::Primary => ATA0_LBA_LO, AtaBus::Secondary => ATA1_LBA_LO };
    let lba_mid     = match bus { AtaBus::Primary => ATA0_LBA_MID, AtaBus::Secondary => ATA1_LBA_MID };
    let lba_hi      = match bus { AtaBus::Primary => ATA0_LBA_HI, AtaBus::Secondary => ATA1_LBA_HI };

    wait_bsy(bus);

    if disk.lba48 {
        // LBA48
        let drive_bit = match disk.drive { AtaDrive::Master => 0u8, AtaDrive::Slave => 0x10 };
        unsafe {
            out8(match bus { AtaBus::Primary => ATA0_DRIVE, AtaBus::Secondary => ATA1_DRIVE },
                 0x40 | drive_bit);
            // Старшие байты
            out8(sec_port, (count >> 8) as u8);
            out8(lba_lo,   ((lba >> 24) & 0xFF) as u8);
            out8(lba_mid,  ((lba >> 32) & 0xFF) as u8);
            out8(lba_hi,   ((lba >> 40) & 0xFF) as u8);
            // Младшие байты
            out8(sec_port, (count & 0xFF) as u8);
            out8(lba_lo,   (lba & 0xFF) as u8);
            out8(lba_mid,  ((lba >> 8) & 0xFF) as u8);
            out8(lba_hi,   ((lba >> 16) & 0xFF) as u8);
            out8(cmd_port, CMD_READ_PIO_EXT);
        }
    } else {
        // LBA28
        let lba_top = ((lba >> 24) & 0x0F) as u8;
        select_drive(bus, disk.drive, lba_top);
        unsafe {
            out8(sec_port, count as u8);
            out8(lba_lo,   (lba & 0xFF) as u8);
            out8(lba_mid,  ((lba >> 8) & 0xFF) as u8);
            out8(lba_hi,   ((lba >> 16) & 0xFF) as u8);
            out8(cmd_port, CMD_READ_PIO);
        }
    }

    // Читаем секторы
    for sector in 0..count as usize {
        wait_bsy(bus);
        if !wait_drq(bus) {
            let s = unsafe { in8(status_port) };
            if s & STATUS_DF != 0 { return Err(AtaError::DriveFault); }
            return Err(AtaError::Error);
        }

        let offset = sector * 512;
        unsafe {
            for i in 0..256 {
                let word = in16(data_port);
                buf[offset + i*2]   = (word & 0xFF) as u8;
                buf[offset + i*2+1] = (word >> 8) as u8;
            }
        }
    }

    Ok(())
}

/// Записывает `count` секторов начиная с LBA `lba` из буфера `buf`
pub fn write_sectors(disk: &AtaDisk, lba: u64, count: u16, buf: &[u8]) -> Result<(), AtaError> {
    if buf.len() < count as usize * 512 { return Err(AtaError::OutOfRange); }

    let bus = disk.bus;
    let cmd_port  = match bus { AtaBus::Primary => ATA0_CMD,    AtaBus::Secondary => ATA1_CMD };
    let data_port = match bus { AtaBus::Primary => ATA0_DATA,   AtaBus::Secondary => ATA1_DATA };
    let sec_port  = match bus { AtaBus::Primary => ATA0_SECTORS, AtaBus::Secondary => ATA1_SECTORS };
    let lba_lo    = match bus { AtaBus::Primary => ATA0_LBA_LO, AtaBus::Secondary => ATA1_LBA_LO };
    let lba_mid   = match bus { AtaBus::Primary => ATA0_LBA_MID, AtaBus::Secondary => ATA1_LBA_MID };
    let lba_hi    = match bus { AtaBus::Primary => ATA0_LBA_HI, AtaBus::Secondary => ATA1_LBA_HI };

    wait_bsy(bus);

    let lba_top = ((lba >> 24) & 0x0F) as u8;
    select_drive(bus, disk.drive, lba_top);

    unsafe {
        out8(sec_port, count as u8);
        out8(lba_lo,   (lba & 0xFF) as u8);
        out8(lba_mid,  ((lba >> 8) & 0xFF) as u8);
        out8(lba_hi,   ((lba >> 16) & 0xFF) as u8);
        out8(cmd_port, CMD_WRITE_PIO);
    }

    for sector in 0..count as usize {
        wait_bsy(bus);
        if !wait_drq(bus) { return Err(AtaError::Error); }

        let offset = sector * 512;
        unsafe {
            for i in 0..256 {
                let word = (buf[offset + i*2] as u16) | ((buf[offset + i*2+1] as u16) << 8);
                out16(data_port, word);
            }
            // Flush cache
            out8(cmd_port, CMD_CACHE_FLUSH);
        }
        wait_bsy(bus);
    }

    Ok(())
}

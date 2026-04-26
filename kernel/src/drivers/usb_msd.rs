// USB Mass Storage Device (MSD) — Bulk-Only Transport
// Позволяет читать/писать USB флешки как блочное устройство
// Используется поверх EHCI/UHCI

// Bulk-Only Transport команды
const CBW_SIGNATURE: u32 = 0x43425355; // 'USBC'
const CSW_SIGNATURE: u32 = 0x53425355; // 'USBS'

const CBW_FLAG_IN:  u8 = 0x80; // device → host
const CBW_FLAG_OUT: u8 = 0x00; // host → device

// SCSI команды
const SCSI_TEST_UNIT_READY: u8 = 0x00;
const SCSI_REQUEST_SENSE:   u8 = 0x03;
const SCSI_INQUIRY:         u8 = 0x12;
const SCSI_READ_CAPACITY:   u8 = 0x25;
const SCSI_READ10:          u8 = 0x28;
const SCSI_WRITE10:         u8 = 0x2A;

#[repr(C, packed)]
struct Cbw {
    signature:  u32,
    tag:        u32,
    data_len:   u32,
    flags:      u8,
    lun:        u8,
    cb_len:     u8,
    cb:         [u8; 16],
}

#[repr(C, packed)]
struct Csw {
    signature:  u32,
    tag:        u32,
    residue:    u32,
    status:     u8,
}

pub struct UsbMsd {
    pub sector_size:  u32,
    pub sector_count: u64,
    pub vendor:       [u8; 8],
    pub product:      [u8; 16],
    tag_counter: u32,
}

impl UsbMsd {
    pub fn new() -> Self {
        UsbMsd {
            sector_size: 512,
            sector_count: 0,
            vendor: [0u8; 8],
            product: [0u8; 16],
            tag_counter: 1,
        }
    }

    /// Инициализация — INQUIRY + READ CAPACITY
    pub fn init(&mut self) -> bool {
        // INQUIRY
        let mut inq_buf = [0u8; 36];
        if self.scsi_inquiry(&mut inq_buf) {
            self.vendor[..8].copy_from_slice(&inq_buf[8..16]);
            self.product[..16].copy_from_slice(&inq_buf[16..32]);
        }

        // READ CAPACITY
        let mut cap_buf = [0u8; 8];
        if self.scsi_read_capacity(&mut cap_buf) {
            self.sector_count = u32::from_be_bytes([cap_buf[0], cap_buf[1], cap_buf[2], cap_buf[3]]) as u64 + 1;
            self.sector_size  = u32::from_be_bytes([cap_buf[4], cap_buf[5], cap_buf[6], cap_buf[7]]);
            return true;
        }
        false
    }

    pub fn vendor_str(&self) -> &str {
        let mut end = 8;
        while end > 0 && (self.vendor[end-1] == b' ' || self.vendor[end-1] == 0) { end -= 1; }
        core::str::from_utf8(&self.vendor[..end]).unwrap_or("Unknown")
    }

    pub fn product_str(&self) -> &str {
        let mut end = 16;
        while end > 0 && (self.product[end-1] == b' ' || self.product[end-1] == 0) { end -= 1; }
        core::str::from_utf8(&self.product[..end]).unwrap_or("Unknown")
    }

    /// Читает `count` секторов начиная с LBA `lba`
    pub fn read_sectors(&mut self, lba: u64, count: u16, buf: &mut [u8]) -> bool {
        let mut cb = [0u8; 16];
        cb[0] = SCSI_READ10;
        cb[2] = ((lba >> 24) & 0xFF) as u8;
        cb[3] = ((lba >> 16) & 0xFF) as u8;
        cb[4] = ((lba >> 8)  & 0xFF) as u8;
        cb[5] = (lba & 0xFF) as u8;
        cb[7] = ((count >> 8) & 0xFF) as u8;
        cb[8] = (count & 0xFF) as u8;

        let data_len = count as u32 * self.sector_size;
        self.bulk_transfer(&cb, 10, CBW_FLAG_IN, data_len, Some(buf))
    }

    /// Записывает `count` секторов начиная с LBA `lba`
    pub fn write_sectors(&mut self, lba: u64, count: u16, buf: &[u8]) -> bool {
        let mut cb = [0u8; 16];
        cb[0] = SCSI_WRITE10;
        cb[2] = ((lba >> 24) & 0xFF) as u8;
        cb[3] = ((lba >> 16) & 0xFF) as u8;
        cb[4] = ((lba >> 8)  & 0xFF) as u8;
        cb[5] = (lba & 0xFF) as u8;
        cb[7] = ((count >> 8) & 0xFF) as u8;
        cb[8] = (count & 0xFF) as u8;

        let data_len = count as u32 * self.sector_size;
        let mut tmp = [0u8; 4096];
        let len = buf.len().min(4096);
        tmp[..len].copy_from_slice(&buf[..len]);
        self.bulk_transfer(&cb, 10, CBW_FLAG_OUT, data_len, Some(&mut tmp[..len]))
    }

    fn scsi_inquiry(&mut self, buf: &mut [u8]) -> bool {
        let mut cb = [0u8; 16];
        cb[0] = SCSI_INQUIRY;
        cb[4] = 36;
        self.bulk_transfer(&cb, 6, CBW_FLAG_IN, 36, Some(buf))
    }

    fn scsi_read_capacity(&mut self, buf: &mut [u8]) -> bool {
        let mut cb = [0u8; 16];
        cb[0] = SCSI_READ_CAPACITY;
        self.bulk_transfer(&cb, 10, CBW_FLAG_IN, 8, Some(buf))
    }

    fn bulk_transfer(&mut self, cb: &[u8], cb_len: u8, flags: u8, data_len: u32, buf: Option<&mut [u8]>) -> bool {
        // В реальной реализации здесь был бы USB bulk transfer через EHCI/UHCI
        // Для нашего уровня — заглушка которая показывает структуру
        // Реальная передача требует настроенных endpoint дескрипторов

        let tag = self.tag_counter;
        self.tag_counter += 1;

        // Строим CBW
        let cbw = Cbw {
            signature: CBW_SIGNATURE,
            tag,
            data_len,
            flags,
            lun: 0,
            cb_len,
            cb: {
                let mut arr = [0u8; 16];
                let l = cb.len().min(16);
                arr[..l].copy_from_slice(&cb[..l]);
                arr
            },
        };

        // TODO: отправить CBW через bulk OUT endpoint
        // TODO: передать данные через bulk IN/OUT endpoint
        // TODO: получить CSW через bulk IN endpoint
        // TODO: проверить CSW.status == 0

        // Пока возвращаем true для компиляции
        // Реальная реализация требует USB transfer layer
        false
    }
}

static mut USB_MSD: Option<UsbMsd> = None;

pub fn init() -> bool {
    let mut msd = UsbMsd::new();
    if msd.init() {
        unsafe { USB_MSD = Some(msd); }
        true
    } else {
        false
    }
}

pub fn get() -> Option<&'static mut UsbMsd> {
    unsafe { USB_MSD.as_mut() }
}

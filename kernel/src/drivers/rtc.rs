// RTC (Real Time Clock) драйвер
// Читает время/дату из CMOS через порты 0x70/0x71

// CMOS регистры
const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

const RTC_SECONDS:  u8 = 0x00;
const RTC_MINUTES:  u8 = 0x02;
const RTC_HOURS:    u8 = 0x04;
const RTC_WEEKDAY:  u8 = 0x06;
const RTC_DAY:      u8 = 0x07;
const RTC_MONTH:    u8 = 0x08;
const RTC_YEAR:     u8 = 0x09;
const RTC_CENTURY:  u8 = 0x32; // не на всех платах
const RTC_STATUS_A: u8 = 0x0A;
const RTC_STATUS_B: u8 = 0x0B;

#[derive(Copy, Clone, Debug)]
pub struct DateTime {
    pub year:    u16,
    pub month:   u8,
    pub day:     u8,
    pub hour:    u8,
    pub minute:  u8,
    pub second:  u8,
    pub weekday: u8,
}

impl DateTime {
    pub fn weekday_str(&self) -> &'static str {
        match self.weekday {
            1 => "Sun", 2 => "Mon", 3 => "Tue", 4 => "Wed",
            5 => "Thu", 6 => "Fri", 7 => "Sat", _ => "???",
        }
    }
}

unsafe fn cmos_read(reg: u8) -> u8 {
    // Бит 7 = NMI disable
    x86_out(CMOS_ADDR, reg | 0x80);
    x86_in(CMOS_DATA)
}

unsafe fn x86_out(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

unsafe fn x86_in(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

fn is_updating() -> bool {
    unsafe { cmos_read(RTC_STATUS_A) & 0x80 != 0 }
}

fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

pub fn read() -> DateTime {
    // Ждём пока RTC не обновляется
    while is_updating() {}

    let (sec, min, hour, day, month, year_raw, weekday, century, status_b) = unsafe {
        (
            cmos_read(RTC_SECONDS),
            cmos_read(RTC_MINUTES),
            cmos_read(RTC_HOURS),
            cmos_read(RTC_DAY),
            cmos_read(RTC_MONTH),
            cmos_read(RTC_YEAR),
            cmos_read(RTC_WEEKDAY),
            cmos_read(RTC_CENTURY),
            cmos_read(RTC_STATUS_B),
        )
    };

    // Читаем второй раз для проверки (защита от гонки)
    while is_updating() {}
    let sec2 = unsafe { cmos_read(RTC_SECONDS) };
    // Если секунды изменились — читаем ещё раз
    let (sec, min, hour, day, month, year_raw, weekday) = if sec != sec2 {
        while is_updating() {}
        unsafe {(
            cmos_read(RTC_SECONDS),
            cmos_read(RTC_MINUTES),
            cmos_read(RTC_HOURS),
            cmos_read(RTC_DAY),
            cmos_read(RTC_MONTH),
            cmos_read(RTC_YEAR),
            cmos_read(RTC_WEEKDAY),
        )}
    } else {
        (sec, min, hour, day, month, year_raw, weekday)
    };

    // BCD → binary если нужно (бит 2 status_b = 0 означает BCD)
    let is_bcd = status_b & 0x04 == 0;
    let is_12h = status_b & 0x02 == 0;

    let (sec, min, mut hour, day, month, year_raw) = if is_bcd {
        (bcd_to_bin(sec), bcd_to_bin(min), bcd_to_bin(hour & 0x7F),
         bcd_to_bin(day), bcd_to_bin(month), bcd_to_bin(year_raw))
    } else {
        (sec, min, hour & 0x7F, day, month, year_raw)
    };

    // 12h → 24h
    if is_12h {
        let pm = unsafe { cmos_read(RTC_HOURS) } & 0x80 != 0;
        if pm && hour != 12 { hour += 12; }
        if !pm && hour == 12 { hour = 0; }
    }

    // Год: century регистр + 2-значный год
    let century = if is_bcd { bcd_to_bin(century) } else { century };
    let year = if century >= 19 && century <= 21 {
        century as u16 * 100 + year_raw as u16
    } else {
        // Fallback: если год < 70 → 2000+, иначе 1900+
        if year_raw < 70 { 2000 + year_raw as u16 } else { 1900 + year_raw as u16 }
    };

    DateTime { year, month, day, hour, minute: min, second: sec, weekday }
}

/// Форматирует время как "HH:MM:SS"
pub fn format_time(dt: &DateTime, buf: &mut [u8; 8]) {
    buf[0] = b'0' + dt.hour / 10;
    buf[1] = b'0' + dt.hour % 10;
    buf[2] = b':';
    buf[3] = b'0' + dt.minute / 10;
    buf[4] = b'0' + dt.minute % 10;
    buf[5] = b':';
    buf[6] = b'0' + dt.second / 10;
    buf[7] = b'0' + dt.second % 10;
}

/// Форматирует дату как "DD.MM.YYYY"
pub fn format_date(dt: &DateTime, buf: &mut [u8; 10]) {
    buf[0] = b'0' + dt.day / 10;
    buf[1] = b'0' + dt.day % 10;
    buf[2] = b'.';
    buf[3] = b'0' + dt.month / 10;
    buf[4] = b'0' + dt.month % 10;
    buf[5] = b'.';
    buf[6] = b'0' + (dt.year / 1000) as u8;
    buf[7] = b'0' + ((dt.year / 100) % 10) as u8;
    buf[8] = b'0' + ((dt.year / 10) % 10) as u8;
    buf[9] = b'0' + (dt.year % 10) as u8;
}

/// Короткий формат для topbar: "HH:MM"
pub fn format_time_short(dt: &DateTime, buf: &mut [u8; 5]) {
    buf[0] = b'0' + dt.hour / 10;
    buf[1] = b'0' + dt.hour % 10;
    buf[2] = b':';
    buf[3] = b'0' + dt.minute / 10;
    buf[4] = b'0' + dt.minute % 10;
}

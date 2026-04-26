// FAT директории — 8.3 имена и LFN (Long File Names)

use super::fat::FatFs;
use super::bpb::FatType;

// Атрибуты записи директории
pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN:    u8 = 0x02;
pub const ATTR_SYSTEM:    u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE:   u8 = 0x20;
pub const ATTR_LFN:       u8 = 0x0F; // LFN запись

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct DirEntry {
    pub name:       [u8; 8],
    pub ext:        [u8; 3],
    pub attr:       u8,
    pub nt_res:     u8,
    pub crt_tenth:  u8,
    pub crt_time:   u16,
    pub crt_date:   u16,
    pub acc_date:   u16,
    pub clus_hi:    u16,  // старшие 16 бит кластера (FAT32)
    pub wrt_time:   u16,
    pub wrt_date:   u16,
    pub clus_lo:    u16,  // младшие 16 бит кластера
    pub file_size:  u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct LfnEntry {
    pub order:      u8,
    pub name1:      [u16; 5],
    pub attr:       u8,
    pub lfn_type:   u8,
    pub checksum:   u8,
    pub name2:      [u16; 6],
    pub clus_lo:    u16,
    pub name3:      [u16; 2],
}

impl DirEntry {
    pub fn is_free(&self) -> bool {
        self.name[0] == 0x00 || self.name[0] == 0xE5
    }

    pub fn is_end(&self) -> bool {
        self.name[0] == 0x00
    }

    pub fn is_dir(&self) -> bool {
        self.attr & ATTR_DIRECTORY != 0
    }

    pub fn is_lfn(&self) -> bool {
        self.attr == ATTR_LFN
    }

    pub fn cluster(&self) -> u32 {
        (self.clus_hi as u32) << 16 | self.clus_lo as u32
    }

    /// Имя в формате "NAME.EXT"
    pub fn short_name(&self, buf: &mut [u8; 13]) -> usize {
        let mut pos = 0;
        // Имя (убираем пробелы справа)
        let mut name_end = 8;
        while name_end > 0 && self.name[name_end-1] == b' ' { name_end -= 1; }
        for i in 0..name_end {
            if pos < 12 { buf[pos] = self.name[i].to_ascii_lowercase(); pos += 1; }
        }
        // Расширение
        let mut ext_end = 3;
        while ext_end > 0 && self.ext[ext_end-1] == b' ' { ext_end -= 1; }
        if ext_end > 0 {
            if pos < 12 { buf[pos] = b'.'; pos += 1; }
            for i in 0..ext_end {
                if pos < 12 { buf[pos] = self.ext[i].to_ascii_lowercase(); pos += 1; }
            }
        }
        buf[pos] = 0;
        pos
    }

    /// Сравнивает с именем (case-insensitive)
    pub fn matches_name(&self, name: &str) -> bool {
        let mut buf = [0u8; 13];
        let len = self.short_name(&mut buf);
        let s = core::str::from_utf8(&buf[..len]).unwrap_or("");
        s.eq_ignore_ascii_case(name)
    }
}

// ── Итератор по директории ────────────────────────────────────────────────

pub struct DirIter<'a> {
    fs:          &'a FatFs,
    cluster:     u32,
    sector_off:  u32,  // сектор внутри кластера
    entry_off:   u32,  // запись внутри сектора
    is_root16:   bool, // FAT16 root dir (фиксированный размер)
    root_sec:    u32,
    root_secs:   u32,
    buf:         [u8; 512],
    buf_loaded:  bool,
    pub lfn_buf: [u16; 256],
    pub lfn_len: usize,
}

impl<'a> DirIter<'a> {
    pub fn new_root(fs: &'a FatFs) -> Self {
        let (is_root16, cluster, root_sec, root_secs) = match fs.geo.fat_type {
            FatType::Fat32 => (false, fs.geo.root_cluster, 0, 0),
            _ => (true, 0, fs.root_dir_sector(), fs.geo.root_dir_secs),
        };
        DirIter {
            fs, cluster, sector_off: 0, entry_off: 0,
            is_root16, root_sec, root_secs,
            buf: [0u8; 512], buf_loaded: false,
            lfn_buf: [0u16; 256], lfn_len: 0,
        }
    }

    pub fn new_dir(fs: &'a FatFs, cluster: u32) -> Self {
        DirIter {
            fs, cluster, sector_off: 0, entry_off: 0,
            is_root16: false, root_sec: 0, root_secs: 0,
            buf: [0u8; 512], buf_loaded: false,
            lfn_buf: [0u16; 256], lfn_len: 0,
        }
    }

    fn current_sector(&self) -> Option<u32> {
        if self.is_root16 {
            let sec = self.root_sec + self.sector_off;
            if self.sector_off >= self.root_secs { None } else { Some(sec) }
        } else {
            if self.cluster < 2 { return None; }
            Some(self.fs.geo.cluster_to_sector(self.cluster) + self.sector_off)
        }
    }

    fn advance(&mut self) {
        self.entry_off += 1;
        if self.entry_off >= 512 / 32 {
            self.entry_off = 0;
            self.sector_off += 1;
            self.buf_loaded = false;

            if !self.is_root16 && self.sector_off >= self.fs.geo.sec_per_clus {
                self.sector_off = 0;
                self.cluster = self.fs.next_cluster(self.cluster).unwrap_or(0);
            }
        }
    }

    /// Следующая не-LFN запись (с накоплением LFN)
    pub fn next(&mut self) -> Option<DirEntry> {
        loop {
            let sec = self.current_sector()?;

            if !self.buf_loaded {
                if !self.fs.read_cluster_sector(sec, &mut self.buf) { return None; }
                self.buf_loaded = true;
            }

            let off = self.entry_off as usize * 32;
            let entry = unsafe { *(self.buf.as_ptr().add(off) as *const DirEntry) };

            if entry.is_end() { return None; }

            self.advance();

            if entry.is_free() { self.lfn_len = 0; continue; }

            if entry.is_lfn() {
                // Накапливаем LFN
                let lfn = unsafe { *((&entry as *const DirEntry) as *const LfnEntry) };
                let order = (lfn.order & 0x3F) as usize;
                if order > 0 && order <= 20 {
                    let base = (order - 1) * 13;
                    // Копируем символы (read_unaligned для packed struct)
                    let name1: [u16; 5] = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(lfn.name1)) };
                    let name2: [u16; 6] = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(lfn.name2)) };
                    let name3: [u16; 2] = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(lfn.name3)) };
                    for (i, &c) in name1.iter().enumerate() {
                        if base + i < 256 { self.lfn_buf[base + i] = c; }
                    }
                    for (i, &c) in name2.iter().enumerate() {
                        if base + 5 + i < 256 { self.lfn_buf[base + 5 + i] = c; }
                    }
                    for (i, &c) in name3.iter().enumerate() {
                        if base + 11 + i < 256 { self.lfn_buf[base + 11 + i] = c; }
                    }
                    if lfn.order & 0x40 != 0 {
                        // Последняя LFN запись — вычисляем длину
                        self.lfn_len = base + 13;
                        // Обрезаем по нулевому символу
                        for i in 0..self.lfn_len {
                            if self.lfn_buf[i] == 0 { self.lfn_len = i; break; }
                        }
                    }
                }
                continue;
            }

            return Some(entry);
        }
    }

    /// Длинное имя файла как UTF-8 (если есть)
    pub fn lfn_as_utf8(&self, buf: &mut [u8; 256]) -> usize {
        if self.lfn_len == 0 { return 0; }
        let mut pos = 0;
        for i in 0..self.lfn_len {
            let c = self.lfn_buf[i];
            if c == 0 { break; }
            // Простая конвертация UTF-16 → UTF-8 (только BMP)
            if c < 0x80 {
                if pos < 255 { buf[pos] = c as u8; pos += 1; }
            } else if c < 0x800 {
                if pos + 1 < 255 {
                    buf[pos]   = 0xC0 | (c >> 6) as u8;
                    buf[pos+1] = 0x80 | (c & 0x3F) as u8;
                    pos += 2;
                }
            } else {
                if pos + 2 < 255 {
                    buf[pos]   = 0xE0 | (c >> 12) as u8;
                    buf[pos+1] = 0x80 | ((c >> 6) & 0x3F) as u8;
                    buf[pos+2] = 0x80 | (c & 0x3F) as u8;
                    pos += 3;
                }
            }
        }
        buf[pos] = 0;
        pos
    }
}

impl FatFs {
    pub fn read_cluster_sector(&self, sector: u32, buf: &mut [u8]) -> bool {
        use crate::drivers::ata::read_sectors;
        read_sectors(self.disk(), self.lba_offset + sector as u64, 1, buf).is_ok()
    }

    /// Ищет запись в директории по имени
    pub fn find_entry(&self, dir_cluster: u32, name: &str) -> Option<DirEntry> {
        let mut iter = if dir_cluster == 0 {
            DirIter::new_root(self)
        } else {
            DirIter::new_dir(self, dir_cluster)
        };

        while let Some(entry) = iter.next() {
            // Проверяем LFN
            if iter.lfn_len > 0 {
                let mut lfn_buf = [0u8; 256];
                let lfn_len = iter.lfn_as_utf8(&mut lfn_buf);
                let lfn_str = core::str::from_utf8(&lfn_buf[..lfn_len]).unwrap_or("");
                if lfn_str.eq_ignore_ascii_case(name) { return Some(entry); }
            }
            // Проверяем 8.3
            if entry.matches_name(name) { return Some(entry); }
        }
        None
    }

    /// Разрешает путь вида "/dir/subdir/file.txt"
    pub fn resolve_path(&self, path: &str) -> Option<DirEntry> {
        let mut cluster = 0u32; // 0 = root

        let parts = path.trim_start_matches('/');
        if parts.is_empty() { return None; }

        let mut remaining = parts;
        loop {
            let (component, rest) = match remaining.find('/') {
                Some(i) => (&remaining[..i], &remaining[i+1..]),
                None    => (remaining, ""),
            };

            let entry = self.find_entry(cluster, component)?;

            if rest.is_empty() {
                return Some(entry);
            }

            if !entry.is_dir() { return None; }
            cluster = entry.cluster();
            remaining = rest;
        }
    }

    /// Список файлов в директории
    pub fn list_dir_entries<F: FnMut(&DirEntry, &str)>(&self, dir_cluster: u32, mut callback: F) {
        let mut iter = if dir_cluster == 0 {
            DirIter::new_root(self)
        } else {
            DirIter::new_dir(self, dir_cluster)
        };

        while let Some(entry) = iter.next() {
            let mut name_buf = [0u8; 256];
            let mut short_buf = [0u8; 13];
            let name_len;
            let name: &str;

            if iter.lfn_len > 0 {
                name_len = iter.lfn_as_utf8(&mut name_buf);
                name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
            } else {
                name_len = entry.short_name(&mut short_buf);
                name = core::str::from_utf8(&short_buf[..name_len]).unwrap_or("");
            };
            callback(&entry, name);
        }
    }
}

// FAT таблица — чтение/запись цепочек кластеров

use super::bpb::{FatGeometry, FatType};
use crate::drivers::ata::{AtaDisk, read_sectors, write_sectors};

pub const FAT_EOC12: u32 = 0xFF8;
pub const FAT_EOC16: u32 = 0xFFF8;
pub const FAT_EOC32: u32 = 0x0FFFFFF8;
pub const FAT_FREE:  u32 = 0x00000000;
pub const FAT_BAD:   u32 = 0x0FFFFFF7;

static mut SECTOR_BUF: [u8; 512] = [0u8; 512];
static mut SECTOR_BUF2: [u8; 512] = [0u8; 512];

pub struct FatFs {
    pub geo:    FatGeometry,
    pub disk:   *const AtaDisk,
    pub lba_offset: u64, // смещение раздела на диске
}

impl FatFs {
    /// Монтирует FAT том с диска начиная с сектора `lba_start`
    pub fn mount(disk: &AtaDisk, lba_start: u64) -> Option<Self> {
        let mut buf = [0u8; 512];
        read_sectors(disk, lba_start, 1, &mut buf).ok()?;

        // Проверяем сигнатуру загрузочного сектора
        if buf[510] != 0x55 || buf[511] != 0xAA { return None; }

        let geo = FatGeometry::from_sector(&buf)?;

        Some(FatFs {
            geo,
            disk: disk as *const AtaDisk,
            lba_offset: lba_start,
        })
    }

    pub fn disk(&self) -> &AtaDisk {
        unsafe { &*self.disk }
    }

    pub fn read_sector(&self, sector: u32, buf: &mut [u8]) -> bool {
        read_sectors(self.disk(), self.lba_offset + sector as u64, 1, buf).is_ok()
    }

    fn write_sector(&self, sector: u32, buf: &[u8]) -> bool {
        let mut tmp = [0u8; 512];
        let l = buf.len().min(512);
        tmp[..l].copy_from_slice(&buf[..l]);
        write_sectors(self.disk(), self.lba_offset + sector as u64, 1, &tmp).is_ok()
    }

    // ── FAT таблица ───────────────────────────────────────────────────────

    /// Читает значение FAT для кластера
    pub fn fat_get(&self, cluster: u32) -> u32 {
        let (sec, off) = self.geo.fat_sector(cluster);
        let buf = unsafe { &mut SECTOR_BUF };
        if !self.read_sector(sec, buf) { return FAT_BAD; }

        match self.geo.fat_type {
            FatType::Fat12 => {
                let val = if off as usize + 1 < 512 {
                    (buf[off as usize] as u32) | ((buf[off as usize + 1] as u32) << 8)
                } else {
                    // Значение пересекает границу сектора
                    let lo = buf[off as usize] as u32;
                    let buf2 = unsafe { &mut SECTOR_BUF2 };
                    if !self.read_sector(sec + 1, buf2) { return FAT_BAD; }
                    lo | ((buf2[0] as u32) << 8)
                };
                if cluster & 1 != 0 { val >> 4 } else { val & 0xFFF }
            }
            FatType::Fat16 => {
                let off = off as usize;
                (buf[off] as u32) | ((buf[off+1] as u32) << 8)
            }
            FatType::Fat32 => {
                let off = off as usize;
                ((buf[off] as u32)
                | ((buf[off+1] as u32) << 8)
                | ((buf[off+2] as u32) << 16)
                | ((buf[off+3] as u32) << 24)) & 0x0FFFFFFF
            }
        }
    }

    /// Записывает значение FAT для кластера
    pub fn fat_set(&self, cluster: u32, value: u32) {
        let (sec, off) = self.geo.fat_sector(cluster);
        let buf = unsafe { &mut SECTOR_BUF };
        if !self.read_sector(sec, buf) { return; }

        match self.geo.fat_type {
            FatType::Fat12 => {
                let off = off as usize;
                if cluster & 1 != 0 {
                    buf[off]   = (buf[off] & 0x0F) | ((value << 4) as u8);
                    if off + 1 < 512 {
                        buf[off+1] = (value >> 4) as u8;
                    }
                } else {
                    buf[off]   = (value & 0xFF) as u8;
                    if off + 1 < 512 {
                        buf[off+1] = (buf[off+1] & 0xF0) | ((value >> 8) as u8 & 0x0F);
                    }
                }
            }
            FatType::Fat16 => {
                let off = off as usize;
                buf[off]   = (value & 0xFF) as u8;
                buf[off+1] = (value >> 8) as u8;
            }
            FatType::Fat32 => {
                let off = off as usize;
                let old = ((buf[off+3] as u32) & 0xF0) << 24;
                let v = (value & 0x0FFFFFFF) | old;
                buf[off]   = (v & 0xFF) as u8;
                buf[off+1] = ((v >> 8) & 0xFF) as u8;
                buf[off+2] = ((v >> 16) & 0xFF) as u8;
                buf[off+3] = ((v >> 24) & 0xFF) as u8;
            }
        }

        self.write_sector(sec, buf);
        // Обновляем все копии FAT
        for i in 1..self.geo.fat_count {
            let sec2 = sec + i * self.geo.fat_size;
            self.write_sector(sec2, buf);
        }
    }

    /// Проверяет конец цепочки
    pub fn is_eoc(&self, cluster: u32) -> bool {
        match self.geo.fat_type {
            FatType::Fat12 => cluster >= FAT_EOC12,
            FatType::Fat16 => cluster >= FAT_EOC16,
            FatType::Fat32 => cluster >= FAT_EOC32,
        }
    }

    /// Следующий кластер в цепочке
    pub fn next_cluster(&self, cluster: u32) -> Option<u32> {
        let next = self.fat_get(cluster);
        if self.is_eoc(next) || next == FAT_FREE || next == FAT_BAD {
            None
        } else {
            Some(next)
        }
    }

    /// Находит свободный кластер
    pub fn alloc_cluster(&self) -> Option<u32> {
        for c in 2..self.geo.total_clusters + 2 {
            if self.fat_get(c) == FAT_FREE {
                return Some(c);
            }
        }
        None
    }

    /// Выделяет цепочку из `count` кластеров
    pub fn alloc_chain(&self, count: u32) -> Option<u32> {
        let mut first = None;
        let mut prev = 0u32;

        for _ in 0..count {
            let c = self.alloc_cluster()?;
            let eoc = match self.geo.fat_type {
                FatType::Fat12 => 0xFFF,
                FatType::Fat16 => 0xFFFF,
                FatType::Fat32 => 0x0FFFFFFF,
            };
            self.fat_set(c, eoc);

            if first.is_none() {
                first = Some(c);
            } else {
                self.fat_set(prev, c);
            }
            prev = c;
        }
        first
    }

    /// Освобождает цепочку кластеров
    pub fn free_chain(&self, start: u32) {
        let mut c = start;
        loop {
            let next = self.fat_get(c);
            self.fat_set(c, FAT_FREE);
            if self.is_eoc(next) || next == FAT_FREE { break; }
            c = next;
        }
    }

    // ── Чтение/запись кластеров ───────────────────────────────────────────

    pub fn read_cluster(&self, cluster: u32, buf: &mut [u8]) -> bool {
        let sec = self.geo.cluster_to_sector(cluster);
        let count = self.geo.sec_per_clus as u16;
        let needed = count as usize * 512;
        if buf.len() < needed { return false; }
        read_sectors(self.disk(), self.lba_offset + sec as u64, count, buf).is_ok()
    }

    pub fn write_cluster(&self, cluster: u32, buf: &[u8]) -> bool {
        let sec = self.geo.cluster_to_sector(cluster);
        let count = self.geo.sec_per_clus as u16;
        let needed = count as usize * 512;
        let mut tmp = [0u8; 4096];
        let l = buf.len().min(needed).min(4096);
        tmp[..l].copy_from_slice(&buf[..l]);
        write_sectors(self.disk(), self.lba_offset + sec as u64, count, &tmp).is_ok()
    }

    // ── Root directory ────────────────────────────────────────────────────

    pub fn root_dir_sector(&self) -> u32 {
        self.geo.reserved_secs + self.geo.fat_count * self.geo.fat_size
    }

    pub fn root_dir_cluster(&self) -> u32 {
        self.geo.root_cluster
    }
}

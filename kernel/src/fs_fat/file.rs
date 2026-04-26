// FAT файловые операции — чтение, запись, создание, удаление

use super::fat::{FatFs, FAT_FREE};
use super::dir::{DirEntry, DirIter, ATTR_ARCHIVE, ATTR_DIRECTORY};
use super::bpb::FatType;
use crate::drivers::ata::{read_sectors, write_sectors};

#[derive(Copy, Clone, PartialEq)]
pub enum OpenMode { Read, Write, Append, Create }

pub struct FatFile<'a> {
    pub fs:       &'a FatFs,
    pub entry:    DirEntry,
    pub cluster:  u32,   // текущий кластер
    pub position: u32,   // позиция в файле
    pub size:     u32,
    pub mode:     OpenMode,
    // Кластер начала файла (для перемотки)
    pub start_cluster: u32,
}

impl<'a> FatFile<'a> {
    pub fn open(fs: &'a FatFs, path: &str, mode: OpenMode) -> Option<Self> {
        match mode {
            OpenMode::Read => {
                let entry = fs.resolve_path(path)?;
                if entry.is_dir() { return None; }
                let start = entry.cluster();
                Some(FatFile {
                    fs, entry, cluster: start, position: 0,
                    size: entry.file_size, mode,
                    start_cluster: start,
                })
            }
            OpenMode::Write | OpenMode::Create => {
                // Создаём или перезаписываем файл
                // Упрощённо — создаём новый файл в корне
                let start = fs.alloc_cluster()?;
                let entry = DirEntry {
                    name: [b' '; 8], ext: [b' '; 3],
                    attr: ATTR_ARCHIVE,
                    nt_res: 0, crt_tenth: 0, crt_time: 0, crt_date: 0,
                    acc_date: 0,
                    clus_hi: (start >> 16) as u16,
                    wrt_time: 0, wrt_date: 0,
                    clus_lo: (start & 0xFFFF) as u16,
                    file_size: 0,
                };
                Some(FatFile {
                    fs, entry, cluster: start, position: 0,
                    size: 0, mode,
                    start_cluster: start,
                })
            }
            OpenMode::Append => {
                let entry = fs.resolve_path(path)?;
                let start = entry.cluster();
                // Находим последний кластер
                let mut last = start;
                while let Some(next) = fs.next_cluster(last) { last = next; }
                let pos = entry.file_size;
                Some(FatFile {
                    fs, entry, cluster: last, position: pos,
                    size: pos, mode,
                    start_cluster: start,
                })
            }
        }
    }

    /// Читает до `buf.len()` байт
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        if self.mode != OpenMode::Read { return 0; }
        if self.position >= self.size { return 0; }

        let remaining = (self.size - self.position) as usize;
        let to_read = buf.len().min(remaining);
        let mut read = 0;

        let clus_size = self.fs.geo.bytes_per_clus as usize;
        let mut clus_buf = [0u8; 4096];

        while read < to_read {
            if self.cluster < 2 { break; }

            let clus_off = (self.position as usize) % clus_size;
            let avail = clus_size - clus_off;
            let chunk = (to_read - read).min(avail);

            if !self.fs.read_cluster(self.cluster, &mut clus_buf[..clus_size]) { break; }

            buf[read..read+chunk].copy_from_slice(&clus_buf[clus_off..clus_off+chunk]);
            read += chunk;
            self.position += chunk as u32;

            // Переходим к следующему кластеру если нужно
            if self.position as usize % clus_size == 0 {
                self.cluster = self.fs.next_cluster(self.cluster).unwrap_or(0);
            }
        }
        read
    }

    /// Записывает данные
    pub fn write(&mut self, data: &[u8]) -> usize {
        if self.mode == OpenMode::Read { return 0; }

        let clus_size = self.fs.geo.bytes_per_clus as usize;
        let mut written = 0;
        let mut clus_buf = [0u8; 4096];

        while written < data.len() {
            // Выделяем новый кластер если нужно
            if self.cluster < 2 {
                match self.fs.alloc_cluster() {
                    Some(c) => {
                        self.fs.fat_set(c, match self.fs.geo.fat_type {
                            FatType::Fat12 => 0xFFF,
                            FatType::Fat16 => 0xFFFF,
                            FatType::Fat32 => 0x0FFFFFFF,
                        });
                        if self.start_cluster < 2 {
                            self.start_cluster = c;
                        }
                        self.cluster = c;
                    }
                    None => break,
                }
            }

            let clus_off = (self.position as usize) % clus_size;
            let avail = clus_size - clus_off;
            let chunk = (data.len() - written).min(avail);

            // Читаем существующий кластер (для частичной записи)
            if clus_off > 0 {
                self.fs.read_cluster(self.cluster, &mut clus_buf[..clus_size]);
            }

            clus_buf[clus_off..clus_off+chunk].copy_from_slice(&data[written..written+chunk]);
            self.fs.write_cluster(self.cluster, &clus_buf[..clus_size]);

            written += chunk;
            self.position += chunk as u32;
            if self.position > self.size { self.size = self.position; }

            // Переходим к следующему кластеру
            if self.position as usize % clus_size == 0 {
                match self.fs.next_cluster(self.cluster) {
                    Some(next) => self.cluster = next,
                    None => {
                        // Выделяем новый кластер
                        if let Some(new_c) = self.fs.alloc_cluster() {
                            let eoc = match self.fs.geo.fat_type {
                                FatType::Fat12 => 0xFFF,
                                FatType::Fat16 => 0xFFFF,
                                FatType::Fat32 => 0x0FFFFFFF,
                            };
                            self.fs.fat_set(new_c, eoc);
                            self.fs.fat_set(self.cluster, new_c);
                            self.cluster = new_c;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        written
    }

    /// Перемотка в начало
    pub fn rewind(&mut self) {
        self.position = 0;
        self.cluster = self.start_cluster;
    }

    /// Seek (только вперёд для простоты)
    pub fn seek(&mut self, pos: u32) {
        if pos < self.position { self.rewind(); }
        let clus_size = self.fs.geo.bytes_per_clus;
        while self.position < pos {
            let skip = (pos - self.position).min(clus_size);
            self.position += skip;
            if self.position % clus_size == 0 {
                self.cluster = self.fs.next_cluster(self.cluster).unwrap_or(0);
            }
        }
    }
}

// ── Монтирование томов ────────────────────────────────────────────────────

const MAX_VOLUMES: usize = 4;

pub struct Volume {
    pub fs:     FatFs,
    pub label:  [u8; 12],
    pub letter: u8, // 'C', 'D', ...
}

static mut VOLUMES: [Option<Volume>; MAX_VOLUMES] = [None, None, None, None];
static mut VOL_COUNT: usize = 0;

/// Монтирует FAT том с диска
pub fn mount_volume(disk: &crate::drivers::ata::AtaDisk, lba: u64, letter: u8) -> bool {
    let fs = match FatFs::mount(disk, lba) {
        Some(f) => f,
        None => return false,
    };

    unsafe {
        if VOL_COUNT < MAX_VOLUMES {
            VOLUMES[VOL_COUNT] = Some(Volume {
                fs,
                label: [0u8; 12],
                letter,
            });
            VOL_COUNT += 1;
            true
        } else {
            false
        }
    }
}

pub fn get_volume(letter: u8) -> Option<&'static FatFs> {
    unsafe {
        for v in VOLUMES.iter().flatten() {
            if v.letter == letter { return Some(&v.fs); }
        }
        None
    }
}

pub fn volume_count() -> usize { unsafe { VOL_COUNT } }

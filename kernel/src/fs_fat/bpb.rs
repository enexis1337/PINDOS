// BPB (BIOS Parameter Block) — структуры заголовка FAT тома

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Bpb {
    pub jump:           [u8; 3],
    pub oem:            [u8; 8],
    pub bytes_per_sec:  u16,
    pub sec_per_clus:   u8,
    pub reserved_secs:  u16,
    pub fat_count:      u8,
    pub root_entry_cnt: u16,  // 0 для FAT32
    pub total_secs16:   u16,  // 0 если > 65535
    pub media:          u8,
    pub fat_size16:     u16,  // 0 для FAT32
    pub sec_per_track:  u16,
    pub head_count:     u16,
    pub hidden_secs:    u32,
    pub total_secs32:   u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Bpb16Ext {
    pub drive_num:  u8,
    pub reserved:   u8,
    pub boot_sig:   u8,
    pub vol_id:     u32,
    pub vol_label:  [u8; 11],
    pub fs_type:    [u8; 8],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Bpb32Ext {
    pub fat_size32:     u32,
    pub ext_flags:      u16,
    pub fs_version:     u16,
    pub root_cluster:   u32,
    pub fs_info:        u16,
    pub backup_boot:    u16,
    pub reserved:       [u8; 12],
    pub drive_num:      u8,
    pub reserved1:      u8,
    pub boot_sig:       u8,
    pub vol_id:         u32,
    pub vol_label:      [u8; 11],
    pub fs_type:        [u8; 8],
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FatType { Fat12, Fat16, Fat32 }

pub struct FatGeometry {
    pub fat_type:       FatType,
    pub bytes_per_sec:  u32,
    pub sec_per_clus:   u32,
    pub reserved_secs:  u32,
    pub fat_count:      u32,
    pub fat_size:       u32,   // секторов на FAT
    pub root_dir_secs:  u32,
    pub first_data_sec: u32,
    pub total_clusters: u32,
    pub root_cluster:   u32,   // FAT32 только
    pub total_secs:     u32,
    pub bytes_per_clus: u32,
}

impl FatGeometry {
    pub fn from_sector(sector: &[u8]) -> Option<Self> {
        if sector.len() < 512 { return None; }

        let bpb = unsafe { &*(sector.as_ptr() as *const Bpb) };

        let bytes_per_sec = bpb.bytes_per_sec as u32;
        let sec_per_clus  = bpb.sec_per_clus as u32;
        let reserved_secs = bpb.reserved_secs as u32;
        let fat_count     = bpb.fat_count as u32;
        let root_entry_cnt = bpb.root_entry_cnt as u32;

        if bytes_per_sec == 0 || sec_per_clus == 0 { return None; }

        let total_secs = if bpb.total_secs16 != 0 {
            bpb.total_secs16 as u32
        } else {
            bpb.total_secs32
        };

        let fat_size = if bpb.fat_size16 != 0 {
            bpb.fat_size16 as u32
        } else {
            // FAT32
            let ext32 = unsafe { &*(sector.as_ptr().add(36) as *const Bpb32Ext) };
            ext32.fat_size32
        };

        let root_dir_secs = (root_entry_cnt * 32 + bytes_per_sec - 1) / bytes_per_sec;
        let first_data_sec = reserved_secs + fat_count * fat_size + root_dir_secs;
        let data_secs = total_secs.saturating_sub(first_data_sec);
        let total_clusters = data_secs / sec_per_clus;

        let fat_type = if total_clusters < 4085 {
            FatType::Fat12
        } else if total_clusters < 65525 {
            FatType::Fat16
        } else {
            FatType::Fat32
        };

        let root_cluster = if fat_type == FatType::Fat32 {
            let ext32 = unsafe { &*(sector.as_ptr().add(36) as *const Bpb32Ext) };
            ext32.root_cluster
        } else {
            0
        };

        Some(FatGeometry {
            fat_type,
            bytes_per_sec,
            sec_per_clus,
            reserved_secs,
            fat_count,
            fat_size,
            root_dir_secs,
            first_data_sec,
            total_clusters,
            root_cluster,
            total_secs,
            bytes_per_clus: bytes_per_sec * sec_per_clus,
        })
    }

    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.first_data_sec + (cluster - 2) * self.sec_per_clus
    }

    pub fn fat_sector(&self, cluster: u32) -> (u32, u32) {
        // Возвращает (сектор, смещение в байтах)
        match self.fat_type {
            FatType::Fat12 => {
                let offset = cluster + cluster / 2;
                (self.reserved_secs + offset / self.bytes_per_sec,
                 offset % self.bytes_per_sec)
            }
            FatType::Fat16 => {
                let offset = cluster * 2;
                (self.reserved_secs + offset / self.bytes_per_sec,
                 offset % self.bytes_per_sec)
            }
            FatType::Fat32 => {
                let offset = cluster * 4;
                (self.reserved_secs + offset / self.bytes_per_sec,
                 offset % self.bytes_per_sec)
            }
        }
    }
}

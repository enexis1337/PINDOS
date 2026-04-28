// Структуры данных пакета

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum PackageState {
    Available,   // есть в реестре, не установлен
    Installed,   // установлен
    Broken,      // установлен, но повреждён
}

/// Метаданные пакета
#[derive(Copy, Clone)]
pub struct PackageMeta {
    pub name:        [u8; 32],
    pub name_len:    usize,
    pub version:     [u8; 16],
    pub version_len: usize,
    pub description: [u8; 128],
    pub desc_len:    usize,
    pub size_kb:     u32,
}

impl PackageMeta {
    pub const fn empty() -> Self {
        PackageMeta {
            name:        [0; 32],
            name_len:    0,
            version:     [0; 16],
            version_len: 0,
            description: [0; 128],
            desc_len:    0,
            size_kb:     0,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    pub fn version_str(&self) -> &str {
        core::str::from_utf8(&self.version[..self.version_len]).unwrap_or("")
    }

    pub fn desc_str(&self) -> &str {
        core::str::from_utf8(&self.description[..self.desc_len]).unwrap_or("")
    }

    pub fn from_strs(name: &str, version: &str, description: &str, size_kb: u32) -> Self {
        let mut meta = PackageMeta::empty();

        let nb = name.as_bytes();
        let nl = nb.len().min(32);
        meta.name[..nl].copy_from_slice(&nb[..nl]);
        meta.name_len = nl;

        let vb = version.as_bytes();
        let vl = vb.len().min(16);
        meta.version[..vl].copy_from_slice(&vb[..vl]);
        meta.version_len = vl;

        let db = description.as_bytes();
        let dl = db.len().min(128);
        meta.description[..dl].copy_from_slice(&db[..dl]);
        meta.desc_len = dl;

        meta.size_kb = size_kb;
        meta
    }
}

/// Запись об установленном/доступном пакете
#[derive(Copy, Clone)]
pub struct Package {
    pub meta:  PackageMeta,
    pub state: PackageState,
}

impl Package {
    pub const fn empty() -> Self {
        Package {
            meta:  PackageMeta::empty(),
            state: PackageState::Available,
        }
    }
}

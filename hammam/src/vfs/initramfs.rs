use super::{Vnode, FileStat, FileKind, IoError};
use alloc::{string::String, sync::Arc, vec::Vec, format};
use core::str;

/// Один файл/директория внутри initramfs
struct InitEntry {
    kind: FileKind,
    data: &'static [u8],   // срез в исходном CPIO-образе
    children: Vec<String>, // для директорий
}

/// Read-only файловая система на основе CPIO newc архива в памяти
pub struct InitramfsFs {
    root: InitEntry,
    all_entries: alloc::collections::BTreeMap<String, InitEntry>,
}

impl InitramfsFs {
    /// Распарсить CPIO newc формат (magic "070701").
    /// data — весь архив как &'static [u8] (лежит в памяти ядра).
    pub fn parse(data: &'static [u8]) -> Result<Arc<Self>, &'static str> {
        let mut entries = alloc::collections::BTreeMap::new();
        
        // Добавляем корневую директорию
        entries.insert(
            String::from("/"),
            InitEntry {
                kind: FileKind::Directory,
                data: &[],
                children: Vec::new(),
            },
        );

        let mut pos = 0usize;

        loop {
            // Проверяем, достаточно ли данных для заголовка
            if pos + 110 > data.len() {
                break;
            }

            // Проверяем magic number CPIO newc
            let magic = &data[pos..pos + 6];
            if magic != b"070701" {
                break;
            }

            // Парсим поля заголовка CPIO newc (все в hex ASCII, по 8 байт)
            let namesize = hex8(&data[pos + 94..pos + 102]) as usize;
            let filesize = hex8(&data[pos + 54..pos + 62]) as usize;
            let mode = hex8(&data[pos + 14..pos + 22]);

            // Извлекаем имя файла
            let name_start = pos + 110;
            if name_start + namesize > data.len() {
                break;
            }

            let name_bytes = &data[name_start..name_start + namesize - 1]; // -1 для null terminator
            let name = str::from_utf8(name_bytes).unwrap_or("");

            // Проверяем TRAILER (конец архива)
            if name == "TRAILER!!!" {
                break;
            }

            // Выравниваем позицию данных на 4 байта
            let data_start = align4(name_start + namesize);
            let data_end = data_start + filesize;

            if data_end > data.len() {
                break;
            }

            // Определяем тип файла по mode
            let kind = if (mode & 0xF000) == 0x4000 {
                FileKind::Directory
            } else {
                FileKind::Regular
            };

            // Создаем запись
            entries.insert(
                String::from(name),
                InitEntry {
                    kind,
                    data: &data[data_start..data_end],
                    children: Vec::new(),
                },
            );

            // Переходим к следующей записи
            pos = align4(data_end);
        }

        // Создаем корневую директорию и вычисляем children
        let root = InitEntry {
            kind: FileKind::Directory,
            data: &[],
            children: Self::compute_children(&entries, "/"),
        };

        Ok(Arc::new(Self {
            root,
            all_entries: entries,
        }))
    }

    /// Вычислить список дочерних файлов для директории
    fn compute_children(
        entries: &alloc::collections::BTreeMap<String, InitEntry>,
        parent_path: &str,
    ) -> Vec<String> {
        let mut children = Vec::new();

        for key in entries.keys() {
            // Проверяем, является ли key дочерним файлом parent_path
            if is_child_of(key, parent_path) {
                let relative = get_relative_name(key, parent_path);
                if !relative.is_empty() {
                    children.push(String::from(&relative));
                }
            }
        }

        children
    }

    /// Найти vnode по пути
    pub fn lookup_path(&self, path: &str) -> Result<Arc<dyn Vnode>, IoError> {
        let normalized = normalize_path(path);

        // Корневая директория
        let parent_fs = self as *const InitramfsFs;
        if normalized == "/" {
            return Ok(Arc::new(InitramfsVnode {
                fs: Arc::new(self.root.clone()),
                path: String::from("/"),
                parent_fs: parent_fs,
            }));
        }

        // Ищем в entries
        if let Some(entry) = self.all_entries.get(&normalized) {
            return Ok(Arc::new(InitramfsVnode {
                fs: Arc::new(entry.clone()),
                path: String::from(&normalized),
                parent_fs: parent_fs,
            }));
        }

        Err(IoError::NotFound)
    }
}

/// Клон для InitEntry (так как нет derive Clone для &'static [u8])
impl Clone for InitEntry {
    fn clone(&self) -> Self {
        InitEntry {
            kind: self.kind.clone(),
            data: self.data,
            children: self.children.clone(),
        }
    }
}

/// Vnode для initramfs
struct InitramfsVnode {
    fs: Arc<InitEntry>,
    path: String,
    parent_fs: *const InitramfsFs,
}

unsafe impl Send for InitramfsVnode {}
unsafe impl Sync for InitramfsVnode {}

impl Vnode for InitramfsVnode {
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, IoError> {
        if self.fs.kind != FileKind::Regular {
            return Err(IoError::NotAFile);
        }

        let offset = offset as usize;
        if offset > self.fs.data.len() {
            return Ok(0);
        }

        let available = self.fs.data.len() - offset;
        let to_read = core::cmp::min(available, buf.len());

        buf[..to_read].copy_from_slice(&self.fs.data[offset..offset + to_read]);
        Ok(to_read)
    }

    fn write(&self, _offset: u64, _buf: &[u8]) -> Result<usize, IoError> {
        Err(IoError::PermissionDenied)
    }

    fn stat(&self) -> Result<FileStat, IoError> {
        Ok(FileStat {
            size: self.fs.data.len() as u64,
            kind: self.fs.kind.clone(),
        })
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Vnode>, IoError> {
        if self.fs.kind != FileKind::Directory {
            return Err(IoError::NotADirectory);
        }

        let full_path = if self.path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", self.path, name)
        };

        if !self.fs.children.contains(&String::from(name)) {
            return Err(IoError::NotFound);
        }

        unsafe { (*self.parent_fs).lookup_path(&full_path) }
    }

    fn readdir(&self) -> Result<Vec<String>, IoError> {
        if self.fs.kind != FileKind::Directory {
            return Err(IoError::NotAFile);
        }

        Ok(self.fs.children.clone())
    }
}

/// Парсить 8-байтовое hex число из ASCII
fn hex8(b: &[u8]) -> u32 {
    let s = str::from_utf8(b).unwrap_or("0");
    u32::from_str_radix(s, 16).unwrap_or(0)
}

/// Выровнять на 4 байта
fn align4(x: usize) -> usize {
    (x + 3) & !3
}

/// Нормализовать путь
fn normalize_path(path: &str) -> String {
    let mut normalized = String::new();
    let mut prev_was_slash = false;

    for ch in path.chars() {
        if ch == '/' {
            if !prev_was_slash {
                normalized.push(ch);
            }
            prev_was_slash = true;
        } else {
            normalized.push(ch);
            prev_was_slash = false;
        }
    }

    if normalized.is_empty() {
        normalized.push('/');
    }

    // Удалить trailing slash если это не корневой путь
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    normalized
}

/// Проверить, является ли path дочерним файлом parent_path
fn is_child_of(path: &str, parent_path: &str) -> bool {
    if parent_path == "/" {
        return path.starts_with('/') && path[1..].contains('/') == false;
    }

    if !path.starts_with(parent_path) {
        return false;
    }

    if path.len() <= parent_path.len() {
        return false;
    }

    path[parent_path.len()..].matches('/').count() == 1
        && path[parent_path.len()..].starts_with('/')
}

/// Получить относительное имя файла
fn get_relative_name(path: &str, parent_path: &str) -> String {
    if parent_path == "/" {
        return String::from(&path[1..]);
    }

    if path.starts_with(parent_path) && path.len() > parent_path.len() {
        let relative = &path[parent_path.len() + 1..];
        if !relative.contains('/') {
            return String::from(relative);
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_child_of() {
        assert!(is_child_of("/bin", "/"));
        assert!(is_child_of("/bin/sh", "/bin"));
        assert!(!is_child_of("/bin/sh/x", "/bin"));
        assert!(!is_child_of("/bin", "/bi"));
    }

    #[test]
    fn test_get_relative_name() {
        assert_eq!(get_relative_name("/bin", "/"), "bin");
        assert_eq!(get_relative_name("/bin/sh", "/bin"), "sh");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path("//"), "/");
        assert_eq!(normalize_path("/bin"), "/bin");
        assert_eq!(normalize_path("/bin/"), "/bin");
    }
}

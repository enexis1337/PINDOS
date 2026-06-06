use alloc::{sync::Arc, string::String, vec::Vec};
use core::fmt;

pub mod initramfs;

/// Метаданные файла
#[derive(Debug, Clone)]
pub struct FileStat {
    pub size: u64,
    pub kind: FileKind,
}

/// Тип файла
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Regular,      // Обычный файл
    Directory,    // Директория
    SymLink,      // Символическая ссылка
}

/// Ошибки операций с файлами
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    NotFound,           // Файл не найден
    NotADirectory,      // Ожидался директорий, а получен файл
    NotAFile,           // Ожидался файл, а получена директория
    PermissionDenied,   // Недостаточно прав
    InvalidOffset,      // Неверное смещение
    Io,                 // Другая I/O ошибка
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IoError::NotFound => write!(f, "File not found"),
            IoError::NotADirectory => write!(f, "Not a directory"),
            IoError::NotAFile => write!(f, "Not a file"),
            IoError::PermissionDenied => write!(f, "Permission denied"),
            IoError::InvalidOffset => write!(f, "Invalid offset"),
            IoError::Io => write!(f, "I/O error"),
        }
    }
}

/// Абстрактный узел файловой системы (Vnode).
/// Реализует операции с файлами независимо от типа FS.
pub trait Vnode: Send + Sync {
    /// Прочитать данные из файла начиная с offset
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, IoError>;

    /// Записать данные в файл начиная с offset
    fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, IoError>;

    /// Получить метаданные файла (stat)
    fn stat(&self) -> Result<FileStat, IoError>;

    /// Найти дочерний узел по имени (для директорий)
    fn lookup(&self, name: &str) -> Result<Arc<dyn Vnode>, IoError>;

    /// Прочитать содержимое директории
    fn readdir(&self) -> Result<Vec<String>, IoError>;
}

/// Точка монтирования файловой системы
pub struct VfsMount {
    pub mountpoint: String,    // Путь к точке монтирования (например "/")
    pub root: Arc<dyn Vnode>,  // Корневой узел файловой системы
}

/// Дерево монтирований — управляет всеми смонтированными файловыми системами
pub struct VfsTree {
    mounts: Vec<VfsMount>,
}

impl VfsTree {
    /// Создать новое пустое дерево монтирований
    pub const fn new() -> Self {
        Self {
            mounts: Vec::new(),
        }
    }

    /// Смонтировать файловую систему по пути
    pub fn mount(&mut self, path: &str, root: Arc<dyn Vnode>) {
        self.mounts.push(VfsMount {
            mountpoint: String::from(path),
            root,
        });
    }

    /// Найти самый длинный совпадающий mountpoint для пути
    fn find_mount_point(&self, path: &str) -> Option<&VfsMount> {
        let mut best_match: Option<&VfsMount> = None;
        let mut best_len = 0;

        for mount in &self.mounts {
            // Проверяем, начинается ли path с mountpoint
            if path.starts_with(&mount.mountpoint) {
                let mount_len = mount.mountpoint.len();
                // Выбираем самый длинный совпадающий mountpoint
                if mount_len > best_len {
                    best_match = Some(mount);
                    best_len = mount_len;
                }
            }
        }

        best_match
    }

    /// Найти vnode по абсолютному пути
    /// Примеры: "/", "/home/user/file.txt", "/etc/passwd"
    pub fn lookup(&self, path: &str) -> Result<Arc<dyn Vnode>, IoError> {
        // Нормализуем путь (удаляем двойные слэши и trailing слэш)
        let normalized_path = normalize_path(path);

        // Найти соответствующий mountpoint
        let mount = self
            .find_mount_point(&normalized_path)
            .ok_or(IoError::NotFound)?;

        // Получить оставшийся путь после mountpoint
        let remaining_path = if mount.mountpoint == "/" {
            &normalized_path[1..]  // Пропустить leading "/"
        } else {
            &normalized_path[mount.mountpoint.len()..]
        };

        // Если remaining_path пусто, это сам mountpoint
        if remaining_path.is_empty() || remaining_path == "/" {
            return Ok(Arc::clone(&mount.root));
        }

        // Пройти по компонентам пути через vnode.lookup()
        let mut current_vnode = Arc::clone(&mount.root);

        for component in remaining_path.split('/') {
            if component.is_empty() {
                continue;  // Пропускаем пустые компоненты
            }

            current_vnode = current_vnode.lookup(component)?;
        }

        Ok(current_vnode)
    }

    /// Получить количество смонтированных файловых систем
    pub fn mount_count(&self) -> usize {
        self.mounts.len()
    }

    /// Список всех mountpoint'ов
    pub fn list_mounts(&self) -> Vec<&str> {
        self.mounts.iter().map(|m| m.mountpoint.as_str()).collect()
    }
}

impl Default for VfsTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Нормализовать путь (удалить двойные слэши и trailing слэш если не корневой)
fn normalize_path(path: &str) -> String {
    let mut normalized = String::new();
    let mut prev_was_slash = false;

    for ch in path.chars() {
        if ch == '/' {
            if !prev_was_slash || normalized.is_empty() {
                normalized.push(ch);
            }
            prev_was_slash = true;
        } else {
            normalized.push(ch);
            prev_was_slash = false;
        }
    }

    // Удалить trailing slash если это не корневой путь
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    // Убедиться, что путь начинается с "/"
    if normalized.is_empty() {
        normalized.push('/');
    } else if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }

    normalized
}

/// Глобальное дерево монтирований, защищенное Mutex'ом
pub static VFS: spin::Mutex<VfsTree> = spin::Mutex::new(VfsTree::new());

/// Вспомогательная функция для чтения файла через VFS
pub fn vfs_read(path: &str, offset: u64, buf: &mut [u8]) -> Result<usize, IoError> {
    let vfs = VFS.lock();
    let vnode = vfs.lookup(path)?;
    vnode.read(offset, buf)
}

/// Вспомогательная функция для записи файла через VFS
pub fn vfs_write(path: &str, offset: u64, buf: &[u8]) -> Result<usize, IoError> {
    let vfs = VFS.lock();
    let vnode = vfs.lookup(path)?;
    vnode.write(offset, buf)
}

/// Вспомогательная функция для получения stat информации
pub fn vfs_stat(path: &str) -> Result<FileStat, IoError> {
    let vfs = VFS.lock();
    let vnode = vfs.lookup(path)?;
    vnode.stat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path("//"), "/");
        assert_eq!(normalize_path("/home"), "/home");
        assert_eq!(normalize_path("/home/"), "/home");
        assert_eq!(normalize_path("/home//user"), "/home/user");
        assert_eq!(normalize_path("home"), "/home");
    }
}

// PINDOS filesystem — поддержка директорий и путей

pub const MAX_ENTRIES: usize = 128;
pub const MAX_NAME:    usize = 64;
pub const MAX_PATH:    usize = 128;
pub const MAX_CONTENT: usize = 4096;

#[derive(Copy, Clone, PartialEq)]
pub enum EntryKind { File, Dir }

#[derive(Copy, Clone)]
pub struct Entry {
    pub name:        [u8; MAX_NAME],
    pub name_len:    usize,
    pub path:        [u8; MAX_PATH],  // полный путь родителя, напр. "/"
    pub path_len:    usize,
    pub kind:        EntryKind,
    pub content:     [u8; MAX_CONTENT],
    pub content_len: usize,
    pub used:        bool,
}

impl Entry {
    const fn empty() -> Self {
        Entry {
            name: [0u8; MAX_NAME], name_len: 0,
            path: [0u8; MAX_PATH], path_len: 0,
            kind: EntryKind::File,
            content: [0u8; MAX_CONTENT], content_len: 0,
            used: false,
        }
    }
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("?")
    }
    pub fn path_str(&self) -> &str {
        core::str::from_utf8(&self.path[..self.path_len]).unwrap_or("/")
    }
    pub fn content_str(&self) -> &str {
        core::str::from_utf8(&self.content[..self.content_len]).unwrap_or("")
    }
    pub fn is_dir(&self) -> bool { self.kind == EntryKind::Dir }
}

static mut FS: [Entry; MAX_ENTRIES] = [Entry::empty(); MAX_ENTRIES];

// Текущая рабочая директория
static mut CWD: [u8; MAX_PATH] = [b'/'; MAX_PATH];
static mut CWD_LEN: usize = 1;

pub fn cwd() -> &'static str {
    unsafe { core::str::from_utf8(&CWD[..CWD_LEN]).unwrap_or("/") }
}

pub fn set_cwd(path: &str) {
    unsafe {
        let b = path.as_bytes();
        let len = b.len().min(MAX_PATH);
        CWD[..len].copy_from_slice(&b[..len]);
        CWD_LEN = len;
    }
}

/// Инициализация — создаём корневые директории
pub fn init() {
    mkdir_abs("/");
    mkdir_abs("/bin");
    mkdir_abs("/home");
    mkdir_abs("/tmp");
    create_abs("/", "readme.txt", EntryKind::File,
        "Welcome to PINDOS!\nType 'help' for commands.\n");
    create_abs("/home", "notes.txt", EntryKind::File, "Your notes here.\n");
}

// ── Внутренние хелперы ────────────────────────────────────────────────────

fn set_name(entry: &mut Entry, name: &str) {
    let b = name.as_bytes();
    let len = b.len().min(MAX_NAME);
    entry.name = [0u8; MAX_NAME];
    entry.name[..len].copy_from_slice(&b[..len]);
    entry.name_len = len;
}

fn set_path(entry: &mut Entry, path: &str) {
    let b = path.as_bytes();
    let len = b.len().min(MAX_PATH);
    entry.path = [0u8; MAX_PATH];
    entry.path[..len].copy_from_slice(&b[..len]);
    entry.path_len = len;
}

fn mkdir_abs(full_path: &str) {
    // full_path — это и есть путь директории, родитель = dirname
    let (parent, name) = split_path(full_path);
    if name.is_empty() { return; } // корень
    unsafe {
        for slot in FS.iter_mut() {
            if !slot.used {
                slot.used = true;
                slot.kind = EntryKind::Dir;
                set_name(slot, name);
                set_path(slot, parent);
                return;
            }
        }
    }
}

fn create_abs(parent: &str, name: &str, kind: EntryKind, content: &str) -> bool {
    unsafe {
        for slot in FS.iter_mut() {
            if !slot.used {
                slot.used = true;
                slot.kind = kind;
                set_name(slot, name);
                set_path(slot, parent);
                let cb = content.as_bytes();
                let clen = cb.len().min(MAX_CONTENT);
                slot.content[..clen].copy_from_slice(&cb[..clen]);
                slot.content_len = clen;
                return true;
            }
        }
        false
    }
}

/// Разбивает "/foo/bar/baz" → ("/foo/bar", "baz")
pub fn split_path(p: &str) -> (&str, &str) {
    if p == "/" { return ("/", ""); }
    match p.rfind('/') {
        Some(0) => (&p[..1], &p[1..]),
        Some(i) => (&p[..i], &p[i+1..]),
        None    => ("/", p),
    }
}

/// Разрешает путь относительно CWD
pub fn resolve<'a>(path: &'a str, buf: &'a mut [u8; MAX_PATH]) -> &'a str {
    if path.starts_with('/') {
        let b = path.as_bytes();
        let len = b.len().min(MAX_PATH);
        buf[..len].copy_from_slice(&b[..len]);
        return core::str::from_utf8(&buf[..len]).unwrap_or("/");
    }
    // относительный путь
    let base = cwd();
    let mut out = [0u8; MAX_PATH];
    let mut pos = 0usize;
    // копируем base
    for &b in base.as_bytes() { if pos < MAX_PATH { out[pos] = b; pos += 1; } }
    // добавляем '/' если нужно
    if pos > 0 && out[pos-1] != b'/' { if pos < MAX_PATH { out[pos] = b'/'; pos += 1; } }
    for &b in path.as_bytes() { if pos < MAX_PATH { out[pos] = b; pos += 1; } }
    let len = pos.min(MAX_PATH);
    buf[..len].copy_from_slice(&out[..len]);
    core::str::from_utf8(&buf[..len]).unwrap_or("/")
}

// ── Публичный API ─────────────────────────────────────────────────────────

/// Создать файл в текущей директории (или по абсолютному пути)
pub fn create(path: &str, content: &str) -> bool {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(path, &mut buf);
    let (parent, name) = split_path(abs);
    if name.is_empty() { return false; }
    if find_in(parent, name).is_some() { return false; }
    create_abs(parent, name, EntryKind::File, content)
}

/// Создать директорию
pub fn mkdir(path: &str) -> bool {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(path, &mut buf);
    let (parent, name) = split_path(abs);
    if name.is_empty() { return false; }
    if find_in(parent, name).is_some() { return false; }
    unsafe {
        for slot in FS.iter_mut() {
            if !slot.used {
                slot.used = true;
                slot.kind = EntryKind::Dir;
                set_name(slot, name);
                set_path(slot, parent);
                return true;
            }
        }
        false
    }
}

/// Найти запись по имени в директории
pub fn find_in(parent: &str, name: &str) -> Option<usize> {
    unsafe {
        for (i, e) in FS.iter().enumerate() {
            if e.used && e.name_str() == name && e.path_str() == parent {
                return Some(i);
            }
        }
        None
    }
}

/// Найти по полному пути
pub fn find_abs(abs_path: &str) -> Option<usize> {
    if abs_path == "/" {
        // корень — виртуальный
        return None;
    }
    let (parent, name) = split_path(abs_path);
    find_in(parent, name)
}

/// Получить запись по пути (относительному или абсолютному)
pub fn get(path: &str) -> Option<&'static Entry> {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(path, &mut buf);
    unsafe { find_abs(abs).map(|i| &FS[i]) }
}

/// Удалить файл
pub fn delete(path: &str) -> bool {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(path, &mut buf);
    unsafe {
        if let Some(i) = find_abs(abs) {
            FS[i] = Entry::empty();
            return true;
        }
        false
    }
}

/// Переименовать / переместить
pub fn rename(src: &str, dst: &str) -> bool {
    let mut b1 = [0u8; MAX_PATH];
    let mut b2 = [0u8; MAX_PATH];
    let abs_src = resolve(src, &mut b1);
    let abs_dst = resolve(dst, &mut b2);
    unsafe {
        if find_abs(abs_dst).is_some() { return false; }
        if let Some(i) = find_abs(abs_src) {
            let (new_parent, new_name) = split_path(abs_dst);
            set_name(&mut FS[i], new_name);
            set_path(&mut FS[i], new_parent);
            return true;
        }
        false
    }
}

/// Записать содержимое файла
pub fn write(path: &str, content: &str) -> bool {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(path, &mut buf);
    unsafe {
        if let Some(i) = find_abs(abs) {
            let cb = content.as_bytes();
            let clen = cb.len().min(MAX_CONTENT);
            FS[i].content[..clen].copy_from_slice(&cb[..clen]);
            FS[i].content_len = clen;
            return true;
        }
        false
    }
}

/// Дописать в конец файла
pub fn append(path: &str, content: &str) -> bool {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(path, &mut buf);
    unsafe {
        if let Some(i) = find_abs(abs) {
            let cb = content.as_bytes();
            let start = FS[i].content_len;
            let space = MAX_CONTENT - start;
            let clen = cb.len().min(space);
            FS[i].content[start..start+clen].copy_from_slice(&cb[..clen]);
            FS[i].content_len += clen;
            return true;
        }
        false
    }
}

/// Скопировать файл
pub fn copy_file(src: &str, dst: &str) -> bool {
    let mut b1 = [0u8; MAX_PATH];
    let abs_src = resolve(src, &mut b1);
    unsafe {
        if let Some(i) = find_abs(abs_src) {
            let clen = FS[i].content_len;
            let mut tmp = [0u8; MAX_CONTENT];
            tmp[..clen].copy_from_slice(&FS[i].content[..clen]);
            let parent = {
                let mut b2 = [0u8; MAX_PATH];
                let abs_dst = resolve(dst, &mut b2);
                let (p, n) = split_path(abs_dst);
                if n.is_empty() { return false; }
                delete(dst);
                create_abs(p, n, EntryKind::File,
                    core::str::from_utf8(&tmp[..clen]).unwrap_or(""))
            };
            return parent;
        }
        false
    }
}

/// Список записей в директории
pub fn list_dir(dir: &str) -> impl Iterator<Item = &'static Entry> {
    let dir_owned = {
        let mut buf = [0u8; MAX_PATH];
        let abs = resolve(dir, &mut buf);
        let b = abs.as_bytes();
        let mut arr = [0u8; MAX_PATH];
        let len = b.len().min(MAX_PATH);
        arr[..len].copy_from_slice(&b[..len]);
        (arr, len)
    };
    unsafe {
        FS.iter().filter(move |e| {
            e.used && {
                let p = e.path_str();
                let d = core::str::from_utf8(&dir_owned.0[..dir_owned.1]).unwrap_or("/");
                p == d
            }
        })
    }
}

/// Обратная совместимость — list() возвращает всё
pub fn list() -> impl Iterator<Item = &'static Entry> {
    unsafe { FS.iter().filter(|e| e.used) }
}

/// find() по имени в CWD (обратная совместимость)
pub fn find(name: &str) -> Option<usize> {
    let mut buf = [0u8; MAX_PATH];
    let abs = resolve(name, &mut buf);
    find_abs(abs)
}

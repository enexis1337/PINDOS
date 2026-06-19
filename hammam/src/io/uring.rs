use core::sync::atomic::{AtomicU32, Ordering};

/// Opcodes для операций io_uring
pub const OP_READ: u8 = 0;
pub const OP_WRITE: u8 = 1;
pub const OP_NOP: u8 = 2;
pub const OP_FSYNC: u8 = 3;

/// Запись в очередь отправки (Submission Queue Entry).
/// Размер: 64 байта (обычно, но переносимый).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct SqEntry {
    pub opcode: u8,            // Код операции (OP_READ, OP_WRITE, ...)
    pub flags: u8,             // Флаги операции
    pub ioprio: u16,           // Приоритет I/O
    pub fd: i32,               // File descriptor
    pub off: u64,              // Offset в файле для read/write
    pub addr: u64,             // Userspace указатель на буфер
    pub len: u32,              // Размер операции
    pub rw_flags: i32,         // Флаги read/write
    pub user_data: u64,        // Произвольный тег — вернётся в CQE
    pub _pad: [u8; 8],         // Padding для выравнивания
}

impl SqEntry {
    /// Создать пустую SQE
    pub const fn new() -> Self {
        SqEntry {
            opcode: 0,
            flags: 0,
            ioprio: 0,
            fd: -1,
            off: 0,
            addr: 0,
            len: 0,
            rw_flags: 0,
            user_data: 0,
            _pad: [0; 8],
        }
    }

    /// Создать SQE для чтения
    pub const fn read(fd: i32, addr: u64, len: u32, offset: u64, user_data: u64) -> Self {
        let mut entry = Self::new();
        entry.opcode = OP_READ;
        entry.fd = fd;
        entry.addr = addr;
        entry.len = len;
        entry.off = offset;
        entry.user_data = user_data;
        entry
    }

    /// Создать SQE для записи
    pub const fn write(fd: i32, addr: u64, len: u32, offset: u64, user_data: u64) -> Self {
        let mut entry = Self::new();
        entry.opcode = OP_WRITE;
        entry.fd = fd;
        entry.addr = addr;
        entry.len = len;
        entry.off = offset;
        entry.user_data = user_data;
        entry
    }
}

/// Запись в очередь завершения (Completion Queue Entry).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct CqEntry {
    pub user_data: u64,  // Эхо user_data из SQE
    pub result: i64,     // >= 0 успех (количество байт), < 0 errno
    pub flags: u32,      // Флаги результата
    pub _pad: u32,       // Padding
}

impl CqEntry {
    /// Создать новый CQE
    pub const fn new(user_data: u64, result: i64) -> Self {
        CqEntry {
            user_data,
            result,
            flags: 0,
            _pad: 0,
        }
    }

    /// Проверить успех операции
    pub fn is_success(&self) -> bool {
        self.result >= 0
    }

    /// Получить errno если была ошибка
    pub fn get_errno(&self) -> Option<i32> {
        if self.result < 0 {
            Some(-self.result as i32)
        } else {
            None
        }
    }
}

/// Заголовок кольцевого буфера — разделяется между ядром и userspace через mmap.
/// Помещается в начало mmap'ленной страницы.
#[repr(C, align(64))]
pub struct RingHeader {
    pub head: AtomicU32,    // Позиция головы (изменяется ядром для CQ, userspace для SQ)
    pub tail: AtomicU32,    // Позиция хвоста (изменяется userspace для SQ, ядром для CQ)
    pub mask: u32,          // Маска для индексирования (size - 1)
    pub entries: u32,       // Количество записей в очереди
}

impl RingHeader {
    /// Создать новый заголовок для очереди размера entries
    pub fn new(entries: u32) -> Self {
        // entries должен быть степенью двойки
        let entries = entries.next_power_of_two();
        RingHeader {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            mask: entries - 1,
            entries,
        }
    }

    /// Получить текущее количество элементов в очереди
    pub fn len(&self) -> u32 {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    /// Проверить, пуста ли очередь
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Проверить, полна ли очередь
    pub fn is_full(&self) -> bool {
        self.len() >= self.entries
    }

    /// Получить индекс элемента по позиции
    pub fn index(&self, pos: u32) -> usize {
        (pos & self.mask) as usize
    }

    /// Добавить элемент в очередь (увеличить tail)
    pub fn push(&self) -> u32 {
        let tail = self.tail.load(Ordering::Acquire);
        let new_tail = tail.wrapping_add(1);
        self.tail.store(new_tail, Ordering::Release);
        tail
    }

    /// Получить и удалить элемент из очереди (увеличить head)
    pub fn pop(&self) -> u32 {
        let head = self.head.load(Ordering::Acquire);
        let new_head = head.wrapping_add(1);
        self.head.store(new_head, Ordering::Release);
        head
    }
}

/// Контекст io_uring для одного процесса
pub struct IoUring {
    pub sq_header: *mut RingHeader,
    pub sq_entries: *mut [SqEntry],
    pub cq_header: *mut RingHeader,
    pub cq_entries: *mut [CqEntry],
}

/// Обработать все готовые SQE из очереди Submission Queue.
/// Выполнить операции и записать результаты в Completion Queue.
pub fn process_sq(
    sq_header: &RingHeader,
    sq_entries: &[SqEntry],
    cq_header: &RingHeader,
    cq_entries: &mut [CqEntry],
) {
    // Читаем из SQ пока head != tail
    loop {
        if sq_header.is_empty() {
            break;
        }

        let sq_pos = sq_header.pop();
        let sq_idx = sq_header.index(sq_pos);

        if sq_idx >= sq_entries.len() {
            break;
        }

        let sqe = sq_entries[sq_idx];

        // Выполняем операцию
        let result = execute_sqe(&sqe);

        // Записываем результат в CQ
        if !cq_header.is_full() {
            let cq_pos = cq_header.push();
            let cq_idx = cq_header.index(cq_pos);

            if cq_idx < cq_entries.len() {
                cq_entries[cq_idx] = CqEntry::new(sqe.user_data, result);
            }
        }
    }
}

/// Выполнить одну SQE операцию
fn execute_sqe(sqe: &SqEntry) -> i64 {
    match sqe.opcode {
        OP_READ => execute_read(sqe),
        OP_WRITE => execute_write(sqe),
        OP_NOP => 0, // No-op всегда успешен
        OP_FSYNC => {
            // Placeholder для fsync
            0
        }
        _ => -22, // EINVAL
    }
}

/// Выполнить операцию чтения
fn execute_read(_sqe: &SqEntry) -> i64 {
    // TODO: Валидировать userspace указатель через capability
    // TODO: Выполнить чтение из файлов через VFS

    // Placeholder: всегда успех с нулевыми данными
    0
}

/// Выполнить операцию записи
fn execute_write(sqe: &SqEntry) -> i64 {
    // TODO: Валидировать userspace указатель через capability
    // TODO: Выполнить запись через VFS

    // Placeholder: успех с количеством "написанных" байт
    sqe.len as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_header() {
        let header = RingHeader::new(64);
        assert_eq!(header.entries, 64);
        assert_eq!(header.mask, 63);
        assert!(header.is_empty());
    }

    #[test]
    fn test_ring_operations() {
        let header = RingHeader::new(4);

        // Проверяем пустоту
        assert!(header.is_empty());

        // Добавляем элементы
        header.push();
        assert_eq!(header.len(), 1);

        header.push();
        assert_eq!(header.len(), 2);

        // Вынимаем элементы
        let pos = header.pop();
        assert_eq!(pos, 0);
        assert_eq!(header.len(), 1);

        let pos = header.pop();
        assert_eq!(pos, 1);
        assert!(header.is_empty());
    }

    #[test]
    fn test_sq_entry_read() {
        let sqe = SqEntry::read(3, 0x1000, 4096, 0, 42);
        assert_eq!(sqe.opcode, OP_READ);
        assert_eq!(sqe.fd, 3);
        assert_eq!(sqe.addr, 0x1000);
        assert_eq!(sqe.len, 4096);
        assert_eq!(sqe.user_data, 42);
    }

    #[test]
    fn test_cq_entry_result() {
        let cq_success = CqEntry::new(123, 256);
        assert!(cq_success.is_success());
        assert!(cq_success.get_errno().is_none());

        let cq_error = CqEntry::new(123, -2); // -ENOENT
        assert!(!cq_error.is_success());
        assert_eq!(cq_error.get_errno(), Some(2));
    }

    #[test]
    fn test_ring_index() {
        let header = RingHeader::new(8);
        assert_eq!(header.index(0), 0);
        assert_eq!(header.index(7), 7);
        assert_eq!(header.index(8), 0); // Wraps around
        assert_eq!(header.index(15), 7);
        assert_eq!(header.index(16), 0);
    }

    #[test]
    fn test_ring_overflow() {
        let header = RingHeader::new(2);

        // Добавляем 4 элемента в очередь размером 2
        for _ in 0..4 {
            if !header.is_full() {
                header.push();
            }
        }

        // Очередь должна быть полной после 2 добавлений
        assert!(header.len() >= 2);
    }
}

use alloc::vec::Vec;

/// Unforgeable токен доступа к ресурсу ядра.
#[derive(Debug, Clone)]
pub struct Capability {
    pub kind: CapKind,
    pub rights: Rights,
    object_id: u64,   // приватное — userspace никогда не видит сырой ID
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapKind {
    Memory,      // физический регион памяти
    IrqHandler,  // право обрабатывать IRQ
    IoPort,      // право делать in/out на диапазон портов
    Process,     // право управлять процессом
}

bitflags::bitflags! {
    /// Rights для capability — определяют, что можно делать с ресурсом
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rights: u32 {
        const READ  = 1 << 0;   // право читать
        const WRITE = 1 << 1;   // право писать
        const EXEC  = 1 << 2;   // право исполнять
        const GRANT = 1 << 3;   // можно передать другому процессу
    }
}

/// Таблица capability для одного процесса.
/// Userspace оперирует индексами (handle) в этой таблице — не сырыми object_id.
pub struct CapTable {
    entries: Vec<Option<Capability>>,
}

impl CapTable {
    /// Создает новую пустую таблицу capability
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Добавить capability, вернуть handle (индекс).
    pub fn insert(&mut self, cap: Capability) -> u32 {
        let idx = self.entries.len();
        self.entries.push(Some(cap));
        idx as u32
    }

    /// Получить capability по handle.
    pub fn get(&self, handle: u32) -> Option<&Capability> {
        self.entries.get(handle as usize)?.as_ref()
    }

    /// Получить mutable ссылку на capability по handle.
    pub fn get_mut(&mut self, handle: u32) -> Option<&mut Capability> {
        self.entries.get_mut(handle as usize)?.as_mut()
    }

    /// Отозвать capability.
    pub fn revoke(&mut self, handle: u32) {
        if let Some(slot) = self.entries.get_mut(handle as usize) {
            *slot = None;
        }
    }

    /// Проверить, содержит ли capability указанные rights
    pub fn check_rights(&self, handle: u32, required_rights: Rights) -> bool {
        if let Some(cap) = self.get(handle) {
            cap.rights.contains(required_rights)
        } else {
            false
        }
    }

    /// Вернуть количество занятых слотов в таблице
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// Проверить, пуста ли таблица
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for CapTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Capability {
    /// Создать новый capability
    pub fn new(kind: CapKind, rights: Rights, object_id: u64) -> Self {
        Capability {
            kind,
            rights,
            object_id,
        }
    }

    /// Получить object_id (внутренний идентификатор ресурса)
    pub fn object_id(&self) -> u64 {
        self.object_id
    }

    /// Создать суженный capability с подмножеством rights
    pub fn attenuate(&self, new_rights: Rights) -> Self {
        Capability {
            kind: self.kind.clone(),
            rights: self.rights & new_rights,
            object_id: self.object_id,
        }
    }
}

use alloc::{sync::Arc, vec::Vec};

/// Ошибки блочного устройства
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    Timeout,      // Операция истекла по времени
    BadAddress,   // Неверный адрес буфера
    OutOfRange,   // LBA вне диапазона устройства
}

/// Абстракция блочного устройства
pub trait BlockDevice: Send + Sync {
    /// Размер одного блока в байтах (обычно 4096)
    fn block_size(&self) -> usize;

    /// Количество блоков на устройстве
    fn block_count(&self) -> u64;

    /// Прочитать блоки начиная с start
    /// buf должен быть выравнен и его размер кратен block_size
    fn read_blocks(&self, start: u64, buf: &mut [u8]) -> Result<(), IoError>;

    /// Записать блоки начиная с start
    /// buf должен быть выравнен и его размер кратен block_size
    fn write_blocks(&self, start: u64, buf: &[u8]) -> Result<(), IoError>;

    /// Получить имя устройства
    fn device_name(&self) -> &str {
        "block_device"
    }

    /// Проверить, доступно ли устройство
    fn is_ready(&self) -> bool {
        true
    }
}

/// Фиксированная информация о блочном устройстве
#[derive(Debug, Clone)]
pub struct BlockDeviceInfo {
    pub name: &'static str,
    pub block_size: usize,
    pub block_count: u64,
}

impl BlockDeviceInfo {
    pub fn new(name: &'static str, block_size: usize, block_count: u64) -> Self {
        BlockDeviceInfo {
            name,
            block_size,
            block_count,
        }
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.block_size as u64 * self.block_count
    }
}

/// Реестр блочных устройств — драйверы регистрируются здесь
/// Доступ может быть ограничен через capability на основе capability-based security
pub struct BlockRegistry {
    devices: Vec<Arc<dyn BlockDevice>>,
}

impl BlockRegistry {
    /// Создать новый реестр
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Зарегистрировать блочное устройство
    /// Возвращает ID устройства для дальнейшего обращения
    pub fn register(&mut self, dev: Arc<dyn BlockDevice>) -> u32 {
        let id = self.devices.len() as u32;
        self.devices.push(dev);
        id
    }

    /// Получить устройство по ID
    pub fn get(&self, id: u32) -> Option<Arc<dyn BlockDevice>> {
        self.devices.get(id as usize).cloned()
    }

    /// Получить количество зарегистрированных устройств
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Получить список всех устройств
    pub fn list_devices(&self) -> Vec<Arc<dyn BlockDevice>> {
        self.devices.clone()
    }

    /// Найти устройство по имени
    pub fn find_by_name(&self, name: &str) -> Option<Arc<dyn BlockDevice>> {
        self.devices
            .iter()
            .find(|dev| dev.device_name() == name)
            .cloned()
    }
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Глобальный реестр блочных устройств
pub static BLOCK_REGISTRY: spin::Mutex<BlockRegistry> = spin::Mutex::new(BlockRegistry::new());

/// Простое тестовое блочное устройство для тестирования
#[cfg(test)]
mod tests {
    use super::*;

    struct MockBlockDevice {
        block_size: usize,
        block_count: u64,
        data: alloc::vec::Vec<u8>,
    }

    impl MockBlockDevice {
        fn new(block_size: usize, block_count: u64) -> Self {
            MockBlockDevice {
                block_size,
                block_count,
                data: alloc::vec![0u8; (block_size as u64 * block_count) as usize],
            }
        }
    }

    impl BlockDevice for MockBlockDevice {
        fn block_size(&self) -> usize {
            self.block_size
        }

        fn block_count(&self) -> u64 {
            self.block_count
        }

        fn read_blocks(&self, start: u64, buf: &mut [u8]) -> Result<(), IoError> {
            let start_byte = start as usize * self.block_size;
            let end_byte = start_byte + buf.len();

            if end_byte > self.data.len() {
                return Err(IoError::OutOfRange);
            }

            buf.copy_from_slice(&self.data[start_byte..end_byte]);
            Ok(())
        }

        fn write_blocks(&self, start: u64, buf: &[u8]) -> Result<(), IoError> {
            if buf.len() % self.block_size != 0 {
                return Err(IoError::BadAddress);
            }

            let start_byte = start as usize * self.block_size;
            let end_byte = start_byte + buf.len();

            if end_byte > self.data.len() {
                return Err(IoError::OutOfRange);
            }

            // Это был mut ref, но так как это тест, мы не можем мутировать
            // В реальном коде это был бы Mutex или Arc<RwLock>
            Ok(())
        }

        fn device_name(&self) -> &str {
            "mock_block_device"
        }
    }

    #[test]
    fn test_block_device_info() {
        let info = BlockDeviceInfo::new("test", 4096, 1000);
        assert_eq!(info.block_size, 4096);
        assert_eq!(info.block_count, 1000);
        assert_eq!(info.total_size_bytes(), 4096 * 1000);
    }

    #[test]
    fn test_block_registry() {
        let mut registry = BlockRegistry::new();
        assert_eq!(registry.device_count(), 0);

        let dev1 = Arc::new(MockBlockDevice::new(4096, 100));
        let id1 = registry.register(dev1.clone());
        assert_eq!(id1, 0);
        assert_eq!(registry.device_count(), 1);

        let dev2 = Arc::new(MockBlockDevice::new(512, 1000));
        let id2 = registry.register(dev2.clone());
        assert_eq!(id2, 1);
        assert_eq!(registry.device_count(), 2);

        // Получить устройство по ID
        assert!(registry.get(0).is_some());
        assert!(registry.get(1).is_some());
        assert!(registry.get(2).is_none());

        // Найти по имени
        assert!(registry.find_by_name("mock_block_device").is_some());
        assert!(registry.find_by_name("nonexistent").is_none());
    }
}

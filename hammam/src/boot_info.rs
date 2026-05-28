/// Информация о загрузке, передаваемая от загрузчика (UEFI) ядру Hammam.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Карта физической памяти системы
    pub memory_map: &'static [MemoryRegion],
    /// Информация о фреймбуфере (видеорежиме)
    pub framebuffer: Option<FramebufferInfo>,
    /// Физический адрес структуры RSDP (ACPI Root System Description Pointer)
    pub rsdp_addr: Option<u64>,
    /// Физический адрес базы загруженного ядра
    pub kernel_phys_base: u64,
}

/// Область памяти с указанием её типа.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegion {
    /// Стартовый физический адрес области
    pub start: u64,
    /// Конечный физический адрес (не включая его)
    pub end: u64,
    /// Тип памяти
    pub kind: MemoryKind,
}

/// Типы физической памяти, совместимые с UEFI/ACPI и физическим аллокатором ядра.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    /// Доступная для ядра и приложений оперативная память
    Usable,
    /// Зарезервировано BIOS/UEFI/железом, использовать нельзя
    Reserved,
    /// Память, содержащая код или данные UEFI Boot Services (можно освободить после инициализации)
    UefiBootServices,
    /// Память, содержащая код или данные UEFI Runtime Services (должна быть сохранена)
    UefiRuntimeServices,
    /// Память, занятая кодом и статическими данными самого ядра Hammam
    Kernel,
    /// Память, занятая структурами загрузчика или initramfs
    Bootloader,
    /// Участки ACPI NVS (Non-Volatile Storage)
    AcpiNvs,
    /// Таблицы данных ACPI
    AcpiReclaimable,
    /// Отображаемая память устройств (Memory-Mapped I/O)
    Mmio,
}

/// Информация о графическом фреймбуфере для вывода на экран реального ПК.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    /// Физический адрес начала фреймбуфера
    pub address: u64,
    /// Размер фреймбуфера в байтах
    pub size: usize,
    /// Ширина экрана в пикселях
    pub width: u32,
    /// Высота экрана в пикселях
    pub height: u32,
    /// Формат пикселей (RGB/BGR)
    pub pixel_format: PixelFormat,
    /// Количество байт/пикселей на строку (stride / pitch)
    pub bytes_per_pixel: u8,
    pub stride: u32,
}

/// Поддерживаемые форматы пикселей для работы с реальными видеокартами через UEFI GOP.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb,
    Bgr,
    U8,
}
